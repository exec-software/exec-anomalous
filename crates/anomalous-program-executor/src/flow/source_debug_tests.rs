extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{sha256, source_debug, validation_workspace};
use anomalous_program_manifest::maximum_image_bytes;
use std::{boxed::Box, format, string::String, vec, vec::Vec};

#[derive(Default)]
struct recorder {
    output: Vec<u8>,
    identities: Vec<[u8; 32]>,
}

impl console_sink for recorder {
    fn write(&mut self, request: console_request<'_>) -> Result<(), service_error> {
        self.output.extend_from_slice(request.bytes());
        self.identities.push(request.edition_identity());
        Ok(())
    }
}

fn source(body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule debug_test\n@ai_context(\"Run with descriptive source metadata\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn with_storage(test: impl FnOnce() + Send) {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn_scoped(scope, test)
            .unwrap()
            .join()
            .unwrap();
    });
}

#[test]
fn lexical_descriptions_do_not_change_execution_or_authority() {
    with_storage(|| {
        let plain = source(
            "PUSH_CONST 42\nSTORE_LOCAL x\nLOAD_LOCAL x\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
        );
        let described = source(
            "# source_debug 0 \"/nonexistent/anomalous/source.matrix\" 1 \"pretend\" \"unregistered\" \"foreign::pointer\" 1024\nPUSH_CONST 42\nSTORE_LOCAL x\n# source_debug 2 \"/nonexistent/anomalous/source.matrix\" 1 \"x\" \"x\" \"text\" 1\nLOAD_LOCAL x\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
        );
        let mut previous_statistics = None;
        for edition in [&plain, &described] {
            let mut validation = Box::new(validation_workspace::new());
            let mut image = vec![0; maximum_image_bytes];
            let size =
                source_debug::encode(edition.as_bytes(), &mut image, &mut validation).unwrap();
            let admitted = source_debug::admit(&image[..size], 1, &mut validation).unwrap();
            assert_eq!(admitted.profile(), 10);
            assert_eq!(admitted.granted_services(), 1);
            let mut workspace = Box::new(execution_workspace::new());
            let mut sink = recorder::default();
            let result = execute(&admitted, limits::default(), &mut workspace, &mut sink);
            assert_eq!(result.outcome, Ok(0));
            assert_eq!(sink.output, b"42\n");
            assert_eq!(sink.identities, [sha256(edition.as_bytes())]);
            assert_eq!(result.statistics.instructions, 6);
            if let Some(expected) = previous_statistics {
                assert_eq!(result.statistics, expected);
            }
            previous_statistics = Some(result.statistics);
        }
    });
}

#[test]
fn debug_records_do_not_consume_instruction_fuel() {
    with_storage(|| {
        let edition = source(
            "# source_debug 0 \"fuel.matrix\" 0\nPUSH_CONST 17\n# source_debug 1 \"fuel.matrix\" 0\nRET",
        );
        let mut validation = Box::new(validation_workspace::new());
        let mut image = vec![0; maximum_image_bytes];
        let size = source_debug::encode(edition.as_bytes(), &mut image, &mut validation).unwrap();
        let admitted = source_debug::admit(&image[..size], 0, &mut validation).unwrap();
        let mut workspace = Box::new(execution_workspace::new());
        for (instructions, expected) in [(1, Err(error::instruction_limit)), (2, Ok(17))] {
            let result = execute(
                &admitted,
                limits {
                    instructions,
                    ..limits::default()
                },
                &mut workspace,
                &mut recorder::default(),
            );
            assert_eq!(result.outcome, expected);
            assert_eq!(result.statistics.instructions, instructions);
            assert_eq!(workspace.depth, 0);
            assert_eq!(workspace.frame_count, 0);
        }
    });
}

#[test]
fn fault_retains_inspectable_metadata_and_scrubs_execution_state() {
    with_storage(|| {
        let edition = source(
            "PUSH_CONST 7\nCONSOLE_WRITELN\nPUSH_CONST 1\nPUSH_CONST 0\n# source_debug 4 \"fault.matrix\" 1 \"denominator\" \"absent\" \"i64\" 2\nDIV i64\nRET",
        );
        let mut validation = Box::new(validation_workspace::new());
        let mut image = vec![0; maximum_image_bytes];
        let size = source_debug::encode(edition.as_bytes(), &mut image, &mut validation).unwrap();
        let admitted = source_debug::admit(&image[..size], 1, &mut validation).unwrap();
        let mut workspace = Box::new(execution_workspace::new());
        let mut sink = recorder::default();
        let result = execute(&admitted, limits::default(), &mut workspace, &mut sink);
        assert_eq!(result.outcome, Err(error::division_by_zero));
        assert_eq!(result.statistics.instructions, 5);
        assert_eq!(result.line, admitted.program().instructions()[4].line);
        assert_eq!(sink.output, b"7\n");
        let record = admitted.program().source_debug(4).unwrap();
        assert_eq!(record.path().bytes().collect::<Vec<_>>(), b"fault.matrix");
        assert_eq!(record.instruction_index(), 4);
        assert_eq!(record.bindings().next().unwrap().scope_depth, 2);
        assert_eq!(
            (
                workspace.depth,
                workspace.frame_count,
                workspace.pc,
                workspace.line
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(workspace.stats, statistics::default());
        assert!(
            workspace
                .values
                .iter()
                .all(|value| matches!(value, value::integer(0)))
        );
        assert!(
            workspace
                .frames
                .iter()
                .all(|frame| frame.locals.iter().all(Option::is_none))
        );
    });
}
