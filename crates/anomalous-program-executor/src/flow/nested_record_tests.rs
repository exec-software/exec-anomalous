extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{nested_records, sha256, validation_workspace};
use anomalous_program_manifest::maximum_image_bytes;
use std::{boxed::Box, format, string::String, vec, vec::Vec};

const leaf_box: &str =
    "struct box<i64>\n child: leaf\nend struct\nstruct leaf\n x: i64\nend struct\n";
const node: &str = "struct node\n next: node\n value: i64\nend struct\n";

#[derive(Default)]
struct recorder {
    output: Vec<u8>,
    refuse: bool,
}

impl console_sink for recorder {
    fn write(&mut self, request: console_request<'_>) -> Result<(), service_error> {
        if self.refuse {
            return Err(service_error::refused);
        }
        self.output.extend_from_slice(request.bytes());
        Ok(())
    }
}

fn source(layouts: &str, helpers: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule nested_test\n# Struct definitions\n{layouts}\n{helpers}\n@ai_context(\"Exercise bounded shared nested records\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
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

fn assert_clear(workspace: &execution_workspace<'_>) {
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
            .all(|item| matches!(item, value::integer(0)))
    );
    assert!(
        workspace
            .frames
            .iter()
            .all(|frame| frame.locals.iter().all(Option::is_none))
    );
    assert!(workspace.globals.iter().all(Option::is_none));
    assert!(workspace.records.is_clear());
    assert!(workspace.arrays.is_clear());
    assert_eq!(workspace.text.used, 0);
    assert!(workspace.text.storage.iter().all(|byte| *byte == 0));
}

fn run(edition: &str, budget: limits, sink: &mut recorder) -> execution {
    let mut validation = Box::new(validation_workspace::new());
    let mut image = vec![0; maximum_image_bytes];
    let size = nested_records::encode(edition.as_bytes(), &mut image, &mut validation).unwrap();
    let admitted = nested_records::admit(&image[..size], 1, &mut validation).unwrap();
    assert_eq!(admitted.profile(), 12);
    let mut workspace = Box::new(execution_workspace::new());
    let result = execute(&admitted, budget, &mut workspace, sink);
    assert_eq!(result.edition_identity, sha256(edition.as_bytes()));
    assert_clear(&workspace);
    result
}

#[test]
fn shared_child_mutation_and_parent_reassignment_preserve_old_aliases() {
    with_storage(|| {
        let edition = source(
            leaf_box,
            "",
            "NEW_STRUCT leaf\nPUSH_CONST 7\nSET_FIELD x\nSTORE_LOCAL first\nNEW_STRUCT leaf\nPUSH_CONST 99\nSET_FIELD x\nSTORE_LOCAL second\nNEW_STRUCT box<i64>\nLOAD_LOCAL first\nSET_FIELD child\nSTORE_LOCAL parent\nNEW_STRUCT box<i64>\nLOAD_LOCAL first\nSET_FIELD child\nSTORE_LOCAL other\nLOAD_LOCAL parent\nGET_FIELD child\nSTORE_LOCAL alias\nLOAD_LOCAL alias\nPUSH_CONST 42\nSET_FIELD x\nPOP\nLOAD_LOCAL parent\nGET_FIELD child\nGET_FIELD x\nCONSOLE_WRITELN\nLOAD_LOCAL parent\nLOAD_LOCAL second\nSET_FIELD child\nPOP\nLOAD_LOCAL other\nGET_FIELD child\nGET_FIELD x\nCONSOLE_WRITELN\nLOAD_LOCAL alias\nGET_FIELD x\nCONSOLE_WRITELN\nLOAD_LOCAL parent\nGET_FIELD child\nGET_FIELD x\nRET",
        );
        let mut sink = recorder::default();
        assert_eq!(run(&edition, limits::default(), &mut sink).outcome, Ok(99));
        assert_eq!(sink.output, b"42\n42\n42\n");
    });
}

#[test]
fn nested_handles_survive_direct_indirect_helper_returns_and_caller_prefixes() {
    with_storage(|| {
        let helpers = helper(
            "make",
            "",
            "box<i64>",
            "NEW_STRUCT leaf\nPUSH_CONST 7\nSET_FIELD x\nSTORE_LOCAL child\nNEW_STRUCT box<i64>\nLOAD_LOCAL child\nSET_FIELD child\nRET",
        ) + &helper(
            "replace",
            "(parent: box<i64>, child: leaf)",
            "box<i64>",
            "STORE_LOCAL child\nSTORE_LOCAL parent\nLOAD_LOCAL parent\nLOAD_LOCAL child\nSET_FIELD child\nRET",
        );
        let edition = source(
            leaf_box,
            &helpers,
            "PUSH_CONST 100\nCALL make 0\nSTORE_LOCAL parent\nNEW_STRUCT leaf\nPUSH_CONST 42\nSET_FIELD x\nSTORE_LOCAL replacement\nPUSH_FN_PTR replace\nSTORE_LOCAL selected\nLOAD_LOCAL parent\nLOAD_LOCAL replacement\nCALL selected 2\nGET_FIELD child\nGET_FIELD x\nCONSOLE_WRITELN\nLOAD_LOCAL parent\nGET_FIELD child\nGET_FIELD x\nADD i64\nRET",
        );
        let mut sink = recorder::default();
        assert_eq!(run(&edition, limits::default(), &mut sink).outcome, Ok(142));
        assert_eq!(sink.output, b"42\n");
    });
}

