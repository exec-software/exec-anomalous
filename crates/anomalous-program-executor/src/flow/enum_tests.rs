extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{enums, sha256, validation_workspace};
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
        "enum sample::state\n    Idle\n    Busy: i64\n    Done: i64\nend enum",
    )
}
fn source_with_layouts(body: &str, helpers: &str, layouts: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule enum_test\n# Enum definitions\n{layouts}\n@ai_context(\"Exercise bounded nominal enum values\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n{helpers}"
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
fn run(edition: &[u8], budget: limits, sink: &mut recorder) -> execution {
    let mut validation = Box::new(validation_workspace::new());
    let mut image = vec![0; maximum_image_bytes];
    let size = enums::encode(edition, &mut image, &mut validation).unwrap();
    let admitted = enums::admit(&image[..size], 1, &mut validation).unwrap();
    assert_eq!(admitted.profile(), 9);
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
fn unit_and_integer_tag_tests_consume_owners_and_keep_nominal_tags() {
    with_storage(|| {
        let edition = source(
            "NEW_VARIANT sample::state::Idle 0 sample::state void\nIS_VARIANT sample::state::Idle sample::state void\nCONSOLE_WRITELN\nPUSH_CONST 42\nNEW_VARIANT sample::state::Busy 1 sample::state i64\nSTORE_LOCAL state\nLOAD_LOCAL state\nIS_VARIANT sample::state::Idle sample::state void\nCONSOLE_WRITELN\nLOAD_LOCAL state\nIS_VARIANT sample::state::Done sample::state i64\nCONSOLE_WRITELN\nLOAD_LOCAL state\nIS_VARIANT sample::state::Busy sample::state i64\nRET",
            "",
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), limits::default(), &mut sink).outcome,
            Ok(1)
        );
        assert_eq!(sink.output, b"1\n0\n0\n");
    });
}

#[test]
fn extraction_preserves_every_signed_integer_bit_and_alias() {
    with_storage(|| {
        for number in [
            i64::MIN,
            i64::MAX,
            9_007_199_254_740_993,
            4_294_967_297,
            -4_294_967_297,
            0,
        ] {
            let edition = source(
                &format!(
                    "PUSH_CONST {number}\nNEW_VARIANT sample::state::Busy 1 sample::state i64\nSTORE_LOCAL state\nLOAD_LOCAL state\nUNWRAP_VARIANT 1 sample::state i64\nCONSOLE_WRITELN\nLOAD_LOCAL state\nUNWRAP_VARIANT 1 sample::state i64\nRET"
                ),
                "",
            );
            let mut sink = recorder::default();
            assert_eq!(
                run(edition.as_bytes(), limits::default(), &mut sink).outcome,
                Ok(number)
            );
            assert_eq!(sink.output, format!("{number}\n").as_bytes());
        }
    });
}

#[test]
fn direct_and_indirect_helper_calls_preserve_enum_arguments_and_results() {
    with_storage(|| {
        let helpers = helper(
            "identity",
            "(state: sample::state)",
            "sample::state",
            "STORE_LOCAL state\nLOAD_LOCAL state\nRET",
        );
        let edition = source(
            "PUSH_CONST -9223372036854775808\nNEW_VARIANT sample::state::Busy 1 sample::state i64\nCALL identity 1\nSTORE_LOCAL state\nPUSH_FN_PTR identity\nSTORE_LOCAL selected\nLOAD_LOCAL state\nCALL selected 1\nUNWRAP_VARIANT 1 sample::state i64\nRET",
            &helpers,
        );
        assert_eq!(
            run(
                edition.as_bytes(),
                limits::default(),
                &mut recorder::default()
            )
            .outcome,
            Ok(i64::MIN)
        );
    });
}

#[test]
fn same_enum_branches_merge_both_stack_and_local_variants() {
    with_storage(|| {
        for condition in [0, 1] {
            let edition = source(
                &format!(
                    "PUSH_CONST {condition}\nJMP_IF_NOT idle\nPUSH_CONST 42\nNEW_VARIANT sample::state::Busy 1 sample::state i64\nJMP joined\nLABEL idle\nNEW_VARIANT sample::state::Idle 0 sample::state void\nLABEL joined\nSTORE_LOCAL state\nLOAD_LOCAL state\nIS_VARIANT sample::state::Busy sample::state i64\nJMP_IF_NOT unit\nLOAD_LOCAL state\nUNWRAP_VARIANT 1 sample::state i64\nRET\nLABEL unit\nLOAD_LOCAL state\nUNWRAP_VARIANT 0 sample::state void\nPUSH_CONST 7\nRET"
                ),
                "",
            );
            assert_eq!(
                run(
                    edition.as_bytes(),
                    limits::default(),
                    &mut recorder::default()
                )
                .outcome,
                Ok(if condition == 0 { 7 } else { 42 })
            );
        }
    });
}

