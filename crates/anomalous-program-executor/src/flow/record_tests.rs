extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{record, sha256, validation_workspace};
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
        "# NIL Textual Module\nmodule record_test\n# String constants\nconst S0 = \"hello \"\nconst S1 = \"world\"\n# Struct definitions\n{layouts}\n@ai_context(\"Exercise bounded shared records\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n{helpers}"
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
    let size = record::encode(edition, &mut image, &mut validation).unwrap();
    let admitted = record::admit(&image[..size], ceiling, &mut validation).unwrap();
    assert_eq!(admitted.profile(), 7);
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
fn shared_alias_mutation_keeps_defaults_and_all_i64_bits() {
    with_storage(|| {
        let edition = source(
            "NEW_STRUCT sample::point\nSTORE_LOCAL point\nLOAD_LOCAL point\nSTORE_LOCAL alias\nLOAD_LOCAL alias\nGET_FIELD y\nCONSOLE_WRITELN\nLOAD_LOCAL alias\nPUSH_CONST -9223372036854775808\nSET_FIELD x\nPUSH_CONST 9223372036854775807\nSET_FIELD y\nPOP\nLOAD_LOCAL point\nGET_FIELD x\nCONSOLE_WRITELN\nLOAD_LOCAL point\nGET_FIELD y\nRET",
            "",
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
            Ok(i64::MAX)
        );
        assert_eq!(sink.output, b"0\n-9223372036854775808\n");
    });
}

#[test]
fn independent_allocations_do_not_alias_and_field_order_is_layout_specific() {
    with_storage(|| {
        let edition = source_with_layouts(
            "NEW_STRUCT sample::point\nPUSH_CONST 7\nSET_FIELD x\nSTORE_LOCAL first\nNEW_STRUCT sample::point\nSTORE_LOCAL second\nNEW_STRUCT sample::reverse\nPUSH_CONST 42\nSET_FIELD x\nPUSH_CONST 9\nSET_FIELD y\nSTORE_LOCAL reverse\nLOAD_LOCAL first\nGET_FIELD x\nCONSOLE_WRITELN\nLOAD_LOCAL second\nGET_FIELD x\nCONSOLE_WRITELN\nLOAD_LOCAL reverse\nGET_FIELD y\nCONSOLE_WRITELN\nLOAD_LOCAL reverse\nGET_FIELD x\nRET",
            "",
            "struct sample::point\n x: i64\n y: i64\nend struct\nstruct sample::reverse\n y: i64\n x: i64\nend struct",
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
            Ok(42)
        );
        assert_eq!(sink.output, b"7\n0\n9\n");
    });
}

#[test]
fn direct_and_indirect_calls_share_mutation_and_return_owned_records() {
    with_storage(|| {
        let helpers = helper(
            "make",
            "",
            "sample::point",
            "NEW_STRUCT sample::point\nPUSH_CONST 7\nSET_FIELD x\nRET",
        ) + &helper(
            "mutate",
            "(point: sample::point, number: i64)",
            "sample::point",
            "STORE_LOCAL number\nSTORE_LOCAL point\nLOAD_LOCAL point\nLOAD_LOCAL number\nSET_FIELD x\nRET",
        );
        let edition = source(
            "PUSH_CONST 100\nCALL make 0\nSTORE_LOCAL point\nPUSH_FN_PTR mutate\nSTORE_LOCAL selected\nLOAD_LOCAL point\nPUSH_CONST 42\nCALL selected 2\nGET_FIELD x\nCONSOLE_WRITELN\nLOAD_LOCAL point\nGET_FIELD x\nADD i64\nRET",
            &helpers,
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
            Ok(142)
        );
        assert_eq!(sink.output, b"42\n");
    });
}

#[test]
fn record_allocations_are_cumulative_even_after_pop_and_callee_return() {
    with_storage(|| {
        for (fields, count, expected) in [
            (0, 128, Ok(7)),
            (0, 129, Err(error::record_limit)),
            (32, 32, Ok(7)),
            (32, 33, Err(error::record_limit)),
        ] {
            let mut layout = String::from("struct sample::point\n");
            for field in 0..fields {
                layout.push_str(&format!(" f{field}: i64\n"));
            }
            layout.push_str("end struct");
            let helpers = helper("make", "", "sample::point", "NEW_STRUCT sample::point\nRET");
            let body = format!(
                "PUSH_CONST 99\nCONSOLE_WRITELN\n{}PUSH_CONST 7\nRET",
                "CALL make 0\nPOP\n".repeat(count)
            );
            let edition = source_with_layouts(&body, &helpers, &layout);
            let mut sink = recorder::default();
            assert_eq!(
                run(edition.as_bytes(), 1, limits::default(), &mut sink).outcome,
                expected
            );
            assert_eq!(sink.output, b"99\n");
        }
    });
}

