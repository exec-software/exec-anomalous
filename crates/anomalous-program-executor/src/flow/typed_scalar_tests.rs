extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{sha256, typed_scalars, validation_workspace};
use anomalous_program_manifest::maximum_image_bytes;
use std::{boxed::Box, format, string::String, vec, vec::Vec};

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

fn source(body: &str, helpers: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule typed_scalar_test\n{helpers}@ai_context(\"Exercise bounded explicit scalar values\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn helper(name: &str, ty: &str) -> String {
    format!(
        "# declaration_line 1\nfunction {name}(item: {ty}) -> {ty}:\nSTORE_LOCAL item\nLOAD_LOCAL item\nCAST {ty}\nRET\nend function\n"
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
    assert_eq!(workspace.text.used, 0);
    assert!(workspace.text.storage.iter().all(|byte| *byte == 0));
    assert!(workspace.arrays.is_clear());
    assert!(workspace.records.is_clear());
}

fn run(edition: &str, budget: limits, sink: &mut recorder) -> execution {
    let mut validation = Box::new(validation_workspace::new());
    let mut image = vec![0; maximum_image_bytes];
    let size = typed_scalars::encode(edition.as_bytes(), &mut image, &mut validation).unwrap();
    let admitted = typed_scalars::admit(&image[..size], 1, &mut validation).unwrap();
    assert_eq!(admitted.profile(), 11);
    let mut workspace = Box::new(execution_workspace::new());
    let result = execute(&admitted, budget, &mut workspace, sink);
    assert_eq!(result.edition_identity, sha256(edition.as_bytes()));
    assert_clear(&workspace);
    result
}

#[test]
fn bool_cast_accepts_exactly_zero_and_one_without_truthiness() {
    with_storage(|| {
        for number in [i64::MIN, -256, -1, 0, 1, 2, 255, 256, i64::MAX] {
            let edition = source(
                &format!("PUSH_CONST {number}\nCAST bool\nCAST i64\nRET"),
                "",
            );
            let result = run(&edition, limits::default(), &mut recorder::default());
            let expected = if matches!(number, 0 | 1) {
                Ok(number)
            } else {
                Err(error::scalar_conversion)
            };
            assert_eq!(result.outcome, expected, "{number}");
            assert_eq!(
                result.statistics.instructions,
                if expected.is_ok() { 4 } else { 2 }
            );
        }
        for (number, expected) in [
            (256, Ok(0)),
            (257, Ok(1)),
            (-255, Ok(1)),
            (-1, Err(error::scalar_conversion)),
        ] {
            let edition = source(
                &format!("PUSH_CONST {number}\nCAST u8\nCAST bool\nCAST bool\nCAST i64\nRET"),
                "",
            );
            assert_eq!(
                run(&edition, limits::default(), &mut recorder::default()).outcome,
                expected
            );
        }
    });
}

#[test]
fn byte_cast_retains_low_bits_and_explicit_integer_widths() {
    with_storage(|| {
        for (number, expected) in [
            (i64::MIN, 0),
            (-257, 255),
            (-256, 0),
            (-1, 255),
            (0, 0),
            (255, 255),
            (256, 0),
            (257, 1),
            (i64::MAX, 255),
        ] {
            let edition = source(
                &format!("PUSH_CONST {number}\nCAST u8\nCAST u8\nCAST i32\nCAST i64\nRET"),
                "",
            );
            assert_eq!(
                run(&edition, limits::default(), &mut recorder::default()).outcome,
                Ok(expected)
            );
        }
        for number in [0, 1] {
            let edition = source(
                &format!("PUSH_CONST {number}\nCAST bool\nCAST u8\nCAST i32\nRET"),
                "",
            );
            assert_eq!(
                run(&edition, limits::default(), &mut recorder::default()).outcome,
                Ok(number)
            );
        }
        let edition = source("PUSH_CONST 4294967295\nCAST i32\nCAST i64\nRET", "");
        assert_eq!(
            run(&edition, limits::default(), &mut recorder::default()).outcome,
            Ok(-1)
        );
    });
}

#[test]
fn boolean_and_byte_helpers_preserve_types_arguments_and_caller_prefix() {
    with_storage(|| {
        for (ty, number) in [("bool", 1), ("u8", 255)] {
            let edition = source(
                &format!(
                    "PUSH_CONST 1000\nPUSH_CONST {number}\nCAST {ty}\nCALL echo 1\nSTORE_LOCAL copied\nPUSH_FN_PTR echo\nSTORE_LOCAL selected\nLOAD_LOCAL copied\nCALL selected 1\nCAST i64\nADD i64\nRET"
                ),
                &helper("echo", ty),
            );
            assert_eq!(
                run(&edition, limits::default(), &mut recorder::default()).outcome,
                Ok(1000 + number)
            );
        }
    });
}

#[test]
fn finite_float_bits_survive_locals_direct_indirect_calls_and_identity_casts() {
    with_storage(|| {
        for (literal, expected) in [
            ("0.0", 0),
            ("-0.0", 0x8000_0000_0000_0000),
            ("0.1", 0x3fb9_9999_9999_999a),
            ("0.5", 0x3fe0_0000_0000_0000),
            ("5e-324", 1),
            ("-5e-324", 0x8000_0000_0000_0001),
            ("2.2250738585072014e-308", 0x0010_0000_0000_0000),
            ("1.7976931348623157e308", 0x7fef_ffff_ffff_ffff),
        ] {
            let edition = source(
                &format!(
                    "PUSH_CONST {literal}\nSTORE_LOCAL original\nLOAD_LOCAL original\nCALL echo 1\nSTORE_LOCAL direct\nPUSH_FN_PTR echo\nSTORE_LOCAL selected\nLOAD_LOCAL direct\nCALL selected 1\nCAST f64\nSTORE_LOCAL indirect\nLOAD_LOCAL indirect\nPOP\nPUSH_CONST 0\nRET"
                ),
                &helper("echo", "f64"),
            );
            let mut validation = Box::new(validation_workspace::new());
            let mut image = vec![0; maximum_image_bytes];
            let size =
                typed_scalars::encode(edition.as_bytes(), &mut image, &mut validation).unwrap();
            let admitted = typed_scalars::admit(&image[..size], 0, &mut validation).unwrap();
            let mut workspace = Box::new(execution_workspace::new());
            let stopped = workspace.run(
                admitted.program(),
                admitted.identity(),
                0,
                limits {
                    instructions: 20,
                    ..limits::default()
                },
                &mut recorder::default(),
            );
            assert_eq!(stopped, Err(error::instruction_limit));
            assert_eq!(workspace.depth, 1);
            assert!(
                matches!(workspace.values[0], value::float(bits) if bits == expected),
                "{literal}"
            );
            let frame = workspace.current().unwrap();
            let copies: Vec<_> = frame
                .locals
                .iter()
                .filter_map(|item| match item {
                    Some(value::float(bits)) => Some(*bits),
                    _ => None,
                })
                .collect();
            assert_eq!(copies, [expected; 3]);
            // The public entry must clear even this retained test-only state on
            // entry, then scrub all new state after a successful run.
            let result = execute(
                &admitted,
                limits::default(),
                &mut workspace,
                &mut recorder::default(),
            );
            assert_eq!(result.outcome, Ok(0));
            assert_eq!(result.statistics.instructions, 23);
            assert_clear(&workspace);
        }
    });
}

#[test]
fn conversion_fault_preserves_prior_output_and_clears_typed_values() {
    with_storage(|| {
        let edition = source(
            "PUSH_CONST -0.0\nSTORE_LOCAL float\nPUSH_CONST 255\nCAST u8\nSTORE_LOCAL byte\nPUSH_CONST 7\nCONSOLE_WRITELN\nLOAD_LOCAL byte\nCAST bool\nPOP\nPUSH_CONST 0\nRET",
            "",
        );
        let mut sink = recorder::default();
        let result = run(&edition, limits::default(), &mut sink);
        assert_eq!(result.outcome, Err(error::scalar_conversion));
        assert_eq!(result.statistics.instructions, 9);
        assert_eq!(result.statistics.console_calls, 1);
        assert_eq!(sink.output, b"7\n");
        assert_eq!(
            result.line,
            edition
                .lines()
                .position(|line| line == "CAST bool")
                .unwrap()
                + 1
        );
    });
}

#[test]
fn typed_values_are_cleared_on_fuel_frame_and_sink_refusals() {
    with_storage(|| {
        let edition = source(
            "PUSH_CONST -0.0\nCALL echo 1\nSTORE_LOCAL float\nPUSH_CONST 1\nCAST bool\nSTORE_LOCAL truth\nPUSH_CONST 42\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
            &helper("echo", "f64"),
        );
        for (budget, refuse, expected) in [
            (
                limits {
                    instructions: 1,
                    ..limits::default()
                },
                false,
                error::instruction_limit,
            ),
            (
                limits {
                    frames: 1,
                    ..limits::default()
                },
                false,
                error::frame_limit,
            ),
            (limits::default(), true, error::service),
        ] {
            assert_eq!(
                run(
                    &edition,
                    budget,
                    &mut recorder {
                        refuse,
                        ..recorder::default()
                    }
                )
                .outcome,
                Err(expected)
            );
        }
    });
}

#[test]
fn private_cast_defends_invalid_kinds_and_nonfinite_bits() {
    use anomalous_program_flow::scalar_cast;
    for bits in [
        0x7ff0_0000_0000_0000,
        0xfff0_0000_0000_0000,
        0x7ff8_0000_0000_0001,
    ] {
        assert!(matches!(
            typed_scalar::cast(value::float(bits), scalar_cast::float_identity),
            Err(error::invalid_state)
        ));
    }
    assert!(matches!(
        typed_scalar::cast(value::integer(1), scalar_cast::float_identity),
        Err(error::invalid_state)
    ));
    assert!(matches!(
        typed_scalar::cast(value::float(0), scalar_cast::integer64),
        Err(error::invalid_state)
    ));
    assert!(matches!(
        typed_scalar::cast(value::text("1"), scalar_cast::boolean),
        Err(error::invalid_state)
    ));
}