#[test]
fn enum_values_are_inline_and_repeated_construction_has_no_arena_limit() {
    with_storage(|| {
        let edition = source(
            "PUSH_CONST 0\nSTORE_LOCAL i\nNEW_VARIANT sample::state::Idle 0 sample::state void\nSTORE_LOCAL state\nLABEL again\nLOAD_LOCAL i\nNEW_VARIANT sample::state::Busy 1 sample::state i64\nSTORE_LOCAL state\nLOAD_LOCAL i\nPUSH_CONST 1\nADD i64\nSTORE_LOCAL i\nLOAD_LOCAL i\nPUSH_CONST 2048\nCMP_LT\nJMP_IF again\nLOAD_LOCAL state\nUNWRAP_VARIANT 1 sample::state i64\nRET",
            "",
        );
        assert_eq!(
            run(
                edition.as_bytes(),
                limits::default(),
                &mut recorder::default()
            )
            .outcome,
            Ok(2047)
        );
    });
}

#[test]
fn exact_extraction_shape_refuses_both_missing_and_discarded_payloads() {
    with_storage(|| {
        for body in [
            "PUSH_CONST 99\nNEW_VARIANT sample::state::Idle 0 sample::state void\nUNWRAP_VARIANT 1 sample::state i64\nADD i64\nRET",
            "PUSH_CONST 99\nPUSH_CONST 42\nNEW_VARIANT sample::state::Busy 1 sample::state i64\nUNWRAP_VARIANT 0 sample::state void\nRET",
        ] {
            let edition = source(body, "");
            let result = run(
                edition.as_bytes(),
                limits::default(),
                &mut recorder::default(),
            );
            assert_eq!(result.outcome, Err(error::enum_shape));
            assert_eq!(result.statistics.output_bytes, 0);
        }
        assert_eq!(error::enum_shape.as_str(), "enum_shape");
    });
}

#[test]
fn missing_helper_payload_cannot_read_through_its_caller_floor() {
    with_storage(|| {
        let helpers = helper(
            "extract",
            "(state: sample::state)",
            "i64",
            "STORE_LOCAL state\nLOAD_LOCAL state\nUNWRAP_VARIANT 1 sample::state i64\nRET",
        );
        let edition = source(
            "PUSH_CONST 99\nNEW_VARIANT sample::state::Idle 0 sample::state void\nCALL extract 1\nADD i64\nRET",
            &helpers,
        );
        assert_eq!(
            run(
                edition.as_bytes(),
                limits::default(),
                &mut recorder::default()
            )
            .outcome,
            Err(error::enum_shape)
        );
    });
}

#[test]
fn shape_fault_preserves_acknowledged_output_and_retires_all_storage() {
    with_storage(|| {
        let edition = source(
            "PUSH_CONST 7\nCAST text\nSTORE_LOCAL text\nPUSH_CONST 8\nNEW_ARRAY_LITERAL 1 i64\nSTORE_LOCAL array\nNEW_STRUCT sample\nPUSH_CONST 9\nSET_FIELD value\nSTORE_LOCAL record\nPUSH_CONST 10\nSTORE_GLOBAL held i64\nPUSH_CONST 41\nCONSOLE_WRITELN\nNEW_VARIANT sample::state::Idle 0 sample::state void\nSTORE_LOCAL state\nLOAD_LOCAL state\nUNWRAP_VARIANT 1 sample::state i64\nRET",
            "",
        ).replace("module enum_test\n", "module enum_test\n# Struct definitions\nstruct sample\nvalue: i64\nend struct\n");
        let mut sink = recorder::default();
        let result = run(edition.as_bytes(), limits::default(), &mut sink);
        assert_eq!(result.outcome, Err(error::enum_shape));
        assert_eq!(sink.output, b"41\n");
        assert_eq!(result.statistics.console_calls, 1);
        assert_eq!(result.statistics.output_bytes, 3);
    });
}

