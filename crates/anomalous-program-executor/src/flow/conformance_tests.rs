extern crate std;

use super::*;
use crate::{conformance, console_request, service_error};
use anomalous_program_flow::{conformance::validate_scalar, scalar, sha256, validation_workspace};
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

fn source(body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn assert_cleared(workspace: &execution_workspace<'_>) {
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
fn raw_console_uses_exact_edition_identity_without_manufactured_intent() {
    with_storage(|| {
        let edition = source(
            "PUSH_CONST 41\nSTORE_LOCAL $ny.temp.value.0\nLOAD_LOCAL $ny.temp.value.0\nCONSOLE_WRITELN\nLOAD_LOCAL $ny.temp.value.0\nPUSH_CONST 1\nADD\nRET",
        );
        let mut validation = Box::new(validation_workspace::new());
        let checked = validate_scalar(edition.as_bytes(), 1, &mut validation).unwrap();
        assert_eq!(checked.intent(), None);
        assert_eq!(checked.edition(), edition.as_bytes());
        let mut workspace = Box::new(execution_workspace::new());
        let mut sink = recorder::default();
        let result = conformance::execute(&checked, limits::default(), &mut workspace, &mut sink);
        assert_eq!(result.outcome, Ok(42));
        assert_eq!(result.edition_identity, sha256(edition.as_bytes()));
        assert_eq!(sink.identities, [result.edition_identity]);
        assert_eq!(sink.output, b"41\n");
        assert_eq!(result.statistics.instructions, 8);
        assert_cleared(&workspace);
    });
}

#[test]
fn raw_fault_keeps_prior_output_and_clears_reusable_execution_storage() {
    with_storage(|| {
        let edition =
            source("PUSH_CONST 7\nCONSOLE_WRITELN\nPUSH_CONST 1\nPUSH_CONST 0\nDIV i64\nRET");
        let mut validation = Box::new(validation_workspace::new());
        let checked = validate_scalar(edition.as_bytes(), 1, &mut validation).unwrap();
        let mut workspace = Box::new(execution_workspace::new());
        let mut sink = recorder::default();
        let result = conformance::execute(&checked, limits::default(), &mut workspace, &mut sink);
        assert_eq!(result.outcome, Err(error::division_by_zero));
        assert_eq!(result.statistics.instructions, 5);
        assert_eq!(result.statistics.output_bytes, 2);
        assert_eq!(sink.output, b"7\n");
        assert_cleared(&workspace);
        let result = conformance::execute(
            &checked,
            limits {
                instructions: 0,
                ..limits::default()
            },
            &mut workspace,
            &mut sink,
        );
        assert_eq!(result.outcome, Err(error::instruction_limit));
        assert_eq!(result.statistics, statistics::default());
        assert_eq!(sink.output, b"7\n");
        assert_cleared(&workspace);
    });
}

#[test]
fn raw_service_refusal_is_one_attempt_without_acknowledged_bytes() {
    with_storage(|| {
        let edition = source("PUSH_CONST 7\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET");
        let mut validation = Box::new(validation_workspace::new());
        let checked = validate_scalar(edition.as_bytes(), 1, &mut validation).unwrap();
        let mut workspace = Box::new(execution_workspace::new());
        let mut sink = recorder {
            refuse: true,
            ..recorder::default()
        };
        let result = conformance::execute(&checked, limits::default(), &mut workspace, &mut sink);
        assert_eq!(result.outcome, Err(error::service));
        assert_eq!(result.statistics.console_calls, 1);
        assert_eq!(result.statistics.output_bytes, 0);
        assert_eq!(sink.identities.len(), 1);
        assert!(sink.output.is_empty());
        assert_cleared(&workspace);
    });
}

#[test]
fn complete_compiler_editions_have_identical_raw_and_production_execution() {
    with_storage(|| {
        let editions: [&[u8]; 8] = [
            include_bytes!("../../fixtures/scalar-flow/zero_ranges.nil"),
            include_bytes!("../../fixtures/scalar-flow/nested_ranges.nil"),
            include_bytes!("../../fixtures/scalar-flow/break_continue.nil"),
            include_bytes!("../../fixtures/scalar-flow/bounds_once.nil"),
            include_bytes!("../../fixtures/scalar-flow/short_circuit.nil"),
            include_bytes!("../../fixtures/scalar-flow/scratch_joins.nil"),
            include_bytes!("../../fixtures/scalar-flow/range_widths.nil"),
            include_bytes!("../../fixtures/scalar-flow/fuel.nil"),
        ];
        for edition in editions {
            let mut validation = Box::new(validation_workspace::new());
            let mut workspace = Box::new(execution_workspace::new());
            let mut image = vec![0; maximum_image_bytes];
            let checked = validate_scalar(edition, 1, &mut validation).unwrap();
            assert!(checked.intent().is_some());
            let mut raw_sink = recorder::default();
            let raw =
                conformance::execute(&checked, limits::default(), &mut workspace, &mut raw_sink);
            assert_cleared(&workspace);
            let length = scalar::encode(edition, &mut image, &mut validation).unwrap();
            let admitted = scalar::admit(&image[..length], 1, &mut validation).unwrap();
            let mut admitted_sink = recorder::default();
            let production = execute(
                &admitted,
                limits::default(),
                &mut workspace,
                &mut admitted_sink,
            );
            assert_eq!(raw, production);
            assert_eq!(raw_sink.output, admitted_sink.output);
            assert_eq!(raw_sink.identities, admitted_sink.identities);
            assert_cleared(&workspace);
        }
    });
}
