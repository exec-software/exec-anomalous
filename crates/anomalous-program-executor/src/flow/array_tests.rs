extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{array as array_profile, sha256, validation_workspace};
use anomalous_program_manifest::maximum_image_bytes;
use std::{boxed::Box, format, string::String, vec, vec::Vec};

#[derive(Default)]
struct recorder {
    output: Vec<u8>,
    identities: Vec<[u8; 32]>,
    refuse: bool,
}
impl console_sink for recorder {
    fn write(&mut self, request: console_request<'_>) -> Result<(), service_error> {
        assert_eq!(request.sequence(), self.identities.len() + 1);
        self.identities.push(request.edition_identity());
        if self.refuse {
            return Err(service_error::refused);
        }
        self.output.extend_from_slice(request.bytes());
        Ok(())
    }
}
fn source(body: &str, helpers: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule array_test\n# String constants\nconst S0 = \"first\"\nconst S1 = \"second\"\nconst S2 = \"\\\\\"\nconst S3 = \"n\"\n@ai_context(\"Exercise immutable typed arrays\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n{helpers}"
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
fn assert_cleared(workspace: &execution_workspace<'_>) {
    assert!(workspace.arrays.is_clear());
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
    assert_eq!(workspace.text.used, 0);
    assert!(workspace.text.storage.iter().all(|byte| *byte == 0));
    assert!(
        workspace
            .values
            .iter()
            .all(|item| matches!(item, value::integer(0)))
    );
    assert!(
        workspace
            .frames
            .iter()
            .all(|frame| frame.locals.iter().all(Option::is_none))
    );
}
fn run(edition: &[u8], budget: limits, sink: &mut recorder) -> execution {
    let mut validation = Box::new(validation_workspace::new());
    let mut image = vec![0; maximum_image_bytes];
    let size = array_profile::encode(edition, &mut image, &mut validation).unwrap();
    let admitted = array_profile::admit(&image[..size], 1, &mut validation).unwrap();
    assert_eq!(admitted.profile(), 5);
    let mut workspace = Box::new(execution_workspace::new());
    let result = execute(&admitted, budget, &mut workspace, sink);
    assert_eq!(result.edition_identity, sha256(edition));
    assert!(
        sink.identities
            .iter()
            .all(|id| *id == result.edition_identity)
    );
    assert_cleared(&workspace);
    result
}
#[test]
fn literal_order_and_full_width_integer_elements() {
    with_storage(|| {
        let edition = source(
            "PUSH_CONST -9223372036854775808\nPUSH_CONST 9223372036854775807\nNEW_ARRAY_LITERAL 2 i64\nSTORE_LOCAL a\nLOAD_LOCAL a\nLEN\nCONSOLE_WRITELN\nLOAD_LOCAL a\nPUSH_CONST 0\nGET_INDEX\nCONSOLE_WRITELN\nLOAD_LOCAL a\nPUSH_CONST 1\nGET_INDEX\nRET",
            "",
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), limits::default(), &mut sink).outcome,
            Ok(i64::MAX)
        );
        assert_eq!(sink.output, b"2\n-9223372036854775808\n");
    });
}
#[test]
fn typed_defaults_cover_empty_negative_and_extreme_indices() {
    with_storage(|| {
        for ty in ["i64", "text"] {
            let mut body = format!("NEW_ARRAY_LITERAL 0 {ty}\nSTORE_LOCAL a\n");
            for index in [i64::MIN, -1, 0, 1, i64::MAX] {
                body.push_str(&format!(
                    "LOAD_LOCAL a\nPUSH_CONST {index}\nGET_INDEX\nCONSOLE_WRITELN\n"
                ));
            }
            body.push_str("LOAD_LOCAL a\nLEN\nRET");
            let edition = source(&body, "");
            let mut sink = recorder::default();
            assert_eq!(
                run(edition.as_bytes(), limits::default(), &mut sink).outcome,
                Ok(0)
            );
            assert_eq!(
                sink.output,
                if ty == "i64" {
                    b"0\n0\n0\n0\n0\n".as_slice()
                } else {
                    b"\n\n\n\n\n"
                }
            );
        }
    });
}
#[test]
fn returned_text_array_aliases_keep_decoded_text_and_caller_floor() {
    with_storage(|| {
        let edition = source(
            "PUSH_CONST 42\nCALL make 0\nSTORE_LOCAL a\nLOAD_LOCAL a\nSTORE_LOCAL alias\nLOAD_LOCAL a\nCALL show 1\nNEW_ARRAY_LITERAL 0 text\nSTORE_LOCAL a\nLOAD_LOCAL alias\nPUSH_CONST 1\nGET_INDEX\nCONSOLE_WRITELN\nRET",
            "# declaration_line 1\nfunction make -> array<text>:\nPUSH_CONST_STR S2\nPUSH_CONST_STR S3\nSTR_CONCAT\nPUSH_CONST_STR S1\nNEW_ARRAY_LITERAL 2 text\nRET\nend function\n# declaration_line 1\nfunction show(a: array<text>) -> void:\nSTORE_LOCAL a\nLOAD_LOCAL a\nPUSH_CONST 0\nGET_INDEX\nCONSOLE_WRITELN\nRET\nend function\n",
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), limits::default(), &mut sink).outcome,
            Ok(42)
        );
        assert_eq!(sink.output, b"\\n\nsecond\n");
    });
}
#[test]
fn descriptor_exhaustion_keeps_prior_effects_and_scrubs_storage() {
    with_storage(|| {
        for count in [maximum_arrays, maximum_arrays + 1] {
            let edition = source(
                &format!(
                    "PUSH_CONST 7\nCONSOLE_WRITELN\n{}PUSH_CONST 0\nRET",
                    "NEW_ARRAY_LITERAL 0 i64\nPOP\n".repeat(count)
                ),
                "",
            );
            let mut sink = recorder::default();
            let result = run(edition.as_bytes(), limits::default(), &mut sink);
            assert_eq!(
                result.outcome,
                if count == maximum_arrays {
                    Ok(0)
                } else {
                    Err(error::array_limit)
                }
            );
            assert_eq!(sink.output, b"7\n");
            if count > maximum_arrays {
                assert_eq!(result.statistics.instructions, 259);
            }
        }
    });
}
#[test]
fn element_exhaustion_retains_aliases_and_failed_allocation_is_atomic() {
    with_storage(|| {
        let mut arena = Box::new(array_arena::new());
        let first = arena
            .allocate(&[value::integer(42); 256], kind::integer)
            .unwrap();
        for _ in 1..4 {
            arena
                .allocate(&[value::integer(7); 256], kind::integer)
                .unwrap();
        }
        let empty = arena.allocate(&[], kind::text).unwrap();
        assert_eq!(arena.length(empty), Ok(0));
        assert_eq!(
            arena.allocate(&[value::integer(9)], kind::integer).err(),
            Some(error::array_limit)
        );
        assert!(matches!(arena.get(first, 255), Ok(value::integer(42))));
        assert!(matches!(arena.get(first, 256), Ok(value::integer(0))));
        assert_eq!(
            arena.allocate(&[value::text("bad")], kind::integer).err(),
            Some(error::invalid_state)
        );
        assert_eq!(arena.length(first), Ok(256));
        arena.clear();
        assert!(arena.is_clear());
        assert_eq!(arena.length(first), Err(error::invalid_state));
    });
}
#[test]
fn dispatcher_element_exhaustion_clears_storage_and_preserves_prior_output() {
    with_storage(|| {
        let block = format!(
            "{}NEW_ARRAY_LITERAL 256 i64\nPOP\n",
            "PUSH_CONST 1\n".repeat(256)
        );
        let edition = source(
            &format!(
                "PUSH_CONST 7\nCONSOLE_WRITELN\n{}PUSH_CONST 1\nNEW_ARRAY_LITERAL 1 i64\nPOP\nPUSH_CONST 0\nRET",
                block.repeat(4)
            ),
            "",
        );
        let mut sink = recorder::default();
        let result = run(edition.as_bytes(), limits::default(), &mut sink);
        assert_eq!(result.outcome, Err(error::array_limit));
        assert_eq!(result.statistics.instructions, 1036);
        assert_eq!(sink.output, b"7\n");
    });
}