#[test]
fn runtime_rechecks_private_enum_identity_tag_and_payload_shape() {
    with_storage(|| {
        let edition = source_with_layouts(
            "PUSH_CONST 0\nRET",
            "",
            "enum a\nUnit\nValue: i64\nend enum\nenum b\nUnit\nend enum",
        );
        let mut validation = Box::new(validation_workspace::new());
        let mut image = vec![0; maximum_image_bytes];
        let size = enums::encode(edition.as_bytes(), &mut image, &mut validation).unwrap();
        let admitted = enums::admit(&image[..size], 0, &mut validation).unwrap();
        let program = admitted.program();
        for (layout, variant, payload) in
            [(2, 0, None), (0, 2, None), (0, 0, Some(1)), (0, 1, None)]
        {
            assert!(matches!(
                enum_value::new(program, layout, variant, payload),
                Err(error::invalid_state)
            ));
        }
        let item = enum_value::new(program, 0, 1, Some(i64::MIN)).unwrap();
        assert_eq!(item.matches(program, 0, 0), Ok(false));
        assert_eq!(item.matches(program, 0, 1), Ok(true));
        assert_eq!(item.matches(program, 1, 0), Err(error::invalid_state));
        assert_eq!(item.matches(program, 0, 2), Err(error::invalid_state));
        assert_eq!(item.unwrap(program, 1, true), Err(error::invalid_state));
        assert_eq!(item.unwrap(program, 0, false), Err(error::enum_shape));
        assert_eq!(item.unwrap(program, 0, true), Ok(Some(i64::MIN)));
    });
}

#[test]
fn highest_layout_and_variant_ids_remain_distinct() {
    with_storage(|| {
        let mut layouts = String::new();
        for index in 0..64 {
            layouts.push_str(&format!("enum state_{index}\n"));
            for variant in 0..if index == 63 { 32 } else { 1 } {
                layouts.push_str(&format!("Value_{variant}: i64\n"));
            }
            layouts.push_str("end enum\n");
        }
        let edition = source_with_layouts(
            "PUSH_CONST 9223372036854775807\nNEW_VARIANT state_63::Value_31 1 state_63 i64\nSTORE_LOCAL state\nLOAD_LOCAL state\nIS_VARIANT state_63::Value_0 state_63 i64\nCONSOLE_WRITELN\nLOAD_LOCAL state\nUNWRAP_VARIANT 1 state_63 i64\nRET",
            "",
            &layouts,
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), limits::default(), &mut sink).outcome,
            Ok(i64::MAX)
        );
        assert_eq!(sink.output, b"0\n");
    });
}

#[test]
fn global_operand_capacity_still_limits_unit_construction_in_a_helper() {
    with_storage(|| {
        let helpers = helper(
            "construct",
            "",
            "void",
            "NEW_VARIANT sample::state::Idle 0 sample::state void\nPOP\nRET",
        );
        let body = format!(
            "{}CALL construct 0\n{}PUSH_CONST 0\nRET",
            "PUSH_CONST 1\n".repeat(maximum_values),
            "POP\n".repeat(maximum_values)
        );
        let edition = source(&body, &helpers);
        assert_eq!(
            run(
                edition.as_bytes(),
                limits::default(),
                &mut recorder::default()
            )
            .outcome,
            Err(error::stack_limit)
        );
    });
}

#[test]
fn limits_service_faults_and_workspace_reuse_clear_enum_operands_and_locals() {
    with_storage(|| {
        let edition = source(
            "PUSH_CONST 42\nNEW_VARIANT sample::state::Busy 1 sample::state i64\nSTORE_LOCAL state\nLOAD_LOCAL state\nUNWRAP_VARIANT 1 sample::state i64\nCONSOLE_WRITELN\nLOAD_LOCAL state\nUNWRAP_VARIANT 1 sample::state i64\nRET",
            "",
        );
        let mut validation = Box::new(validation_workspace::new());
        let mut image = vec![0; maximum_image_bytes];
        let size = enums::encode(edition.as_bytes(), &mut image, &mut validation).unwrap();
        let admitted = enums::admit(&image[..size], 1, &mut validation).unwrap();
        let mut workspace = Box::new(execution_workspace::new());
        for (budget, refuse, expected) in [
            (
                limits {
                    instructions: maximum_steps + 1,
                    ..limits::default()
                },
                false,
                Err(error::invalid_limits),
            ),
            (
                limits {
                    frames: 0,
                    ..limits::default()
                },
                false,
                Err(error::frame_limit),
            ),
            (
                limits {
                    instructions: 4,
                    ..limits::default()
                },
                false,
                Err(error::instruction_limit),
            ),
            (
                limits {
                    output_bytes: 2,
                    ..limits::default()
                },
                false,
                Err(error::output_limit),
            ),
            (limits::default(), true, Err(error::service)),
            (limits::default(), false, Ok(42)),
        ] {
            // Entry clearing also retires stale caller-owned workspace contents.
            workspace.values[0] =
                value::enumeration(enum_value::new(admitted.program(), 0, 1, Some(99)).unwrap());
            workspace.frames[0].locals[0] = Some(workspace.values[0]);
            let mut sink = recorder {
                refuse,
                ..recorder::default()
            };
            let result = execute(&admitted, budget, &mut workspace, &mut sink);
            assert_eq!(result.outcome, expected);
            assert_cleared(&workspace);
        }
    });
}
