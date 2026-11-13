extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{callable, sha256, validation_workspace};
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
        "# NIL Textual Module\nmodule callable_test\n# String constants\nconst S0 = \"hello \"\nconst S1 = \"world\"\n@ai_context(\"Exercise typed internal dispatch\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n{helpers}"
    )
}
fn helper(name: &str, parameters: &str, result: &str, body: &str) -> String {
    format!(
        "# declaration_line 1\nfunction {name}{parameters} -> {result}:\n{body}\nend function\n"
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
fn run(edition: &[u8], ceiling: u64, budget: limits, sink: &mut recorder) -> execution {
    let mut validation = Box::new(validation_workspace::new());
    let mut image = vec![0; maximum_image_bytes];
    let size = callable::encode(edition, &mut image, &mut validation).unwrap();
    let admitted = callable::admit(&image[..size], ceiling, &mut validation).unwrap();
    assert_eq!(admitted.profile(), 6);
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
fn aliases_preserve_target_identity_after_same_signature_reassignment() {
    with_storage(|| {
        let helpers = helper("first", "", "i64", "PUSH_CONST 7\nRET")
            + &helper("second", "", "i64", "PUSH_CONST 42\nRET");
        let edition = source(
            "PUSH_FN_PTR first\nSTORE_LOCAL selected\nLOAD_LOCAL selected\nSTORE_LOCAL alias\nPUSH_FN_PTR second\nSTORE_LOCAL selected\nCALL alias 0\nCONSOLE_WRITELN\nCALL selected 0\nRET",
            &helpers,
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
            Ok(42)
        );
        assert_eq!(sink.output, b"7\n");
    });
}
#[test]
fn same_signature_branch_join_dispatches_the_selected_target() {
    with_storage(|| {
        let helpers = helper("first", "", "i64", "PUSH_CONST 7\nRET")
            + &helper("second", "", "i64", "PUSH_CONST 42\nRET");
        for (condition, expected) in [(0, 42), (1, 7)] {
            let edition = source(
                &format!(
                    "PUSH_CONST {condition}\nJMP_IF_NOT alternative\nPUSH_FN_PTR first\nJMP joined\nLABEL alternative\nPUSH_FN_PTR second\nLABEL joined\nSTORE_LOCAL selected\nCALL selected 0\nRET"
                ),
                &helpers,
            );
            assert_eq!(
                run(
                    edition.as_bytes(),
                    0,
                    limits::default(),
                    &mut recorder::default()
                )
                .outcome,
                Ok(expected)
            );
        }
    });
}
#[test]
fn argument_order_caller_prefix_and_callee_local_names_are_isolated() {
    with_storage(|| {
        let helpers = helper(
            "subtract",
            "(left: i64, right: i64)",
            "i64",
            "STORE_LOCAL right\nSTORE_LOCAL left\nPUSH_FN_PTR constant\nSTORE_LOCAL selected\nLOAD_LOCAL left\nLOAD_LOCAL right\nSUB i64\nRET",
        ) + &helper("constant", "", "i64", "PUSH_CONST 99\nRET");
        let edition = source(
            "PUSH_CONST 100\nPUSH_FN_PTR subtract\nSTORE_LOCAL selected\nPUSH_CONST 20\nPUSH_CONST 3\nCALL selected 2\nCONSOLE_WRITELN\nPUSH_CONST 9\nPUSH_CONST 2\nCALL selected 2\nADD i64\nRET",
            &helpers,
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
            Ok(107)
        );
        assert_eq!(sink.output, b"17\n");
    });
}
#[test]
fn local_shadowing_internal_and_builtin_names_keeps_zero_authority() {
    with_storage(|| {
        let helpers = helper("declared", "", "i64", "PUSH_CONST 7\nRET")
            + &helper("replacement", "", "i64", "PUSH_CONST 42\nRET");
        for local in ["declared", "println", "console__write"] {
            let edition = source(
                &format!("PUSH_FN_PTR replacement\nSTORE_LOCAL {local}\nCALL {local} 0\nRET"),
                &helpers,
            );
            let mut sink = recorder::default();
            assert_eq!(
                run(edition.as_bytes(), 0, limits::default(), &mut sink).outcome,
                Ok(42)
            );
            assert!(sink.output.is_empty());
        }
    });
}
#[test]
fn computed_text_and_array_returns_keep_the_existing_owned_lifetimes() {
    with_storage(|| {
        let helpers = helper(
            "make",
            "",
            "array<text>",
            "PUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\nNEW_ARRAY_LITERAL 1 text\nRET",
        ) + &helper(
            "show",
            "(items: array<text>)",
            "void",
            "STORE_LOCAL items\nLOAD_LOCAL items\nPUSH_CONST 0\nGET_INDEX\nCONSOLE_WRITELN\nRET",
        );
        let edition = source(
            "PUSH_CONST 42\nPUSH_FN_PTR make\nSTORE_LOCAL make_selected\nPUSH_FN_PTR show\nSTORE_LOCAL show_selected\nCALL make_selected 0\nSTORE_LOCAL items\nLOAD_LOCAL items\nCALL show_selected 1\nLOAD_LOCAL items\nPUSH_CONST 0\nGET_INDEX\nCONSOLE_WRITELN\nRET",
            &helpers,
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
            Ok(42)
        );
        assert_eq!(sink.output, b"hello world\nhello world\n");
    });
}
#[test]
fn recursive_indirect_calls_use_bounded_frames_and_preserve_prior_output() {
    with_storage(|| {
        let helpers = helper(
            "recur",
            "(n: i64)",
            "i64",
            "STORE_LOCAL n\nPUSH_FN_PTR recur\nSTORE_LOCAL next_call\nLOAD_LOCAL n\nPUSH_CONST 1\nCMP_LE\nJMP_IF_NOT recursive\nPUSH_CONST 1\nRET\nLABEL recursive\nLOAD_LOCAL n\nLOAD_LOCAL n\nPUSH_CONST 1\nSUB i64\nCALL next_call 1\nMUL i64\nRET",
        );
        for (n, expected) in [(5, Ok(120)), (20, Err(error::frame_limit))] {
            let edition = source(
                &format!(
                    "PUSH_CONST 7\nCONSOLE_WRITELN\nPUSH_FN_PTR recur\nSTORE_LOCAL selected\nPUSH_CONST {n}\nCALL selected 1\nRET"
                ),
                &helpers,
            );
            let mut sink = recorder::default();
            assert_eq!(
                run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
                expected
            );
            assert_eq!(sink.output, b"7\n");
        }
    });
}
#[test]
fn indirect_fault_fuel_and_service_refusals_scrub_all_storage() {
    with_storage(|| {
        for (body, steps, refuse, expected) in [
            (
                "PUSH_CONST 1\nPUSH_CONST 0\nDIV i64\nRET",
                maximum_steps,
                false,
                error::division_by_zero,
            ),
            ("PUSH_CONST 0\nRET", 10, false, error::instruction_limit),
            (
                "PUSH_CONST_STR S0\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
                maximum_steps,
                true,
                error::service,
            ),
        ] {
            let helpers = helper("selected_body", "", "i64", body);
            let edition = source(
                "PUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\nNEW_ARRAY_LITERAL 1 text\nSTORE_LOCAL items\nPUSH_FN_PTR selected_body\nSTORE_LOCAL selected\nCALL selected 0\nRET",
                &helpers,
            );
            let mut sink = recorder {
                refuse,
                ..recorder::default()
            };
            let result = run(
                edition.as_bytes(),
                1,
                limits {
                    instructions: steps,
                    ..limits::default()
                },
                &mut sink,
            );
            assert_eq!(result.outcome, Err(expected));
            assert!(sink.output.is_empty());
            assert!(result.line > 0);
        }
    });
}
#[test]
fn last_function_table_entry_remains_addressable_without_pointer_conversion() {
    with_storage(|| {
        let mut helpers = String::new();
        for index in 0..63 {
            helpers.push_str(&helper(
                &format!("target_{index}"),
                "",
                "i64",
                &format!("PUSH_CONST {index}\nRET"),
            ));
        }
        let edition = source(
            "PUSH_FN_PTR target_62\nSTORE_LOCAL selected\nCALL selected 0\nRET",
            &helpers,
        );
        assert_eq!(
            run(
                edition.as_bytes(),
                0,
                limits::default(),
                &mut recorder::default()
            )
            .outcome,
            Ok(62)
        );
    });
}
#[cfg(feature = "host_conformance")]
#[test]
fn workspace_reuse_does_not_retain_function_identity_between_editions() {
    with_storage(|| {
        let editions = [
            source(
                "PUSH_FN_PTR target\nSTORE_LOCAL selected\nCALL selected 0\nRET",
                &helper("target", "", "i64", "PUSH_CONST 7\nRET"),
            ),
            source(
                "PUSH_FN_PTR target\nSTORE_LOCAL selected\nCALL selected 0\nRET",
                &helper("target", "", "i64", "PUSH_CONST 42\nRET"),
            ),
        ];
        let mut validation = Box::new(validation_workspace::new());
        let mut workspace = Box::new(execution_workspace::new());
        for (edition, expected) in editions.iter().zip([7, 42]) {
            let checked = anomalous_program_flow::conformance::validate_callable(
                edition.as_bytes(),
                0,
                &mut validation,
            )
            .unwrap();
            assert_eq!(checked.profile(), 6);
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
