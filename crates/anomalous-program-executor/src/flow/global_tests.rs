extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{globals, sha256, validation_workspace};
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
    source_with_layouts(
        body,
        helpers,
        "struct sample::point\n    x: i64\n    y: i64\nend struct",
    )
}
fn source_with_layouts(body: &str, helpers: &str, layouts: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule global_test\n# String constants\nconst S0 = \"hello \"\nconst S1 = \"world\"\n# Struct definitions\n{layouts}\n@ai_context(\"Exercise bounded module globals\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n{helpers}"
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
    assert!(workspace.records.is_clear());
    assert!(workspace.globals.iter().all(Option::is_none));
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
    let size = globals::encode(edition, &mut image, &mut validation).unwrap();
    let admitted = globals::admit(&image[..size], ceiling, &mut validation).unwrap();
    assert_eq!(admitted.profile(), 8);
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
fn initializer_is_an_ordinary_call_and_runs_each_time_requested() {
    with_storage(|| {
        let helpers = helper(
            "__globals_init__",
            "",
            "void",
            "PUSH_CONST 42\nSTORE_GLOBAL module::answer i64\nRET",
        );
        for (body, expected) in [
            (
                "LOAD_GLOBAL module::answer i64\nRET",
                Err(error::global_uninitialized),
            ),
            (
                "CALL __globals_init__ 0\nLOAD_GLOBAL module::answer i64\nRET",
                Ok(42),
            ),
            (
                "CALL __globals_init__ 0\nPUSH_CONST 7\nSTORE_GLOBAL module::answer i64\nCALL __globals_init__ 0\nLOAD_GLOBAL module::answer i64\nRET",
                Ok(42),
            ),
        ] {
            let edition = source(body, &helpers);
            assert_eq!(
                run(
                    edition.as_bytes(),
                    0,
                    limits::default(),
                    &mut recorder::default()
                )
                .outcome,
                expected
            );
        }
    });
}

#[test]
fn globals_are_shared_across_direct_and_indirect_calls_with_separate_namespaces() {
    with_storage(|| {
        let helpers = helper(
            "update",
            "(counter: i64)",
            "i64",
            "STORE_LOCAL counter\nLOAD_LOCAL counter\nSTORE_GLOBAL counter i64\nLOAD_GLOBAL counter i64\nRET",
        );
        let edition = source(
            "PUSH_CONST 100\nSTORE_LOCAL counter\nPUSH_CONST 7\nCALL update 1\nCONSOLE_WRITELN\nPUSH_FN_PTR update\nSTORE_LOCAL selected\nPUSH_CONST 0\nSTORE_GLOBAL selected i64\nPUSH_CONST 0\nSTORE_GLOBAL update i64\nPUSH_CONST 42\nCALL selected 1\nCONSOLE_WRITELN\nLOAD_LOCAL counter\nLOAD_GLOBAL counter i64\nADD i64\nRET",
            &helpers,
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
            Ok(142)
        );
        assert_eq!(sink.output, b"7\n42\n");
    });
}

#[test]
fn global_slots_keep_full_integer_values_and_last_slot_identity() {
    with_storage(|| {
        let mut body = String::new();
        for index in 0..maximum_global_slots {
            body.push_str(&format!(
                "PUSH_CONST {index}\nSTORE_GLOBAL module::slot_{index} i64\n"
            ));
        }
        body.push_str("LOAD_GLOBAL module::slot_127 i64\nCONSOLE_WRITELN\nPUSH_CONST -9223372036854775808\nSTORE_GLOBAL module::slot_0 i64\nPUSH_CONST 9223372036854775807\nSTORE_GLOBAL module::slot_127 i64\nLOAD_GLOBAL module::slot_0 i64\nCONSOLE_WRITELN\nLOAD_GLOBAL module::slot_127 i64\nRET");
        let edition = source(&body, "");
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
            Ok(i64::MAX)
        );
        assert_eq!(sink.output, b"127\n-9223372036854775808\n");
    });
}

#[test]
fn missing_global_preserves_prior_output_and_clears_all_owned_storage() {
    with_storage(|| {
        let edition = source(
            "PUSH_CONST 7\nSTORE_GLOBAL initialized i64\nLOAD_GLOBAL initialized i64\nCAST text\nCONSOLE_WRITELN\nPUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\nNEW_ARRAY_LITERAL 1 text\nSTORE_LOCAL items\nNEW_STRUCT sample::point\nPUSH_CONST 99\nSET_FIELD x\nSTORE_LOCAL point\nLOAD_GLOBAL never_written i64\nRET",
            "",
        );
        let mut sink = recorder::default();
        let result = run(edition.as_bytes(), 1, limits::default(), &mut sink);
        assert_eq!(result.outcome, Err(error::global_uninitialized));
        assert_eq!(sink.output, b"7\n");
        assert_eq!(result.statistics.console_calls, 1);
        assert!(result.line > 0);
    });
}

#[test]
fn integer_text_is_exact_decimal_and_survives_helpers_arrays_and_aliases() {
    with_storage(|| {
        let helpers = helper(
            "render",
            "(number: i64)",
            "text",
            "STORE_LOCAL number\nLOAD_LOCAL number\nCAST text\nRET",
        );
        for number in [
            i64::MIN,
            -9007199254740993,
            -1,
            0,
            1,
            4294967295,
            9007199254740993,
            i64::MAX,
        ] {
            let edition = source(
                &format!(
                    "PUSH_CONST {number}\nCALL render 1\nSTORE_LOCAL held\nLOAD_LOCAL held\nNEW_ARRAY_LITERAL 1 text\nSTORE_LOCAL items\nPUSH_CONST_STR S0\nLOAD_LOCAL held\nSTR_CONCAT\nCONSOLE_WRITELN\nLOAD_LOCAL items\nPUSH_CONST 0\nGET_INDEX\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET"
                ),
                &helpers,
            );
            let mut sink = recorder::default();
            assert_eq!(
                run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
                Ok(0)
            );
            assert_eq!(
                sink.output,
                format!("hello {number}\n{number}\n").as_bytes()
            );
        }
    });
}

