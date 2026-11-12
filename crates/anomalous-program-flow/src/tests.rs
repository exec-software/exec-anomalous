extern crate std;

use super::*;
use std::{format, string::String, vec, vec::Vec};

// Validation storage is explicitly caller-owned. Host tests use an explicit
// generous thread stack rather than relying on the test harness's small stack.
fn checked(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn edition(helpers: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n# String constants\nconst S = \"text\"\n{helpers}@ai_context(\"Validate bounded control flow\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn helper(name: &str, parameters: &str, result: &str, body: &str) -> String {
    format!(
        "# declaration_line 1\nfunction {name}{parameters} -> {result}:\n{body}\nend function\n"
    )
}

fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut image = vec![0; maximum_image_bytes];
    let length = encode(source.as_bytes(), &mut image, &mut workspace).unwrap();
    image.truncate(length);
    image
}

fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    encode(source.as_bytes(), &mut output, &mut workspace).map(|_| ())
}

#[test]
fn complete_compiler_outputs_admit_without_rewriting() {
    checked(|| {
        let fixtures = [
            include_str!("../../anomalous-program-executor/fixtures/flow/branches.nil"),
            include_str!("../../anomalous-program-executor/fixtures/flow/calls.nil"),
            include_str!("../../anomalous-program-executor/fixtures/flow/early_returns.nil"),
            include_str!("../../anomalous-program-executor/fixtures/flow/recursion.nil"),
            include_str!("../../anomalous-program-executor/fixtures/flow/text_void.nil"),
            include_str!("../../anomalous-program-executor/fixtures/flow/while.nil"),
        ];
        for source in fixtures {
            let image = package(source);
            let mut workspace = validation_workspace::new();
            let witness = admit(&image, authority::console_write, &mut workspace).unwrap();
            assert_eq!(witness.edition(), source.as_bytes());
            assert_eq!(witness.identity(), sha256(source.as_bytes()));
            assert_eq!(
                witness.program().functions()[witness.program().main()].name,
                "main"
            );
            for instruction in witness.program().instructions() {
                assert!(instruction.line > 0 && instruction.line <= source.lines().count());
            }
        }
    });
}

