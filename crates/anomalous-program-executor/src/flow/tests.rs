extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{admit, encode, validation_workspace};
use anomalous_program_manifest::maximum_image_bytes;
use std::{boxed::Box, format, string::String, vec, vec::Vec};

#[derive(Default)]
struct recorder {
    output: Vec<u8>,
    requests: usize,
    refuse: bool,
    identity: Option<[u8; 32]>,
}
impl console_sink for recorder {
    fn write(&mut self, request: console_request<'_>) -> Result<(), service_error> {
        self.requests += 1;
        assert_eq!(request.sequence(), self.requests);
        if let Some(identity) = self.identity {
            assert_eq!(identity, request.edition_identity());
        }
        self.identity = Some(request.edition_identity());
        if self.refuse {
            return Err(service_error::refused);
        }
        self.output.extend_from_slice(request.bytes());
        Ok(())
    }
}

fn source(main: &str, helpers: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n@ai_context(\"Exercise bounded calls and branches\")\n# declaration_line 1\nfunction main -> i64:\n{main}\nend function\n{helpers}"
    )
}

fn cleared(workspace: &execution_workspace<'_>) {
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
    for frame in &workspace.frames {
        assert_eq!((frame.function, frame.floor, frame.resume), (0, 0, 0));
        assert!(frame.locals.iter().all(Option::is_none));
    }
}

fn on_stack<T: Send>(run: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn_scoped(scope, run)
            .unwrap()
            .join()
            .unwrap()
    })
}

fn run(edition: &[u8], budget: limits, sink: &mut recorder) -> execution {
    on_stack(move || run_inner(edition, budget, sink))
}

fn run_inner(edition: &[u8], budget: limits, sink: &mut recorder) -> execution {
    let mut validation = Box::new(validation_workspace::new());
    let mut image = vec![0; maximum_image_bytes];
    let length = encode(edition, &mut image, &mut validation).unwrap();
    let admitted = admit(&image[..length], 1, &mut validation).unwrap();
    let mut workspace = Box::new(execution_workspace::new());
    let result = execute(&admitted, budget, &mut workspace, sink);
    assert_eq!(result.edition_identity, admitted.identity());
    if let Some(identity) = sink.identity {
        assert_eq!(identity, admitted.identity());
    }
    cleared(&workspace);
    result
}

#[test]
fn complete_compiler_functions_branches_loops_and_recursion_match_results() {
    let cases: &[(&[u8], i64, &[u8])] = &[
        (include_bytes!("../../fixtures/flow/calls.nil"), 42, b""),
        (include_bytes!("../../fixtures/flow/branches.nil"), 32, b""),
        (include_bytes!("../../fixtures/flow/while.nil"), 13, b""),
        (
            include_bytes!("../../fixtures/flow/recursion.nil"),
            120,
            b"",
        ),
        (
            include_bytes!("../../fixtures/flow/text_void.nil"),
            0,
            b"yes\n",
        ),
        (
            include_bytes!("../../fixtures/flow/early_returns.nil"),
            0,
            b"",
        ),
    ];
    for &(edition, expected, output) in cases {
        let mut sink = recorder::default();
        let result = run(edition, limits::default(), &mut sink);
        assert_eq!(result.outcome, Ok(expected));
        assert_eq!(sink.output, output);
        assert_eq!(
            result.statistics.console_calls,
            usize::from(!output.is_empty())
        );
        assert_eq!(result.statistics.output_bytes, output.len());
        assert!(result.statistics.instructions > 0);
    }
}

#[test]
fn calls_preserve_order_and_caller_prefix() {
    let edition = source(
        "PUSH_CONST 100\nPUSH_CONST 7\nPUSH_CONST 9\nCALL subtract 2\nSUB i64\nRET",
        "# declaration_line 2\nfunction subtract(left: i64, right: i64) -> i64:\nSTORE_LOCAL right\nSTORE_LOCAL left\nLOAD_LOCAL left\nLOAD_LOCAL right\nSUB i64\nRET\nend function\n",
    );
    let result = run(
        edition.as_bytes(),
        limits::default(),
        &mut recorder::default(),
    );
    assert_eq!(result.outcome, Ok(102));
    assert_eq!(result.statistics.instructions, 12);
}

#[test]
fn typed_calls_and_returns_do_not_insert_narrowing() {
    let edition = source(
        "PUSH_CONST 4294967297\nCALL identity 1\nRET",
        "# declaration_line 2\nfunction identity(number: i32) -> i32:\nSTORE_LOCAL number\nLOAD_LOCAL number\nRET\nend function\n",
    );
    let result = run(
        edition.as_bytes(),
        limits::default(),
        &mut recorder::default(),
    );
    assert_eq!(result.outcome, Ok(4294967297));
}

#[test]
fn ordinary_labels_consume_steps_and_untaken_branches_pop_conditions() {
    let edition = source(
        "PUSH_CONST 0\nJMP_IF taken\nPUSH_CONST -3\nJMP_IF_NOT taken\nJMP done\nLABEL taken\nPUSH_CONST 0\nRET\nLABEL done\nPUSH_CONST 42\nRET",
        "",
    );
    let result = run(
        edition.as_bytes(),
        limits::default(),
        &mut recorder::default(),
    );
    assert_eq!(result.outcome, Ok(42));
    assert_eq!(result.statistics.instructions, 8);
}

