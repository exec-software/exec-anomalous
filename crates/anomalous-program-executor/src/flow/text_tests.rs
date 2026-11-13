extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{sha256, text as text_profile, validation_workspace};
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

fn source(strings: &[&str], body: &str, helpers: &str) -> String {
    let mut result = String::from("# NIL Textual Module\nmodule text_test\n# String constants\n");
    for (index, value) in strings.iter().enumerate() {
        result.push_str(&format!("const S{index} = \"{value}\"\n"));
    }
    result.push_str(&format!("@ai_context(\"Exercise bounded computed text\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n{helpers}"));
    result
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
    let size = text_profile::encode(edition, &mut image, &mut validation).unwrap();
    let admitted = text_profile::admit(&image[..size], 1, &mut validation).unwrap();
    assert_eq!(admitted.profile(), 4);
    let mut workspace = Box::new(execution_workspace::new());
    let result = execute(&admitted, budget, &mut workspace, sink);
    assert_eq!(result.edition_identity, sha256(edition));
    assert!(
        sink.identities
            .iter()
            .all(|identity| *identity == result.edition_identity)
    );
    assert_cleared(&workspace);
    result
}

#[test]
fn returned_computed_text_and_aliases_survive_callee_teardown() {
    with_storage(|| {
        let edition = source(
            &["a", "b", "c"],
            "PUSH_CONST_STR S0\nPUSH_CONST_STR S1\nCALL join 2\nSTORE_LOCAL original\nLOAD_LOCAL original\nPUSH_CONST_STR S2\nCALL join 2\nSTORE_LOCAL successor\nLOAD_LOCAL original\nCONSOLE_WRITELN\nLOAD_LOCAL successor\nCONSOLE_WRITELN\nPUSH_CONST 0x2A\nRET",
            "# declaration_line 1\nfunction join(left: text, right: text) -> text:\nSTORE_LOCAL right\nSTORE_LOCAL left\nLOAD_LOCAL left\nLOAD_LOCAL right\nSTR_CONCAT\nRET\nend function\n",
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), limits::default(), &mut sink).outcome,
            Ok(42)
        );
        assert_eq!(sink.output, b"ab\nabc\n");
    });
}

#[test]
fn unicode_and_escape_boundaries_are_decoded_once() {
    with_storage(|| {
        let edition = source(
            &[r"\\", "n", r"\b", "é✓", r"\n", ""],
            "PUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\nSTORE_LOCAL slash_n\nLOAD_LOCAL slash_n\nPUSH_CONST_STR S2\nSTR_CONCAT\nPUSH_CONST_STR S3\nSTR_CONCAT\nPUSH_CONST_STR S4\nSTR_CONCAT\nPUSH_CONST_STR S5\nSTR_CONCAT\nCONSOLE_WRITE\nLOAD_LOCAL slash_n\nCONSOLE_WRITE\nPUSH_CONST 0\nRET",
            "",
        );
        let mut sink = recorder::default();
        assert_eq!(
            run(edition.as_bytes(), limits::default(), &mut sink).outcome,
            Ok(0)
        );
        assert_eq!(sink.output, "\\n\\bé✓\n\\n".as_bytes());
    });
}

#[test]
fn empty_computed_output_is_a_noop_except_for_requested_newline() {
    with_storage(|| {
        let edition = source(
            &[""],
            "PUSH_CONST_STR S0\nPUSH_CONST_STR S0\nSTR_CONCAT\nSTORE_LOCAL empty\nLOAD_LOCAL empty\nCONSOLE_WRITE\nLOAD_LOCAL empty\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
            "",
        );
        let mut sink = recorder::default();
        let result = run(edition.as_bytes(), limits::default(), &mut sink);
        assert_eq!(result.outcome, Ok(0));
        assert_eq!(result.statistics.console_calls, 1);
        assert_eq!(sink.output, b"\n");
    });
}

#[test]
fn full_arena_keeps_aliases_and_refuses_before_publishing_more_bytes() {
    with_storage(|| {
        let literal = "x".repeat(1024);
        let mut arena = Box::new(text_arena::new());
        let first = arena
            .concatenate(value::text(&literal), value::text(""))
            .unwrap();
        for _ in 1..64 {
            arena
                .concatenate(value::text(&literal), value::text(""))
                .unwrap();
        }
        assert_eq!(arena.used, maximum_text_bytes);
        assert_eq!(arena.bytes(first).unwrap(), literal.as_bytes());
        let empty = arena.concatenate(value::text(""), value::text("")).unwrap();
        assert!(arena.bytes(empty).unwrap().is_empty());
        let before = arena.storage.to_vec();
        assert_eq!(
            arena
                .concatenate(value::computed_text(first), value::text(""))
                .err(),
            Some(error::text_limit)
        );
        assert_eq!(arena.used, maximum_text_bytes);
        assert_eq!(arena.storage.as_slice(), before);
        arena.clear();
        assert_eq!(arena.used, 0);
        assert!(arena.storage.iter().all(|byte| *byte == 0));
        assert!(arena.bytes(first).is_err());
    });
}