#[test]
fn function_signatures_keep_parameter_order_and_resolved_targets() {
    checked(|| {
        let functions = helper(
            "pair",
            "(number: i32, message: text)",
            "text",
            "STORE_LOCAL message\nSTORE_LOCAL number\nLOAD_LOCAL message\nRET",
        );
        let source = edition(
            &functions,
            "PUSH_CONST 7\nPUSH_CONST_STR S\nCALL pair 2\nPOP\nPUSH_CONST 0\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = admit(&image, 0, &mut workspace).unwrap();
        let plan = witness.program();
        assert_eq!(
            plan.functions()[0].parameter_kinds(),
            &[kind::integer, kind::text]
        );
        assert_eq!(plan.functions()[0].return_kind, Some(kind::text));
        assert_eq!(plan.functions()[0].local_count, 2);
        assert!(
            plan.instructions()
                .iter()
                .any(|item| item.operation == operation::call(0))
        );
        assert_eq!(plan.constants()[0].escaped, "text");
    });
}

#[test]
fn recursion_and_nonterminating_loops_validate_without_recursive_analysis() {
    checked(|| {
        let recursive = helper(
            "again",
            "(value: i64)",
            "i64",
            "STORE_LOCAL value\nLOAD_LOCAL value\nCALL again 1\nRET",
        );
        assert_eq!(
            validate(&edition(&recursive, "PUSH_CONST 0\nCALL again 1\nRET")),
            Ok(())
        );
        assert_eq!(validate(&edition("", "LABEL forever\nJMP forever")), Ok(()));
    });
}

#[test]
fn joins_require_identical_operand_depth_and_kinds() {
    checked(|| {
        let good = "PUSH_CONST 0\nJMP_IF other\nPUSH_CONST 1\nJMP joined\nLABEL other\nPUSH_CONST 2\nLABEL joined\nRET";
        assert_eq!(validate(&edition("", good)), Ok(()));
        let wrong_depth = good.replace("PUSH_CONST 2", "PUSH_CONST 2\nPUSH_CONST 3");
        let wrong_kind = good.replace("PUSH_CONST 2", "PUSH_CONST_STR S");
        assert_eq!(validate(&edition("", &wrong_depth)), Err(error::syntax));
        assert_eq!(validate(&edition("", &wrong_kind)), Err(error::syntax));
    });
}

#[test]
fn locals_must_be_initialized_on_every_incoming_path() {
    checked(|| {
        let good = "PUSH_CONST 0\nJMP_IF other\nPUSH_CONST 1\nSTORE_LOCAL result\nJMP joined\nLABEL other\nPUSH_CONST 2\nSTORE_LOCAL result\nLABEL joined\nLOAD_LOCAL result\nRET";
        assert_eq!(validate(&edition("", good)), Ok(()));
        let missing = good.replace("PUSH_CONST 2\nSTORE_LOCAL result", "NOP");
        assert_eq!(validate(&edition("", &missing)), Err(error::syntax));
        let conflict = good.replace("PUSH_CONST 2", "PUSH_CONST_STR S");
        assert_eq!(validate(&edition("", &conflict)), Err(error::syntax));
    });
}

#[test]
fn loop_backedges_preserve_stack_and_zero_iterations_do_not_initialize() {
    checked(|| {
        let good = "PUSH_CONST 0\nSTORE_LOCAL counter\nLABEL loop_start\nLOAD_LOCAL counter\nJMP_IF loop_end\nPUSH_CONST 1\nSTORE_LOCAL counter\nJMP loop_start\nLABEL loop_end\nLOAD_LOCAL counter\nRET";
        assert_eq!(validate(&edition("", good)), Ok(()));
        let growth = good.replace("JMP loop_start", "PUSH_CONST 9\nJMP loop_start");
        assert_eq!(validate(&edition("", &growth)), Err(error::syntax));
        let uninitialized = "LABEL loop_start\nPUSH_CONST 0\nJMP_IF loop_end\nPUSH_CONST 1\nSTORE_LOCAL result\nJMP loop_start\nLABEL loop_end\nLOAD_LOCAL result\nRET";
        assert_eq!(validate(&edition("", uninitialized)), Err(error::syntax));
    });
}

#[test]
fn dead_instructions_are_resolved_and_contribute_authority_without_fake_stack_roots() {
    checked(|| {
        let source = edition(
            "",
            "PUSH_CONST 0\nRET\nPOP\nCONSOLE_WRITE\nJMP dead_end\nLABEL dead_end\nRET $ny.generated.nonvoid-fallthrough",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        assert_eq!(admit(&image, 0, &mut workspace).err(), Some(error::ceiling));
        let witness = admit(&image, authority::console_write, &mut workspace).unwrap();
        assert_eq!(witness.required_services(), authority::console_write);
        assert_eq!(witness.granted_services(), authority::console_write);
        assert!(
            witness
                .program()
                .instructions()
                .iter()
                .any(|item| item.operation == operation::generated_fallthrough)
        );
        for (tail, expected) in [
            ("TIME_NOW", error::unsupported),
            ("JMP missing", error::syntax),
            ("CALL missing 0", error::unsupported),
            ("PUSH_CONST_STR missing", error::syntax),
            ("CALL println 2", error::unsupported),
            ("ADD", error::unsupported),
        ] {
            assert_eq!(
                validate(&edition("", &format!("PUSH_CONST 0\nRET\n{tail}"))),
                Err(expected)
            );
        }
    });
}

#[test]
fn every_function_is_validated_even_when_never_called() {
    checked(|| {
        let bad = helper("unused", "", "i64", "POP\nPUSH_CONST 0\nRET");
        assert_eq!(
            validate(&edition(&bad, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
        let effect = helper("unused", "", "void", "PUSH_CONST 1\nCONSOLE_WRITE\nRET");
        let image = package(&edition(&effect, "PUSH_CONST 0\nRET"));
        let mut workspace = validation_workspace::new();
        assert_eq!(admit(&image, 0, &mut workspace).err(), Some(error::ceiling));
    });
}

#[test]
fn generated_fallthrough_is_nonvoid_only_and_remains_a_distinct_operation() {
    checked(|| {
        let source = edition("", "RET $ny.generated.nonvoid-fallthrough");
        assert_eq!(validate(&source), Ok(()));
        let bad = helper(
            "unused",
            "",
            "void",
            "RET $ny.generated.nonvoid-fallthrough",
        );
        assert_eq!(
            validate(&edition(&bad, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&edition("", "RET $other")),
            Err(error::unsupported)
        );
    });
}

#[test]
fn branch_conditions_are_integer_and_targets_stay_within_their_function() {
    checked(|| {
        assert_eq!(
            validate(&edition(
                "",
                "PUSH_CONST_STR S\nJMP_IF done\nLABEL done\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        let other = helper("other", "", "void", "LABEL other_label\nRET");
        assert_eq!(
            validate(&edition(&other, "JMP other_label")),
            Err(error::syntax)
        );
        assert_eq!(validate(&edition("", "JMP main")), Err(error::syntax));
    });
}

#[test]
fn all_labels_and_function_names_share_a_collision_free_global_namespace() {
    checked(|| {
        let other = helper("other", "", "void", "LABEL shared\nRET");
        assert_eq!(
            validate(&edition(&other, "LABEL shared\nPUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&edition("", "LABEL main\nPUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&edition("", "LABEL again\nLABEL again\nPUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
        let duplicate = helper("other", "", "void", "RET").repeat(2);
        assert_eq!(
            validate(&edition(&duplicate, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
    });
}

#[test]
fn static_call_names_cannot_be_redirected_through_locals_or_parameters() {
    checked(|| {
        let later = helper("target", "", "i64", "PUSH_CONST 0\nRET");
        let source = edition(
            "",
            "PUSH_CONST_STR S\nSTORE_LOCAL target\nPUSH_CONST 0\nRET",
        ) + &later;
        assert_eq!(validate(&source), Err(error::unsupported));
        let parameter = helper("other", "(target: text)", "void", "POP\nRET") + &later;
        assert_eq!(
            validate(&edition(&parameter, "PUSH_CONST 0\nRET")),
            Err(error::unsupported)
        );
        assert_eq!(
            validate(&edition(
                "",
                "PUSH_CONST 1\nSTORE_LOCAL println\nPUSH_CONST 0\nRET"
            )),
            Err(error::unsupported)
        );
        assert_eq!(
            validate(&edition(
                &helper("println", "", "void", "RET"),
                "PUSH_CONST 0\nRET"
            )),
            Err(error::unsupported)
        );
    });
}

#[test]
fn call_arity_and_argument_kinds_match_resolved_signatures() {
    checked(|| {
        let callee = helper(
            "callee",
            "(value: text)",
            "text",
            "STORE_LOCAL value\nLOAD_LOCAL value\nRET",
        );
        assert_eq!(
            validate(&edition(
                &callee,
                "PUSH_CONST_STR S\nCALL callee 1\nPOP\nPUSH_CONST 0\nRET"
            )),
            Ok(())
        );
        assert_eq!(
            validate(&edition(
                &callee,
                "PUSH_CONST 1\nCALL callee 1\nPOP\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&edition(&callee, "CALL callee 0\nPOP\nPUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&edition(&callee, "PUSH_CONST 0\nRET\nCALL callee 0")),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&edition(&callee, "CALL callee 1\nPOP\nPUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
    });
}

#[test]
fn return_kinds_and_reachable_fallthrough_are_checked() {
    checked(|| {
        for body in [
            "NOP",
            "PUSH_CONST_STR S\nRET",
            "PUSH_CONST 0\nPUSH_CONST 1\nRET",
        ] {
            assert_eq!(validate(&edition("", body)), Err(error::syntax));
        }
        assert_eq!(
            validate(&edition(
                &helper("other", "", "void", "PUSH_CONST 0\nRET"),
                "PUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        let missing_local = helper("read", "(value: i64)", "i64", "LOAD_LOCAL value\nRET");
        assert_eq!(
            validate(&edition(&missing_local, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
    });
}

#[test]
fn canonical_wire_is_profile_two_and_cross_profile_admission_fails() {
    checked(|| {
        let source = edition("", "PUSH_CONST 0\nRET");
        let image = package(&source);
        assert_eq!(&image[..8], b"ANPROG\r\n");
        assert_eq!(get32(&image, 12), 2);
        assert_eq!(&image[64..72], b"ANSMAN\r\n");
        assert_eq!(get32(&image, 76), 2);
        assert_eq!(
            anomalous_program_manifest::admit(&image, 0).err(),
            Some(error::profile)
        );
        let mut profile_one = vec![0; maximum_image_bytes];
        let length =
            anomalous_program_manifest::encode(source.as_bytes(), &mut profile_one).unwrap();
        profile_one.truncate(length);
        let mut workspace = validation_workspace::new();
        assert_eq!(
            admit(&profile_one, 0, &mut workspace).err(),
            Some(error::profile)
        );
        for offset in [12, 76] {
            let mut changed = image.clone();
            put32(&mut changed, offset, 1);
            let mut workspace = validation_workspace::new();
            assert_eq!(
                admit(&changed, 0, &mut workspace).err(),
                Some(error::profile)
            );
        }
    });
}

#[test]
fn wire_mutations_reject_before_witness_publication() {
    checked(|| {
        let source = edition("", "PUSH_CONST 0\nRET");
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        for (offset, expected) in [
            (0, error::encoding),
            (8, error::version),
            (10, error::encoding),
            (16, error::layout),
            (24, error::layout),
            (32, error::layout),
            (40, error::layout),
            (48, error::layout),
            (56, error::encoding),
            (64, error::encoding),
            (72, error::version),
            (74, error::encoding),
            (88, error::layout),
            (96, error::identity),
            (128, error::layout),
            (132, error::encoding),
            (159, error::encoding),
        ] {
            let mut changed = image.clone();
            changed[offset] ^= 1;
            let mut workspace = validation_workspace::new();
            assert_eq!(
                admit(&changed, 0, &mut workspace).err(),
                Some(expected),
                "offset {offset}"
            );
        }
        let mut changed = image.clone();
        put64(&mut changed, 32, u64::MAX);
        assert_eq!(
            admit(&changed, 0, &mut workspace).err(),
            Some(error::layout)
        );
        let mut changed = image.clone();
        changed.push(0);
        assert_eq!(
            admit(&changed, 0, &mut workspace).err(),
            Some(error::layout)
        );
        for length in 0..160 {
            assert!(admit(&image[..length], 0, &mut workspace).is_err());
        }
        assert_eq!(admit(&image, 2, &mut workspace).err(), Some(error::ceiling));
    });
}

#[test]
fn authority_and_intent_are_rederived_from_exact_edition_bytes() {
    checked(|| {
        let source = edition("", "PUSH_CONST 1\nCONSOLE_WRITE\nPUSH_CONST 0\nRET");
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let mut changed = image.clone();
        put64(&mut changed, 80, 0);
        assert_eq!(
            admit(&changed, 1, &mut workspace).err(),
            Some(error::mismatch)
        );
        let mut changed = image.clone();
        changed[160] ^= 1;
        assert_eq!(
            admit(&changed, 1, &mut workspace).err(),
            Some(error::mismatch)
        );
        let mut changed = image.clone();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        assert_eq!(
            admit(&changed, 1, &mut workspace).err(),
            Some(error::identity)
        );
        assert_eq!(admit(&image, 0, &mut workspace).err(), Some(error::ceiling));
    });
}

#[test]
fn invalid_input_and_short_output_never_modify_the_destination() {
    checked(|| {
        let source = edition("", "PUSH_CONST 0\nRET");
        let mut workspace = validation_workspace::new();
        let mut output = [0xa5; 160];
        assert_eq!(
            encode(source.as_bytes(), &mut output, &mut workspace),
            Err(error::output)
        );
        assert_eq!(output, [0xa5; 160]);
        let bad = edition("", "PUSH_CONST 0\nJMP missing\nRET");
        assert_eq!(
            encode(bad.as_bytes(), &mut output, &mut workspace),
            Err(error::syntax)
        );
        assert_eq!(output, [0xa5; 160]);
        assert_eq!(
            encode(b"", &mut output, &mut workspace),
            Err(error::edition)
        );
    });
}

#[test]
fn workspace_reuse_cannot_retain_previous_authority_or_flow_state() {
    checked(|| {
        let effect = edition("", "PUSH_CONST 1\nCONSOLE_WRITE\nPUSH_CONST 0\nRET");
        let pure = edition("", "PUSH_CONST 42\nRET");
        let a = package(&effect);
        let b = package(&pure);
        let mut workspace = validation_workspace::new();
        assert_eq!(admit(&a, 1, &mut workspace).unwrap().required_services(), 1);
        assert_eq!(admit(&a, 0, &mut workspace).err(), Some(error::ceiling));
        let witness = admit(&b, 0, &mut workspace).unwrap();
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.identity(), sha256(pure.as_bytes()));
        assert_eq!(witness.program().instructions().len(), 2);
    });
}

#[test]
fn instruction_stack_and_local_bounds_accept_exactly_the_limit() {
    checked(|| {
        let exact = "NOP\n".repeat(4094) + "PUSH_CONST 0\nRET";
        assert_eq!(validate(&edition("", &exact)), Ok(()));
        assert_eq!(
            validate(&edition("", &(String::from("NOP\n") + &exact))),
            Err(error::limit)
        );
        let stack = "PUSH_CONST 1\n".repeat(256) + &"POP\n".repeat(255) + "RET";
        assert_eq!(validate(&edition("", &stack)), Ok(()));
        assert_eq!(
            validate(&edition("", &(String::from("PUSH_CONST 1\n") + &stack))),
            Err(error::limit)
        );
        let mut locals = String::new();
        for index in 0..128 {
            locals.push_str(&format!("PUSH_CONST {index}\nSTORE_LOCAL local_{index}\n"));
        }
        assert_eq!(
            validate(&edition(
                "",
                &(locals.clone() + "LOAD_LOCAL local_127\nRET")
            )),
            Ok(())
        );
        assert_eq!(
            validate(&edition(
                "",
                &(locals + "PUSH_CONST 1\nSTORE_LOCAL excess\nPUSH_CONST 0\nRET")
            )),
            Err(error::limit)
        );
    });
}

#[test]
fn function_parameter_and_basic_block_caps_are_explicit() {
    checked(|| {
        let mut functions = String::new();
        for index in 0..63 {
            functions.push_str(&helper(&format!("helper_{index}"), "", "void", "RET"));
        }
        assert_eq!(validate(&edition(&functions, "PUSH_CONST 0\nRET")), Ok(()));
        functions.push_str(&helper("excess", "", "void", "RET"));
        assert_eq!(
            validate(&edition(&functions, "PUSH_CONST 0\nRET")),
            Err(error::limit)
        );
        let args = "(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64)";
        let eight = helper("eight", args, "void", &"POP\n".repeat(8));
        let eight = eight.replace("\nend function", "RET\nend function");
        assert_eq!(validate(&edition(&eight, "PUSH_CONST 0\nRET")), Ok(()));
        let nine = eight.replace("h:i64)", "h:i64,i:i64)");
        assert_eq!(
            validate(&edition(&nine, "PUSH_CONST 0\nRET")),
            Err(error::limit)
        );
        let mut labels = String::new();
        for index in 0..256 {
            labels.push_str(&format!("LABEL label_{index}\n"));
        }
        assert_eq!(
            validate(&edition("", &(labels.clone() + "PUSH_CONST 0\nRET"))),
            Ok(())
        );
        labels.push_str("LABEL excess\nPUSH_CONST 0\nRET");
        assert_eq!(validate(&edition("", &labels)), Err(error::limit));
    });
}

#[test]
fn constant_and_name_bounds_and_unknown_records_fail_closed() {
    checked(|| {
        let mut constants = String::new();
        for index in 1..128 {
            constants.push_str(&format!("const S{index} = \"value\"\n"));
        }
        let source = edition("", "PUSH_CONST 0\nRET")
            .replace("@ai_context", &(constants.clone() + "@ai_context"));
        assert_eq!(validate(&source), Ok(()));
        let excess = source.replace("@ai_context", "const excess = \"value\"\n@ai_context");
        assert_eq!(validate(&excess), Err(error::limit));
        let long = "x".repeat(129);
        assert_eq!(
            validate(&edition(
                "",
                &format!("PUSH_CONST 1\nSTORE_LOCAL {long}\nPUSH_CONST 0\nRET")
            )),
            Err(error::limit)
        );
        for body in [
            "PUSH_CONST 0x10\nRET",
            "PUSH_CONST 0\nSTORE_LOCAL $value\nPUSH_CONST 0\nRET",
            "PUSH_CONST 0\nRET\n# function_annotation \"hidden\"",
        ] {
            assert!(validate(&edition("", body)).is_err());
        }
    });
}