#[test]
fn record_fault_budget_and_service_paths_scrub_every_arena() {
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
                "PUSH_CONST 42\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
                limits::default(),
                true,
                error::service,
            ),
            (
                "PUSH_CONST 42\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
                limits {
                    output_bytes: 1,
                    ..limits::default()
                },
                false,
                error::output_limit,
            ),
        ] {
            let edition = source(
                &format!(
                    "NEW_STRUCT sample::point\nPUSH_CONST 99\nSET_FIELD x\nSTORE_LOCAL point\nPUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\nNEW_ARRAY_LITERAL 1 text\nSTORE_LOCAL items\nPUSH_FN_PTR target\nSTORE_LOCAL selected\n{tail}"
                ),
                &helper("target", "", "i64", "PUSH_CONST 0\nRET"),
            );
            let mut sink = recorder {
                refuse,
                ..recorder::default()
            };
            let result = run(edition.as_bytes(), 1, budget, &mut sink);
            assert_eq!(result.outcome, Err(expected));
            assert!(result.line > 0);
            assert!(sink.output.is_empty());
        }
    });
}

#[test]
fn record_creation_preflights_stack_capacity_with_a_preserved_caller_prefix() {
    with_storage(|| {
        let helpers = helper("make", "", "void", "NEW_STRUCT sample::point\nPOP\nRET");
        for existing in [0, maximum_records] {
            let body = format!(
                "{}{}CALL make 0\n{}PUSH_CONST 0\nRET",
                "NEW_STRUCT sample::point\nPOP\n".repeat(existing),
                "PUSH_CONST 1\n".repeat(maximum_values),
                "POP\n".repeat(maximum_values),
            );
            let edition = source(&body, &helpers);
            assert_eq!(
                run(
                    edition.as_bytes(),
                    0,
                    limits::default(),
                    &mut recorder::default()
                )
                .outcome,
                Err(error::stack_limit)
            );
        }
    });
}

#[test]
fn recursive_record_mutation_obeys_frame_limit_and_scrubs_on_return() {
    with_storage(|| {
        let helpers = helper(
            "recur",
            "(point: sample::point)",
            "i64",
            "STORE_LOCAL point\nLOAD_LOCAL point\nLOAD_LOCAL point\nGET_FIELD x\nPUSH_CONST 1\nADD i64\nSET_FIELD x\nSTORE_LOCAL point\nLOAD_LOCAL point\nCALL recur 1\nRET",
        );
        let edition = source("NEW_STRUCT sample::point\nCALL recur 1\nRET", &helpers);
        assert_eq!(
            run(
                edition.as_bytes(),
                0,
                limits::default(),
                &mut recorder::default()
            )
            .outcome,
            Err(error::frame_limit)
        );
    });
}

#[test]
fn last_nominal_type_and_global_field_id_remain_addressable() {
    with_storage(|| {
        let mut layouts = String::new();
        for index in 0..64 {
            layouts.push_str(&format!("struct record_{index}\n"));
            for field in 0..4 {
                layouts.push_str(&format!(" field_{}: i64\n", index * 4 + field));
            }
            layouts.push_str("end struct\n");
        }
        let edition = source_with_layouts(
            "NEW_STRUCT record_63\nPUSH_CONST 42\nSET_FIELD field_255\nGET_FIELD field_255\nRET",
            "",
            &layouts,
        );
        assert_eq!(
            run(
                edition.as_bytes(),
                0,
                limits::default(),
                &mut recorder::default()
            )
            .outcome,
            Ok(42)
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn reused_workspace_drops_nominal_layouts_aliases_and_mutation_between_editions() {
    with_storage(|| {
        let editions = [
            source(
                "NEW_STRUCT sample::point\nPUSH_CONST 42\nSET_FIELD x\nGET_FIELD x\nRET",
                "",
            ),
            source_with_layouts(
                "NEW_STRUCT other\nGET_FIELD y\nRET",
                "",
                "struct other\n y: i64\nend struct",
            ),
        ];
        let mut validation = Box::new(validation_workspace::new());
        let mut workspace = Box::new(execution_workspace::new());
        for (edition, expected) in editions.iter().zip([42, 0]) {
            let checked = anomalous_program_flow::conformance::validate_record(
                edition.as_bytes(),
                0,
                &mut validation,
            )
            .unwrap();
            assert_eq!(checked.profile(), 7);
            assert_eq!(
                crate::conformance::execute(
                    &checked,
                    limits::default(),
                    &mut workspace,
                    &mut recorder::default()
                )
                .outcome,
                Ok(expected)
            );
            assert_cleared(&workspace);
        }
    });
}
