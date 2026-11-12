extern crate std;

use super::{console_request, console_sink, error, execute, execution, limits, service_error};
use anomalous_program_manifest::{
    admit, authority, encode, encoded_length, error as admission_error, sha256,
};
use std::{format, string::String, vec, vec::Vec};

#[derive(Debug)]
struct recorded_request {
    identity: [u8; 32],
    sequence: usize,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct recorder {
    requests: Vec<recorded_request>,
    published: Vec<u8>,
    refuse_sequence: Option<usize>,
    publish_on_refusal: usize,
}

impl console_sink for recorder {
    fn write(&mut self, request: console_request<'_>) -> Result<(), service_error> {
        self.requests.push(recorded_request {
            identity: request.edition_identity(),
            sequence: request.sequence(),
            bytes: request.bytes().to_vec(),
        });
        if self.refuse_sequence == Some(request.sequence()) {
            let length = self.publish_on_refusal.min(request.bytes().len());
            self.published.extend_from_slice(&request.bytes()[..length]);
            return Err(service_error::refused);
        }
        self.published.extend_from_slice(request.bytes());
        Ok(())
    }
}

fn edition(constants: &[(&str, &str)], body: &str) -> String {
    let mut source = String::from("# NIL Textual Module\nmodule root\n");
    if !constants.is_empty() {
        source.push_str("# String constants\n");
        for (name, escaped) in constants {
            source.push_str(&format!("const {name} = \"{escaped}\"\n"));
        }
    }
    source.push_str("@ai_context(\"Exercise bounded execution\")\n# declaration_line 1\nfunction main -> i64:\n");
    source.push_str(body);
    source.push_str("\nend function\n");
    source
}

fn package(source: &[u8]) -> Vec<u8> {
    let mut image = vec![0; encoded_length(source).unwrap()];
    assert_eq!(encode(source, &mut image).unwrap(), image.len());
    image
}

fn run(source: &str, ceiling: u64, budget: limits, sink: &mut recorder) -> execution {
    let image = package(source.as_bytes());
    let admitted = admit(&image, ceiling).unwrap();
    execute(&admitted, budget, sink)
}

#[test]
fn complete_compiler_editions_return_their_results_and_authored_output() {
    let cases: &[(&[u8], i64, &[u8], usize)] = &[
        (
            include_bytes!("../../anomalous-program-manifest/fixtures/native/arithmetic.nil"),
            42,
            b"",
            15,
        ),
        (
            include_bytes!("../../anomalous-program-manifest/fixtures/native/console.nil"),
            0,
            b"Anomalous manifest: hello\n",
            5,
        ),
        (
            include_bytes!("../../anomalous-program-manifest/fixtures/native/large.nil"),
            79,
            b"",
            244,
        ),
    ];
    for &(source, result, output, instructions) in cases {
        let image = package(source);
        let admitted = admit(&image, authority::console_write).unwrap();
        let mut sink = recorder::default();
        let observed = execute(&admitted, limits::default(), &mut sink);
        assert_eq!(observed.outcome, Ok(result));
        assert_eq!(observed.edition_identity, sha256(source));
        assert_eq!(observed.statistics.instructions, instructions);
        assert_eq!(
            observed.statistics.console_calls,
            usize::from(!output.is_empty())
        );
        assert_eq!(observed.statistics.output_bytes, output.len());
        assert_eq!(sink.published, output);
    }
}

#[test]
fn denied_admission_never_reaches_the_sink() {
    let source = edition(&[], "PUSH_CONST 7\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET");
    let mut image = package(source.as_bytes());
    let mut sink = recorder::default();
    let rejected = match admit(&image, 0) {
        Ok(witness) => {
            execute(&witness, limits::default(), &mut sink);
            panic!("console edition admitted without its ceiling");
        }
        Err(failure) => failure,
    };
    assert_eq!(rejected, admission_error::ceiling);
    let last = image.len() - 1;
    image[last] = b' ';
    assert_eq!(
        admit(&image, authority::console_write).unwrap_err(),
        admission_error::identity
    );
    assert!(sink.requests.is_empty());
    assert!(sink.published.is_empty());
}

#[test]
fn unused_bodies_require_authority_but_never_execute_or_consume_steps() {
    let unused = "# declaration_line 1\nfunction unused -> i64:\nPUSH_CONST 99\nCONSOLE_WRITELN\nPUSH_CONST 1\nPUSH_CONST 0\nDIV i64\nRET\nend function\n";
    for before in [false, true] {
        let main = edition(&[], "PUSH_CONST 42\nRET");
        let source = if before {
            main.replacen("@ai_context", &format!("{unused}@ai_context"), 1)
        } else {
            main + unused
        };
        let image = package(source.as_bytes());
        assert_eq!(admit(&image, 0).unwrap_err(), admission_error::ceiling);
        let witness = admit(&image, authority::console_write).unwrap();
        assert_eq!(witness.granted_services(), authority::console_write);
        let mut sink = recorder::default();
        let result = execute(
            &witness,
            limits {
                instructions: 2,
                output_bytes: 0,
            },
            &mut sink,
        );
        assert_eq!(result.outcome, Ok(42));
        assert_eq!(result.statistics.instructions, 2);
        assert_eq!(result.statistics.console_calls, 0);
        assert!(sink.requests.is_empty());
    }
}

#[test]
fn sink_requests_bind_exact_identity_and_sequence_without_empty_write_gaps() {
    let source = edition(
        &[("empty", ""), ("word", "hello")],
        "PUSH_CONST_STR empty\nCONSOLE_WRITE\nPUSH_CONST_STR word\nCONSOLE_WRITE\nPUSH_CONST_STR empty\nCONSOLE_WRITELN\nPUSH_CONST -17\nCALL println 1\nPUSH_CONST 0\nRET",
    );
    let mut sink = recorder::default();
    let result = run(
        &source,
        authority::console_write,
        limits::default(),
        &mut sink,
    );
    assert_eq!(result.outcome, Ok(0));
    assert_eq!(result.statistics.console_calls, 3);
    assert_eq!(result.statistics.output_bytes, 10);
    assert_eq!(sink.published, b"hello\n-17\n");
    for (index, expected) in [b"hello".as_slice(), b"\n", b"-17\n"].iter().enumerate() {
        assert_eq!(sink.requests[index].identity, sha256(source.as_bytes()));
        assert_eq!(sink.requests[index].sequence, index + 1);
        assert_eq!(sink.requests[index].bytes, *expected);
    }
}

#[test]
fn service_refusal_counts_attempts_and_acknowledgements_without_retry() {
    let source = edition(
        &[],
        "PUSH_CONST 1\nCONSOLE_WRITELN\nPUSH_CONST 2\nCONSOLE_WRITELN\nPUSH_CONST 3\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
    );
    let mut sink = recorder {
        refuse_sequence: Some(2),
        ..recorder::default()
    };
    let result = run(
        &source,
        authority::console_write,
        limits::default(),
        &mut sink,
    );
    assert_eq!(result.outcome, Err(error::service));
    assert_eq!(result.statistics.instructions, 4);
    assert_eq!(result.statistics.console_calls, 2);
    assert_eq!(result.statistics.output_bytes, 2);
    assert_eq!(sink.requests.len(), 2);
    assert_eq!(sink.requests[1].bytes, b"2\n");
    assert_eq!(sink.published, b"1\n");
}

#[test]
fn refusing_sink_effects_are_not_misreported_as_acknowledged_output() {
    let source = edition(
        &[("text", "partial")],
        "PUSH_CONST_STR text\nCONSOLE_WRITE\nPUSH_CONST 0\nRET",
    );
    let mut sink = recorder {
        refuse_sequence: Some(1),
        publish_on_refusal: 3,
        ..recorder::default()
    };
    let result = run(
        &source,
        authority::console_write,
        limits::default(),
        &mut sink,
    );
    assert_eq!(result.outcome, Err(error::service));
    assert_eq!(result.statistics.console_calls, 1);
    assert_eq!(result.statistics.output_bytes, 0);
    assert_eq!(sink.requests.len(), 1);
    assert_eq!(sink.published, b"par");
}

#[test]
fn control_at_end_of_formatted_output_refuses_the_whole_instruction() {
    for suffix in ["\\r", "\\v", "\\f", "\u{7f}", "\u{85}"] {
        let escaped = format!("{}{suffix}", "a".repeat(4000));
        let source = edition(
            &[("text", &escaped)],
            "PUSH_CONST_STR text\nCONSOLE_WRITE\nPUSH_CONST 0\nRET",
        );
        // Literal controls are rejected by admission itself; escape spellings
        // reach execution and exercise complete output policy preflight.
        if suffix.starts_with('\\') {
            let mut sink = recorder::default();
            let result = run(
                &source,
                authority::console_write,
                limits::default(),
                &mut sink,
            );
            assert_eq!(result.outcome, Err(error::output_policy));
            assert_eq!(result.statistics.instructions, 2);
            assert_eq!(result.statistics.console_calls, 0);
            assert_eq!(result.statistics.output_bytes, 0);
            assert!(sink.requests.is_empty());
        } else {
            assert!(encoded_length(source.as_bytes()).is_err());
        }
    }
}

#[test]
fn per_instruction_length_includes_utf8_bytes_and_appended_newline() {
    for (text, operation, expected) in [
        ("a".repeat(4096), "CONSOLE_WRITE", Ok(0)),
        (
            "a".repeat(4096),
            "CONSOLE_WRITELN",
            Err(error::output_limit),
        ),
        ("a".repeat(4097), "CONSOLE_WRITE", Err(error::output_limit)),
        (
            format!("{}é", "a".repeat(4095)),
            "CONSOLE_WRITE",
            Err(error::output_limit),
        ),
    ] {
        let source = edition(
            &[("text", &text)],
            &format!("PUSH_CONST_STR text\n{operation}\nPUSH_CONST 0\nRET"),
        );
        let mut sink = recorder::default();
        let result = run(
            &source,
            authority::console_write,
            limits::default(),
            &mut sink,
        );
        assert_eq!(result.outcome, expected);
        if expected.is_ok() {
            assert_eq!(sink.published.len(), 4096);
            assert_eq!(result.statistics.output_bytes, 4096);
        } else {
            assert!(sink.requests.is_empty());
            assert!(sink.published.is_empty());
            assert_eq!(result.statistics.console_calls, 0);
            assert_eq!(result.statistics.output_bytes, 0);
        }
    }
}

#[test]
fn total_output_limit_accepts_exact_boundary_then_preserves_earlier_output() {
    let text = "x".repeat(4096);
    for (writes, expected) in [(16, Ok(0)), (17, Err(error::output_limit))] {
        let body = "PUSH_CONST_STR text\nCONSOLE_WRITE\n".repeat(writes) + "PUSH_CONST 0\nRET";
        let source = edition(&[("text", &text)], &body);
        let mut sink = recorder::default();
        let result = run(
            &source,
            authority::console_write,
            limits::default(),
            &mut sink,
        );
        assert_eq!(result.outcome, expected);
        assert_eq!(result.statistics.console_calls, 16);
        assert_eq!(result.statistics.output_bytes, 65536);
        assert_eq!(sink.requests.len(), 16);
        assert_eq!(sink.published, vec![b'x'; 65536]);
    }
}

#[test]
fn smaller_output_budget_rejects_a_whole_later_payload() {
    let source = edition(
        &[],
        "PUSH_CONST 1\nCONSOLE_WRITELN\nPUSH_CONST 2\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET",
    );
    let mut sink = recorder::default();
    let result = run(
        &source,
        authority::console_write,
        limits {
            instructions: 4096,
            output_bytes: 3,
        },
        &mut sink,
    );
    assert_eq!(result.outcome, Err(error::output_limit));
    assert_eq!(result.statistics.console_calls, 1);
    assert_eq!(result.statistics.output_bytes, 2);
    assert_eq!(sink.published, b"1\n");
}

#[test]
fn zero_limits_are_restrictive_without_becoming_invalid_limits() {
    let pure = edition(&[], "PUSH_CONST 0\nRET");
    let mut sink = recorder::default();
    let result = run(
        &pure,
        0,
        limits {
            instructions: 0,
            output_bytes: 0,
        },
        &mut sink,
    );
    assert_eq!(result.outcome, Err(error::instruction_limit));
    assert_eq!(result.statistics.instructions, 0);
    assert!(sink.requests.is_empty());
    assert_eq!(
        run(
            &pure,
            0,
            limits {
                instructions: 2,
                output_bytes: 0
            },
            &mut sink
        )
        .outcome,
        Ok(0)
    );

    let empty = edition(
        &[("text", "")],
        "PUSH_CONST_STR text\nCONSOLE_WRITE\nPUSH_CONST 0\nRET",
    );
    let result = run(
        &empty,
        authority::console_write,
        limits {
            instructions: 4,
            output_bytes: 0,
        },
        &mut sink,
    );
    assert_eq!(result.outcome, Ok(0));
    assert_eq!(result.statistics.console_calls, 0);
    let newline = empty.replace("CONSOLE_WRITE\n", "CONSOLE_WRITELN\n");
    assert_eq!(
        run(
            &newline,
            authority::console_write,
            limits {
                instructions: 4,
                output_bytes: 0
            },
            &mut sink
        )
        .outcome,
        Err(error::output_limit)
    );
    assert!(sink.requests.is_empty());
}

#[test]
fn callers_cannot_raise_hard_limits() {
    let source = edition(&[], "PUSH_CONST 1\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET");
    for budget in [
        limits {
            instructions: 4097,
            output_bytes: 65536,
        },
        limits {
            instructions: 4096,
            output_bytes: 65537,
        },
        limits {
            instructions: usize::MAX,
            output_bytes: usize::MAX,
        },
    ] {
        let mut sink = recorder::default();
        let result = run(&source, authority::console_write, budget, &mut sink);
        assert_eq!(result.outcome, Err(error::invalid_limits));
        assert_eq!(result.statistics.instructions, 0);
        assert_eq!(result.statistics.console_calls, 0);
        assert_eq!(result.statistics.output_bytes, 0);
        assert_eq!(result.line, 0);
        assert!(sink.requests.is_empty());
    }
}

#[test]
fn step_exhaustion_happens_before_dispatch_and_keeps_acknowledged_output() {
    let source = edition(&[], "PUSH_CONST 9\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET");
    let mut sink = recorder::default();
    let result = run(
        &source,
        authority::console_write,
        limits {
            instructions: 2,
            output_bytes: 65536,
        },
        &mut sink,
    );
    assert_eq!(result.outcome, Err(error::instruction_limit));
    assert_eq!(result.statistics.instructions, 2);
    assert_eq!(result.statistics.console_calls, 1);
    assert_eq!(result.statistics.output_bytes, 2);
    assert_eq!(sink.published, b"9\n");
    assert_eq!(source.lines().nth(result.line - 1), Some("PUSH_CONST 0"));
}

#[test]
fn full_instruction_budget_is_usable_without_off_by_one() {
    let source = edition(&[], &("NOP\n".repeat(4094) + "PUSH_CONST 42\nRET"));
    let mut sink = recorder::default();
    let result = run(&source, 0, limits::default(), &mut sink);
    assert_eq!(result.outcome, Ok(42));
    assert_eq!(result.statistics.instructions, 4096);
    let result = run(
        &source,
        0,
        limits {
            instructions: 4095,
            output_bytes: 0,
        },
        &mut sink,
    );
    assert_eq!(result.outcome, Err(error::instruction_limit));
    assert_eq!(result.statistics.instructions, 4095);
    assert_eq!(source.lines().nth(result.line - 1), Some("RET"));
}

#[test]
fn later_arithmetic_failure_retains_earlier_output_and_failure_location() {
    let source = edition(
        &[],
        "PUSH_CONST 7\nCONSOLE_WRITELN\nPUSH_CONST 1\nPUSH_CONST 0\nDIV i64\nRET",
    );
    let mut sink = recorder::default();
    let result = run(
        &source,
        authority::console_write,
        limits::default(),
        &mut sink,
    );
    assert_eq!(result.outcome, Err(error::division_by_zero));
    assert_eq!(result.statistics.instructions, 5);
    assert_eq!(result.statistics.console_calls, 1);
    assert_eq!(result.statistics.output_bytes, 2);
    assert_eq!(source.lines().nth(result.line - 1), Some("DIV i64"));
    assert_eq!(sink.published, b"7\n");
}

#[test]
fn text_locals_reassign_and_decode_the_pinned_literal_spellings() {
    let source = edition(
        &[("first", "first"), ("second", "é\\n\\t\\\"\\\\\\b")],
        "PUSH_CONST_STR first\nSTORE_LOCAL text\nLOAD_LOCAL text\nCALL println 1\nPUSH_CONST_STR second\nSTORE_LOCAL text\nLOAD_LOCAL text\nCONSOLE_WRITE\nPUSH_CONST 0\nRET",
    );
    let mut sink = recorder::default();
    let result = run(
        &source,
        authority::console_write,
        limits::default(),
        &mut sink,
    );
    assert_eq!(result.outcome, Ok(0));
    assert_eq!(result.statistics.console_calls, 2);
    assert_eq!(sink.published, "first\né\n\t\"\\\\b".as_bytes());
    assert_eq!(result.statistics.output_bytes, sink.published.len());
}

#[test]
fn all_console_spellings_have_their_exact_newline_behavior() {
    for (operation, expected) in [
        ("CALL println 1", b"42\n".as_slice()),
        ("CALL core__print 1", b"42\n"),
        ("CALL console__writeln 1", b"42\n"),
        ("CALL console__write 1", b"42"),
        ("CALL console__sys_write 1", b"42"),
        ("CONSOLE_WRITE", b"42"),
        ("CONSOLE_WRITELN", b"42\n"),
        ("INTEROP_CALL intrinsic console.write 1 void", b"42"),
        ("INTEROP_CALL intrinsic console.writeln 1 void", b"42\n"),
    ] {
        let source = edition(
            &[],
            &format!("PUSH_CONST 42\n{operation}\nPUSH_CONST 0\nRET"),
        );
        let mut sink = recorder::default();
        let result = run(
            &source,
            authority::console_write,
            limits::default(),
            &mut sink,
        );
        assert_eq!(result.outcome, Ok(0), "{operation}");
        assert_eq!(result.statistics.console_calls, 1);
        assert_eq!(result.statistics.output_bytes, expected.len());
        assert_eq!(sink.published, expected, "{operation}");
    }
}

#[test]
fn exact_stack_and_local_capacity_are_executable() {
    let stack_body = "PUSH_CONST 11\n".repeat(256) + &"POP\n".repeat(255) + "RET";
    let mut sink = recorder::default();
    assert_eq!(
        run(&edition(&[], &stack_body), 0, limits::default(), &mut sink).outcome,
        Ok(11)
    );

    let mut local_body = String::new();
    for index in 0..128 {
        local_body.push_str(&format!("PUSH_CONST {index}\nSTORE_LOCAL local_{index}\n"));
    }
    local_body.push_str("LOAD_LOCAL local_127\nRET");
    assert_eq!(
        run(&edition(&[], &local_body), 0, limits::default(), &mut sink).outcome,
        Ok(127)
    );
    assert!(sink.requests.is_empty());
}

#[test]
fn exact_constant_capacity_resolves_the_last_constant() {
    let names: Vec<String> = (0..128).map(|index| format!("text_{index}")).collect();
    let constants: Vec<(&str, &str)> = names.iter().map(|name| (name.as_str(), "last")).collect();
    let source = edition(
        &constants,
        "PUSH_CONST_STR text_127\nCONSOLE_WRITE\nPUSH_CONST 0\nRET",
    );
    let mut sink = recorder::default();
    let result = run(
        &source,
        authority::console_write,
        limits::default(),
        &mut sink,
    );
    assert_eq!(result.outcome, Ok(0));
    assert_eq!(sink.published, b"last");
}