#[test]
fn allocation_fault_fuel_and_service_failures_scrub_arrays() {
    with_storage(|| {
        let prefix = "PUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\nNEW_ARRAY_LITERAL 1 text\nSTORE_LOCAL a\n";
        for (tail, steps, refuse, expected) in [
            (
                "PUSH_CONST 1\nPUSH_CONST 0\nDIV i64\nRET",
                maximum_steps,
                false,
                error::division_by_zero,
            ),
            ("PUSH_CONST 0\nRET", 4, false, error::instruction_limit),
            (
                "LOAD_LOCAL a\nPUSH_CONST 0\nGET_INDEX\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
                maximum_steps,
                true,
                error::service,
            ),
        ] {
            let edition = source(&format!("{prefix}{tail}"), "");
            let mut sink = recorder {
                refuse,
                ..recorder::default()
            };
            assert_eq!(
                run(
                    edition.as_bytes(),
                    limits {
                        instructions: steps,
                        ..limits::default()
                    },
                    &mut sink
                )
                .outcome,
                Err(expected)
            );
            assert!(sink.output.is_empty());
        }
    });
}
#[cfg(feature = "host_conformance")]
#[test]
fn raw_array_path_reuses_cleared_workspace() {
    with_storage(|| {
        let editions = [
            source(
                "PUSH_CONST 42\nNEW_ARRAY_LITERAL 1 i64\nPUSH_CONST 0\nGET_INDEX\nRET",
                "",
            ),
            source("NEW_ARRAY_LITERAL 0 text\nLEN\nRET", ""),
        ];
        let mut validation = Box::new(validation_workspace::new());
        let mut workspace = Box::new(execution_workspace::new());
        for (edition, expected) in editions.iter().zip([42, 0]) {
            let checked = anomalous_program_flow::conformance::validate_array(
                edition.as_bytes(),
                1,
                &mut validation,
            )
            .unwrap();
            let result = crate::conformance::execute(
                &checked,
                limits::default(),
                &mut workspace,
                &mut recorder::default(),
            );
            assert_eq!(result.outcome, Ok(expected));
            assert_cleared(&workspace);
        }
    });
}
