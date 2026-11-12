extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{scalar, validation_workspace};
use anomalous_program_manifest::maximum_image_bytes;
use std::{boxed::Box, format, string::String, vec, vec::Vec};

#[derive(Default)]
struct recorder {
    output: Vec<u8>,
    requests: usize,
    refuse: bool,
}

impl console_sink for recorder {
    fn write(&mut self, request: console_request<'_>) -> Result<(), service_error> {
        self.requests += 1;
        assert_eq!(request.sequence(), self.requests);
        if self.refuse {
            return Err(service_error::refused);
        }
        self.output.extend_from_slice(request.bytes());
        Ok(())
    }
}

fn source(main: &str, helpers: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n@ai_context(\"Exercise bounded compiler scalar forms\")\n# declaration_line 1\nfunction main -> i64:\n{main}\nend function\n{helpers}"
    )
}

fn run(edition: &[u8], budget: limits, sink: &mut recorder) -> execution {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn_scoped(scope, move || {
                let mut validation = Box::new(validation_workspace::new());
                let mut image = vec![0; maximum_image_bytes];
                let length = scalar::encode(edition, &mut image, &mut validation).unwrap();
                let admitted = scalar::admit(&image[..length], 1, &mut validation).unwrap();
                assert_eq!(admitted.profile(), scalar::derivation_profile);
                let mut workspace = Box::new(execution_workspace::new());
                let result = execute(&admitted, budget, &mut workspace, sink);
                assert_eq!(result.edition_identity, admitted.identity());
                assert_eq!(
                    (workspace.depth, workspace.frame_count, workspace.pc),
                    (0, 0, 0)
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
                result
            })
            .unwrap()
            .join()
            .unwrap()
    })
}

#[test]
fn operandless_integer_add_wraps_at_i64_without_implicit_i32_narrowing() {
    for (left, right, expected) in [
        (i64::MAX, 1, i64::MIN),
        (i64::MIN, -1, i64::MAX),
        (4294967296, 1, 4294967297),
    ] {
        let edition = source(
            &format!("PUSH_CONST {left}\nPUSH_CONST {right}\nADD\nRET"),
            "",
        );
        let result = run(
            edition.as_bytes(),
            limits::default(),
            &mut recorder::default(),
        );
        assert_eq!(result.outcome, Ok(expected));
        assert_eq!(result.statistics.instructions, 4);
    }
}

#[test]
fn generated_locals_stay_in_their_frame_and_preserve_the_caller_prefix() {
    let edition = source(
        "PUSH_CONST 100\nPUSH_CONST 2\nSTORE_LOCAL $ny.temp.range_idx.0\nLOAD_LOCAL $ny.temp.range_idx.0\nCALL helper 1\nLOAD_LOCAL $ny.temp.range_idx.0\nADD\nADD\nRET",
        "# declaration_line 1\nfunction helper(value: i64) -> i64:\nSTORE_LOCAL value\nPUSH_CONST 5\nSTORE_LOCAL $ny.temp.range_idx.0\nLOAD_LOCAL value\nLOAD_LOCAL $ny.temp.range_idx.0\nADD\nRET\nend function\n",
    );
    let result = run(
        edition.as_bytes(),
        limits::default(),
        &mut recorder::default(),
    );
    assert_eq!(result.outcome, Ok(109));
    assert_eq!(result.statistics.instructions, 16);
}

#[test]
fn generated_local_loop_stops_at_fuel_and_keeps_acknowledged_output() {
    let edition = source(
        "PUSH_CONST 7\nCONSOLE_WRITELN\nPUSH_CONST 0\nSTORE_LOCAL $ny.temp.range_idx.0\nLABEL again\nLOAD_LOCAL $ny.temp.range_idx.0\nPUSH_CONST 1\nADD\nSTORE_LOCAL $ny.temp.range_idx.0\nJMP again",
        "",
    );
    let mut sink = recorder::default();
    let result = run(
        edition.as_bytes(),
        limits {
            instructions: 10000,
            ..limits::default()
        },
        &mut sink,
    );
    assert_eq!(result.outcome, Err(error::instruction_limit));
    assert_eq!(result.statistics.instructions, 10000);
    assert_eq!(result.statistics.console_calls, 1);
    assert_eq!(result.statistics.output_bytes, 2);
    assert_eq!(sink.output, b"7\n");
}

#[test]
fn scalar_service_refusal_counts_once_and_clears_the_machine() {
    let edition = source(
        "PUSH_CONST 7\nSTORE_LOCAL $ny.temp.logic_and.0\nLOAD_LOCAL $ny.temp.logic_and.0\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
        "",
    );
    let mut sink = recorder {
        refuse: true,
        ..recorder::default()
    };
    let result = run(edition.as_bytes(), limits::default(), &mut sink);
    assert_eq!(result.outcome, Err(error::service));
    assert_eq!(result.statistics.instructions, 4);
    assert_eq!(result.statistics.console_calls, 1);
    assert_eq!(result.statistics.output_bytes, 0);
    assert_eq!(sink.requests, 1);
    assert!(sink.output.is_empty());
}

#[test]
fn complete_compiler_ranges_and_short_circuit_outputs_match_reference() {
    let cases: &[(&[u8], i64, &[u8])] = &[
        (
            include_bytes!("../../fixtures/scalar-flow/zero_ranges.nil"),
            7,
            b"",
        ),
        (
            include_bytes!("../../fixtures/scalar-flow/nested_ranges.nil"),
            189,
            b"",
        ),
        (
            include_bytes!("../../fixtures/scalar-flow/break_continue.nil"),
            13,
            b"0\n1\n3\n4\n5\n",
        ),
        (
            include_bytes!("../../fixtures/scalar-flow/bounds_once.nil"),
            9,
            b"2\n5\n",
        ),
        (
            include_bytes!("../../fixtures/scalar-flow/short_circuit.nil"),
            15,
            b"33\n44\n",
        ),
        (
            include_bytes!("../../fixtures/scalar-flow/scratch_joins.nil"),
            430,
            b"",
        ),
        (
            include_bytes!("../../fixtures/scalar-flow/range_widths.nil"),
            -3,
            b"2147483647\n-2147483648\n-2\n",
        ),
    ];
    for &(edition, expected, output) in cases {
        let mut sink = recorder::default();
        let result = run(edition, limits::default(), &mut sink);
        assert_eq!(result.outcome, Ok(expected));
        assert_eq!(sink.output, output);
        assert_eq!(result.statistics.output_bytes, output.len());
    }
}

#[test]
fn finite_compiler_range_explicitly_refuses_when_runtime_fuel_is_exhausted() {
    let result = run(
        include_bytes!("../../fixtures/scalar-flow/fuel.nil"),
        limits::default(),
        &mut recorder::default(),
    );
    assert_eq!(result.outcome, Err(error::instruction_limit));
    assert_eq!(result.statistics.instructions, maximum_steps);
    assert_eq!(result.statistics.console_calls, 0);
    assert_eq!(result.statistics.output_bytes, 0);
}