#[test]
fn runtime_text_exhaustion_retains_prior_output_and_clears_every_byte() {
    with_storage(|| {
        let literal = "x".repeat(1024);
        let body = format!(
            "PUSH_CONST 7\nCONSOLE_WRITELN\n{}PUSH_CONST 0\nRET",
            "PUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\nPOP\n".repeat(65)
        );
        let edition = source(&[&literal, ""], &body, "");
        let mut sink = recorder::default();
        let result = run(edition.as_bytes(), limits::default(), &mut sink);
        assert_eq!(result.outcome, Err(error::text_limit));
        assert_eq!(result.statistics.instructions, 261);
        assert_eq!(result.statistics.output_bytes, 2);
        assert_eq!(sink.output, b"7\n");
    });
}

#[test]
fn computed_output_keeps_console_controls_and_payload_limits() {
    with_storage(|| {
        let large = "x".repeat(4096);
        for (left, right, newline, expected) in [
            ("bad", r"\r", "CONSOLE_WRITE", error::output_policy),
            (large.as_str(), "", "CONSOLE_WRITELN", error::output_limit),
        ] {
            let body = format!(
                "PUSH_CONST 7\nCONSOLE_WRITELN\nPUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\n{newline}\nPUSH_CONST 0\nRET"
            );
            let edition = source(&[left, right], &body, "");
            let mut sink = recorder::default();
            let result = run(edition.as_bytes(), limits::default(), &mut sink);
            assert_eq!(result.outcome, Err(expected));
            assert_eq!(result.statistics.console_calls, 1);
            assert_eq!(sink.output, b"7\n");
        }
    });
}

#[test]
fn computed_service_refusal_is_one_attempt_without_retry() {
    with_storage(|| {
        let edition = source(
            &["one", "two"],
            "PUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
            "",
        );
        let mut sink = recorder {
            refuse: true,
            ..recorder::default()
        };
        let result = run(edition.as_bytes(), limits::default(), &mut sink);
        assert_eq!(result.outcome, Err(error::service));
        assert_eq!(result.statistics.console_calls, 1);
        assert_eq!(result.statistics.output_bytes, 0);
        assert!(sink.output.is_empty());
    });
}

#[test]
fn runtime_fault_and_fuel_limits_clear_allocated_text() {
    with_storage(|| {
        for (tail, steps, expected) in [
            (
                "PUSH_CONST 1\nPUSH_CONST 0\nDIV i64\nRET",
                maximum_steps,
                error::division_by_zero,
            ),
            ("PUSH_CONST 0\nRET", 3, error::instruction_limit),
        ] {
            let edition = source(
                &["a", "b"],
                &format!("PUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\nPOP\n{tail}"),
                "",
            );
            let result = run(
                edition.as_bytes(),
                limits {
                    instructions: steps,
                    ..limits::default()
                },
                &mut recorder::default(),
            );
            assert_eq!(result.outcome, Err(expected));
        }
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_text_path_reuses_cleared_storage_between_distinct_editions() {
    with_storage(|| {
        let mut validation = Box::new(validation_workspace::new());
        let mut workspace = Box::new(execution_workspace::new());
        let editions = [
            source(
                &["one", "two"],
                "PUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
                "",
            ),
            source(
                &["three", "four"],
                "PUSH_CONST_STR S0\nPUSH_CONST_STR S1\nSTR_CONCAT\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
                "",
            ),
        ];
        for (edition, expected) in editions
            .iter()
            .zip([b"onetwo\n".as_slice(), b"threefour\n"])
        {
            let checked = anomalous_program_flow::conformance::validate_text(
                edition.as_bytes(),
                1,
                &mut validation,
            )
            .unwrap();
            assert_eq!(checked.profile(), 4);
            let mut sink = recorder::default();
            let result =
                crate::conformance::execute(&checked, limits::default(), &mut workspace, &mut sink);
            assert_eq!(result.outcome, Ok(0));
            assert_eq!(sink.output, expected);
            assert_cleared(&workspace);
        }
    });
}