#[test]
fn child_failure_keeps_prior_output_and_clears_all_frame_storage() {
    let edition = source(
        "CALL broken 0\nRET",
        "# declaration_line 2\nfunction broken -> i64:\nPUSH_CONST 7\nCONSOLE_WRITELN\nPUSH_CONST 1\nPUSH_CONST 0\nDIV i64\nRET\nend function\n",
    );
    let mut sink = recorder::default();
    let result = run(edition.as_bytes(), limits::default(), &mut sink);
    assert_eq!(result.outcome, Err(error::division_by_zero));
    assert_eq!(result.statistics.instructions, 6);
    assert_eq!(result.statistics.console_calls, 1);
    assert_eq!(result.statistics.output_bytes, 2);
    assert_eq!(sink.output, b"7\n");
}

#[test]
fn recursive_frames_refuse_at_explicit_limit() {
    let edition = source(
        "CALL recurse 0\nRET",
        "# declaration_line 2\nfunction recurse -> i64:\nCALL recurse 0\nRET\nend function\n",
    );
    for frames in [1, 2, 16] {
        let result = run(
            edition.as_bytes(),
            limits {
                frames,
                ..limits::default()
            },
            &mut recorder::default(),
        );
        assert_eq!(result.outcome, Err(error::frame_limit));
        assert_eq!(result.statistics.instructions, frames);
        assert_eq!(result.statistics.console_calls, 0);
    }
}

#[test]
fn looping_program_exhausts_fuel_without_host_recursion() {
    let edition = source("PUSH_CONST 7\nCONSOLE_WRITE\nLABEL repeat\nJMP repeat", "");
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
    assert_eq!(sink.output, b"7");
}

#[test]
fn global_operand_pool_is_bounded_across_caller_prefixes() {
    let main = "PUSH_CONST 1\n".repeat(248)
        + "PUSH_CONST 0\nCALL helper 1\n"
        + &"POP\n".repeat(248)
        + "RET";
    let helper = String::from(
        "# declaration_line 2\nfunction helper(argument: i64) -> i64:\nSTORE_LOCAL argument\n",
    ) + &"PUSH_CONST 0\n".repeat(9)
        + &"POP\n".repeat(8)
        + "RET\nend function\n";
    let result = run(
        source(&main, &helper).as_bytes(),
        limits::default(),
        &mut recorder::default(),
    );
    assert_eq!(result.outcome, Err(error::stack_limit));
    assert_eq!(result.statistics.instructions, 260);
}

#[test]
fn reached_generated_fallthrough_is_distinct_from_return() {
    let edition = source("RET $ny.generated.nonvoid-fallthrough", "");
    let result = run(
        edition.as_bytes(),
        limits::default(),
        &mut recorder::default(),
    );
    assert_eq!(result.outcome, Err(error::generated_fallthrough));
    assert_eq!(result.statistics.instructions, 1);
}

#[test]
fn exhausted_or_invalid_initial_limits_never_reach_a_sink() {
    let edition = source("PUSH_CONST 7\nCONSOLE_WRITE\nPUSH_CONST 0\nRET", "");
    for (budget, expected) in [
        (
            limits {
                instructions: 0,
                ..limits::default()
            },
            error::instruction_limit,
        ),
        (
            limits {
                frames: 0,
                ..limits::default()
            },
            error::frame_limit,
        ),
        (
            limits {
                instructions: maximum_steps + 1,
                ..limits::default()
            },
            error::invalid_limits,
        ),
        (
            limits {
                frames: maximum_frames + 1,
                ..limits::default()
            },
            error::invalid_limits,
        ),
        (
            limits {
                output_bytes: maximum_output_bytes + 1,
                ..limits::default()
            },
            error::invalid_limits,
        ),
    ] {
        let mut sink = recorder::default();
        let result = run(edition.as_bytes(), budget, &mut sink);
        assert_eq!(result.outcome, Err(expected));
        assert_eq!(result.statistics, statistics::default());
        assert_eq!(sink.requests, 0);
    }
}

#[test]
fn service_failure_in_child_is_not_retried() {
    let mut sink = recorder {
        refuse: true,
        ..recorder::default()
    };
    let result = run(
        include_bytes!("../../fixtures/flow/text_void.nil"),
        limits::default(),
        &mut sink,
    );
    assert_eq!(result.outcome, Err(error::service));
    assert_eq!(result.statistics.console_calls, 1);
    assert_eq!(result.statistics.output_bytes, 0);
    assert_eq!(sink.requests, 1);
    assert!(sink.output.is_empty());
}

#[test]
fn execution_workspace_is_reusable_after_a_terminal_error() {
    on_stack(|| {
        let edition = source("PUSH_CONST 42\nRET", "");
        let mut validation = Box::new(validation_workspace::new());
        let mut image = vec![0; maximum_image_bytes];
        let length = encode(edition.as_bytes(), &mut image, &mut validation).unwrap();
        let admitted = admit(&image[..length], 0, &mut validation).unwrap();
        let mut workspace = Box::new(execution_workspace::new());
        let mut sink = recorder::default();
        let refused = execute(
            &admitted,
            limits {
                instructions: 1,
                ..limits::default()
            },
            &mut workspace,
            &mut sink,
        );
        assert_eq!(refused.outcome, Err(error::instruction_limit));
        cleared(&workspace);
        let returned = execute(&admitted, limits::default(), &mut workspace, &mut sink);
        assert_eq!(returned.outcome, Ok(42));
        assert_eq!(returned.statistics.instructions, 2);
        cleared(&workspace);
    });
}