#[test]
fn integer_formatting_shares_cumulative_text_capacity_and_keeps_live_aliases() {
    with_storage(|| {
        for overflow in [false, true] {
            let tail = if overflow {
                "PUSH_CONST 0\nCAST text\nPOP\n"
            } else {
                ""
            };
            // 3276 twenty-byte strings plus one sixteen-byte string exactly
            // fill the existing 65536-byte arena. Dropping handles reclaims none.
            let edition = source(
                &format!(
                    "PUSH_CONST -9223372036854775808\nCAST text\nSTORE_LOCAL held\nPUSH_CONST 0\nSTORE_GLOBAL index i64\nLABEL loop\nLOAD_GLOBAL index i64\nPUSH_CONST 3275\nCMP_LT\nJMP_IF_NOT done\nPUSH_CONST -9223372036854775808\nCAST text\nPOP\nLOAD_GLOBAL index i64\nPUSH_CONST 1\nADD i64\nSTORE_GLOBAL index i64\nJMP loop\nLABEL done\nPUSH_CONST 1234567890123456\nCAST text\nPOP\nLOAD_LOCAL held\nCONSOLE_WRITELN\n{tail}PUSH_CONST 0\nRET"
                ),
                "",
            );
            let mut sink = recorder::default();
            assert_eq!(
                run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
                if overflow {
                    Err(error::text_limit)
                } else {
                    Ok(0)
                }
            );
            assert_eq!(sink.output, b"-9223372036854775808\n");
        }
    });
}

#[test]
fn global_and_formatted_text_storage_clear_after_each_failure_class() {
    with_storage(|| {
        for (tail, budget, refuse, expected) in [
            (
                "PUSH_CONST 1\nPUSH_CONST 0\nDIV i64\nRET",
                limits::default(),
                false,
                error::division_by_zero,
            ),
            (
                "LABEL forever\nJMP forever",
                limits {
                    instructions: 40,
                    ..limits::default()
                },
                false,
                error::instruction_limit,
            ),
            (
                "LOAD_GLOBAL shared i64\nCAST text\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
                limits::default(),
                true,
                error::service,
            ),
            (
                "LOAD_GLOBAL shared i64\nCAST text\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
                limits {
                    output_bytes: 1,
                    ..limits::default()
                },
                false,
                error::output_limit,
            ),
            (
                "CALL recurse 0\nRET",
                limits::default(),
                false,
                error::frame_limit,
            ),
        ] {
            let edition = source(
                &format!(
                    "PUSH_CONST 42\nSTORE_GLOBAL shared i64\nLOAD_GLOBAL shared i64\nCAST text\nSTORE_LOCAL held\n{tail}"
                ),
                &helper("recurse", "", "i64", "CALL recurse 0\nRET"),
            );
            let mut sink = recorder {
                refuse,
                ..recorder::default()
            };
            assert_eq!(
                run(edition.as_bytes(), 1, budget, &mut sink).outcome,
                Err(expected)
            );
            assert!(sink.output.is_empty());
        }
    });
}

#[test]
fn failed_decimal_allocation_leaves_committed_text_and_cursor_unchanged() {
    let mut arena = text_arena::new();
    arena.used = maximum_text_bytes - 19;
    arena.storage[..arena.used].fill(b'x');
    let before = arena.storage;
    assert!(matches!(arena.integer(i64::MIN), Err(error::text_limit)));
    assert_eq!(arena.used, maximum_text_bytes - 19);
    assert_eq!(arena.storage, before);
    let span = arena.integer(i64::MAX).unwrap();
    assert_eq!(arena.bytes(span).unwrap(), b"9223372036854775807");
    assert_eq!(arena.used, maximum_text_bytes);
}

#[cfg(feature = "host_conformance")]
#[test]
fn workspace_reuse_resets_global_values_initialization_and_name_bindings() {
    with_storage(|| {
        let editions = [
            source(
                "PUSH_CONST 42\nSTORE_GLOBAL previous i64\nPUSH_CONST 0\nRET",
                "",
            ),
            source("LOAD_GLOBAL previous i64\nRET", ""),
            source(
                "PUSH_CONST 99\nSTORE_GLOBAL other i64\nPUSH_CONST 1\nPUSH_CONST 0\nDIV i64\nRET",
                "",
            ),
            source("LOAD_GLOBAL different_name i64\nRET", ""),
        ];
        let mut validation = Box::new(validation_workspace::new());
        let mut workspace = Box::new(execution_workspace::new());
        for (edition, expected) in editions.iter().zip([
            Ok(0),
            Err(error::global_uninitialized),
            Err(error::division_by_zero),
            Err(error::global_uninitialized),
        ]) {
            let checked = anomalous_program_flow::conformance::validate_globals(
                edition.as_bytes(),
                0,
                &mut validation,
            )
            .unwrap();
            assert_eq!(checked.profile(), 8);
            assert_eq!(
                crate::conformance::execute(
                    &checked,
                    limits::default(),
                    &mut workspace,
                    &mut recorder::default()
                )
                .outcome,
                expected
            );
            assert_cleared(&workspace);
        }
    });
}