#[test]
fn missing_nested_field_faults_without_exposing_an_older_integer_operand() {
    with_storage(|| {
        let edition = source(
            leaf_box,
            "",
            "PUSH_CONST 7\nCONSOLE_WRITELN\nPUSH_CONST 123\nNEW_STRUCT box<i64>\nGET_FIELD child\nGET_FIELD x\nADD i64\nRET",
        );
        let mut validation = Box::new(validation_workspace::new());
        let mut image = vec![0; maximum_image_bytes];
        let size = nested_records::encode(edition.as_bytes(), &mut image, &mut validation).unwrap();
        let admitted = nested_records::admit(&image[..size], 1, &mut validation).unwrap();
        let mut workspace = Box::new(execution_workspace::new());
        let mut sink = recorder::default();
        let result = workspace.run(
            admitted.program(),
            admitted.identity(),
            1,
            limits::default(),
            &mut sink,
        );
        assert_eq!(result, Err(error::record_uninitialized));
        assert_eq!(workspace.depth, 2);
        assert!(matches!(workspace.values[0], value::integer(123)));
        assert!(matches!(workspace.values[1], value::record(_)));
        assert_eq!(sink.output, b"7\n");
        let result = execute(
            &admitted,
            limits::default(),
            &mut workspace,
            &mut recorder::default(),
        );
        assert_eq!(result.outcome, Err(error::record_uninitialized));
        assert_eq!(result.statistics.instructions, 5);
        assert_eq!(
            result.line,
            edition
                .lines()
                .position(|line| line == "GET_FIELD child")
                .unwrap()
                + 1
        );
        assert_clear(&workspace);
    });
}

#[test]
fn self_and_mutual_cycles_preserve_aliases_without_recursive_traversal() {
    with_storage(|| {
        let self_cycle = source(
            node,
            "",
            "NEW_STRUCT node\nSTORE_LOCAL root\nLOAD_LOCAL root\nLOAD_LOCAL root\nSET_FIELD next\nPOP\nLOAD_LOCAL root\nGET_FIELD next\nGET_FIELD next\nPUSH_CONST 42\nSET_FIELD value\nPOP\nLOAD_LOCAL root\nGET_FIELD value\nRET",
        );
        assert_eq!(
            run(&self_cycle, limits::default(), &mut recorder::default()).outcome,
            Ok(42)
        );
        let mutual = source(
            "struct left\n next: right\n value: i64\nend struct\nstruct right\n next: left\nend struct",
            "",
            "NEW_STRUCT left\nSTORE_LOCAL first\nNEW_STRUCT right\nLOAD_LOCAL first\nSET_FIELD next\nSTORE_LOCAL second\nLOAD_LOCAL first\nLOAD_LOCAL second\nSET_FIELD next\nPOP\nLOAD_LOCAL first\nGET_FIELD next\nGET_FIELD next\nPUSH_CONST 99\nSET_FIELD value\nPOP\nLOAD_LOCAL first\nGET_FIELD value\nRET",
        );
        assert_eq!(
            run(&mutual, limits::default(), &mut recorder::default()).outcome,
            Ok(99)
        );
    });
}

#[test]
fn maximum_depth_is_bounded_by_handles_and_instruction_fuel() {
    with_storage(|| {
        let body = format!(
            "NEW_STRUCT node\nPUSH_CONST 42\nSET_FIELD value\nSTORE_LOCAL head\n{}LOAD_LOCAL head\n{}GET_FIELD value\nRET",
            "NEW_STRUCT node\nLOAD_LOCAL head\nSET_FIELD next\nSTORE_LOCAL head\n".repeat(127),
            "GET_FIELD next\n".repeat(127)
        );
        let edition = source(node, "", &body);
        assert_eq!(
            run(&edition, limits::default(), &mut recorder::default()).outcome,
            Ok(42)
        );
        let result = run(
            &edition,
            limits {
                instructions: 600,
                ..limits::default()
            },
            &mut recorder::default(),
        );
        assert_eq!(result.outcome, Err(error::instruction_limit));
        assert_eq!(result.statistics.instructions, 600);
    });
}

#[test]
fn nested_graph_allocation_still_counts_all_descriptors_and_field_cells() {
    with_storage(|| {
        for (fields, count, expected) in [
            (1, 128, Ok(7)),
            (1, 129, Err(error::record_limit)),
            (32, 32, Ok(7)),
            (32, 33, Err(error::record_limit)),
        ] {
            let mut layouts = String::from("struct node\n next: node\n");
            for index in 1..fields {
                layouts.push_str(&format!(" f{index}: i64\n"));
            }
            layouts.push_str("end struct\n");
            let body = format!("{}PUSH_CONST 7\nRET", "NEW_STRUCT node\nSTORE_LOCAL fresh\nLOAD_LOCAL fresh\nLOAD_LOCAL fresh\nSET_FIELD next\nPOP\n".repeat(count));
            assert_eq!(
                run(
                    &source(&layouts, "", &body),
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
fn field_kind_comes_from_exact_owner_even_when_names_and_generic_bases_match() {
    with_storage(|| {
        let layouts = "struct box<i64>\n value: i64\nend struct\nstruct box<text>\n value: box<i64>\nend struct";
        let edition = source(
            layouts,
            "",
            "NEW_STRUCT box<i64>\nPUSH_CONST 42\nSET_FIELD value\nSTORE_LOCAL child\nNEW_STRUCT box<text>\nLOAD_LOCAL child\nSET_FIELD value\nGET_FIELD value\nGET_FIELD value\nRET",
        );
        assert_eq!(
            run(&edition, limits::default(), &mut recorder::default()).outcome,
            Ok(42)
        );
    });
}

#[test]
fn unset_integer_fields_remain_zero_after_explicit_child_initialization() {
    with_storage(|| {
        let edition = source(
            leaf_box,
            "",
            "NEW_STRUCT leaf\nSTORE_LOCAL child\nNEW_STRUCT box<i64>\nLOAD_LOCAL child\nSET_FIELD child\nGET_FIELD child\nGET_FIELD x\nRET",
        );
        assert_eq!(
            run(&edition, limits::default(), &mut recorder::default()).outcome,
            Ok(0)
        );
    });
}

#[test]
fn cyclic_records_clear_on_service_frame_and_stack_failures() {
    with_storage(|| {
        let setup = "NEW_STRUCT node\nSTORE_LOCAL root\nLOAD_LOCAL root\nLOAD_LOCAL root\nSET_FIELD next\nPOP\n";
        let edition = source(
            node,
            "",
            &format!("{setup}PUSH_CONST 42\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET"),
        );
        assert_eq!(
            run(
                &edition,
                limits::default(),
                &mut recorder {
                    refuse: true,
                    ..recorder::default()
                }
            )
            .outcome,
            Err(error::service)
        );
        let recursive = helper(
            "recur",
            "(root: node)",
            "i64",
            "STORE_LOCAL root\nLOAD_LOCAL root\nGET_FIELD next\nCALL recur 1\nRET",
        );
        let edition = source(
            node,
            &recursive,
            &format!("{setup}LOAD_LOCAL root\nCALL recur 1\nRET"),
        );
        assert_eq!(
            run(&edition, limits::default(), &mut recorder::default()).outcome,
            Err(error::frame_limit)
        );
        let make = helper("make", "", "void", "NEW_STRUCT node\nPOP\nRET");
        let edition = source(
            node,
            &make,
            &format!(
                "{setup}{}CALL make 0\n{}PUSH_CONST 0\nRET",
                "PUSH_CONST 1\n".repeat(maximum_values),
                "POP\n".repeat(maximum_values)
            ),
        );
        assert_eq!(
            run(&edition, limits::default(), &mut recorder::default()).outcome,
            Err(error::stack_limit)
        );
    });
}

#[test]
fn repeated_execution_does_not_retain_cycles_or_child_initialization() {
    with_storage(|| {
        let edition = source(
            node,
            "",
            "NEW_STRUCT node\nSTORE_LOCAL root\nLOAD_LOCAL root\nLOAD_LOCAL root\nSET_FIELD next\nPOP\nLOAD_LOCAL root\nGET_FIELD next\nGET_FIELD value\nRET",
        );
        let mut validation = Box::new(validation_workspace::new());
        let mut image = vec![0; maximum_image_bytes];
        let size = nested_records::encode(edition.as_bytes(), &mut image, &mut validation).unwrap();
        let admitted = nested_records::admit(&image[..size], 0, &mut validation).unwrap();
        let mut workspace = Box::new(execution_workspace::new());
        for _ in 0..3 {
            assert_eq!(
                execute(
                    &admitted,
                    limits::default(),
                    &mut workspace,
                    &mut recorder::default()
                )
                .outcome,
                Ok(0)
            );
            assert_clear(&workspace);
        }
    });
}
