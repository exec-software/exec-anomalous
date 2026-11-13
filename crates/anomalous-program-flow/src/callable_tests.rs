extern crate std;

use super::*;
use std::{format, string::String, vec, vec::Vec};

fn checked(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

const helpers: &str = "# declaration_line 1\nfunction first(value: i64) -> i64:\nRET\nend function\n# declaration_line 1\nfunction second(other: i64) -> i64:\nPUSH_CONST 1\nADD i64\nRET\nend function\n# declaration_line 1\nfunction answer -> i64:\nPUSH_CONST 42\nRET\nend function\n";

fn edition(annotation: &str, functions: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n# String constants\nconst A = \"first\"\n{functions}{annotation}# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn authored(functions: &str, body: &str) -> String {
    edition(
        "@ai_context(\"Exercise typed internal callables\")\n",
        functions,
        body,
    )
}

fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let length = callable::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(length);
    output
}

fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    callable::encode(source.as_bytes(), &mut output, &mut workspace).map(|_| ())
}

#[test]
fn callable_profile_changes_only_the_two_explicit_tags_for_older_syntax() {
    checked(|| {
        let source = authored("", "NEW_ARRAY_LITERAL 0 i64\nLEN\nRET");
        let image = package(&source);
        let mut previous = vec![0; maximum_image_bytes];
        let mut workspace = validation_workspace::new();
        let length = array::encode(source.as_bytes(), &mut previous, &mut workspace).unwrap();
        previous.truncate(length);
        assert_eq!(callable::derivation_profile, 6);
        assert_eq!(get32(&image, 12), 6);
        assert_eq!(get32(&image, image_header_bytes + 12), 6);
        put32(&mut previous, 12, 6);
        put32(&mut previous, image_header_bytes + 12, 6);
        assert_eq!(previous, image);
        let witness = callable::admit(&image, 1, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 6);
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.intent(), "Exercise typed internal callables");
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.granted_services(), 0);
    });
}

#[test]
fn callable_headers_require_profile_six_and_older_entry_points_refuse_it() {
    checked(|| {
        let image = package(&authored("", "PUSH_CONST 0\nRET"));
        for outer in [0, 1, 2, 3, 4, 5, 6, 7, u32::MAX] {
            for inner in [0, 1, 2, 3, 4, 5, 6, 7, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                if outer == 6 && inner == 6 {
                    assert_eq!(
                        callable::admit(&changed, 0, &mut workspace)
                            .unwrap()
                            .profile(),
                        6
                    );
                } else {
                    assert_eq!(
                        callable::admit(&changed, 0, &mut workspace).err(),
                        Some(error::profile)
                    );
                }
            }
        }
        let mut workspace = validation_workspace::new();
        assert_eq!(
            array::admit(&image, 0, &mut workspace).err(),
            Some(error::profile)
        );
        assert_eq!(
            text::admit(&image, 0, &mut workspace).err(),
            Some(error::profile)
        );
        assert_eq!(
            scalar::admit(&image, 0, &mut workspace).err(),
            Some(error::profile)
        );
        assert_eq!(admit(&image, 0, &mut workspace).err(), Some(error::profile));
        assert_eq!(
            anomalous_program_manifest::admit(&image, 0).err(),
            Some(error::profile)
        );
    });
}

#[test]
fn earlier_profiles_keep_callable_syntax_and_collision_refusals() {
    checked(|| {
        for body in [
            "PUSH_FN_PTR answer\nSTORE_LOCAL chosen\nCALL chosen 0\nRET",
            "PUSH_CONST 0\nRET\nPUSH_FN_PTR answer",
            "PUSH_CONST 0\nRET\nCALL $ny.temp.callee.0 0\nPUSH_CONST 0\nSTORE_LOCAL $ny.temp.callee.0",
            "PUSH_CONST 0\nSTORE_LOCAL first\nPUSH_CONST 0\nRET",
            "PUSH_CONST 0\nSTORE_LOCAL println\nPUSH_CONST 0\nRET",
        ] {
            let source = authored(helpers, body);
            // Prior CALL grammar checks a simple identifier before target
            // resolution; a dollar temporary therefore retains syntax refusal.
            let expected = if body.contains("CALL $ny.temp.") {
                error::syntax
            } else {
                error::unsupported
            };
            let image = package(&source);
            let mut workspace = validation_workspace::new();
            let mut output = vec![0xa5; maximum_image_bytes];
            assert_eq!(
                encode(source.as_bytes(), &mut output, &mut workspace),
                Err(expected)
            );
            assert_eq!(
                scalar::encode(source.as_bytes(), &mut output, &mut workspace),
                Err(expected)
            );
            assert_eq!(
                text::encode(source.as_bytes(), &mut output, &mut workspace),
                Err(expected)
            );
            assert_eq!(
                array::encode(source.as_bytes(), &mut output, &mut workspace),
                Err(expected)
            );
            assert!(output.iter().all(|byte| *byte == 0xa5));
            for profile in [2, 3, 4, 5] {
                let mut changed = image.clone();
                put32(&mut changed, 12, profile);
                put32(&mut changed, image_header_bytes + 12, profile);
                let mut workspace = validation_workspace::new();
                let result = match profile {
                    2 => admit(&changed, 0, &mut workspace),
                    3 => scalar::admit(&changed, 0, &mut workspace),
                    4 => text::admit(&changed, 0, &mut workspace),
                    _ => array::admit(&changed, 0, &mut workspace),
                };
                assert_eq!(result.err(), Some(expected));
            }
        }
        for (body, expected) in [
            (
                "PUSH_FN_PTR main\nPOP\nPUSH_CONST 0\nRET",
                error::unsupported,
            ),
            ("PUSH_CONST 0\nRET\nPUSH_FN_PTR main", error::syntax),
        ] {
            let source = authored("", body);
            let mut output = vec![0; maximum_image_bytes];
            assert_eq!(
                anomalous_program_manifest::encode(source.as_bytes(), &mut output),
                Err(expected)
            );
        }
    });
}

#[test]
fn references_and_local_calls_resolve_into_typed_plan_operations() {
    checked(|| {
        let source = authored(
            helpers,
            "PUSH_FN_PTR second\nSTORE_LOCAL chosen\nLOAD_LOCAL chosen\nSTORE_LOCAL $ny.temp.callee.0\nPUSH_CONST 41\nCALL $ny.temp.callee.0 1\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = callable::admit(&image, 0, &mut workspace).unwrap();
        let plan = witness.program();
        let start = plan.functions()[plan.main()].start;
        assert_eq!(plan.instructions()[start].operation, operation::function(1));
        assert_eq!(
            plan.instructions()[start + 5].operation,
            operation::call_local {
                local: 1,
                arguments: 1
            }
        );
        assert_eq!(plan.functions()[0].signature(), 0);
        assert_eq!(plan.functions()[1].signature(), 0);
        assert_eq!(plan.functions()[2].signature(), 2);
        assert_eq!(plan.functions()[3].signature(), 2);
        assert_eq!(core::mem::size_of::<kind>(), 2);
    });
}

#[test]
fn exact_signature_identity_preserves_width_order_arity_arrays_and_void() {
    checked(|| {
        let functions = "# declaration_line 1\nfunction word32(value: i32) -> i32:\nRET\nend function\n# declaration_line 1\nfunction word64(value: i64) -> i64:\nRET\nend function\n# declaration_line 1\nfunction change_result(value: i32) -> i64:\nRET\nend function\n# declaration_line 1\nfunction words(left: i32, right: i64) -> void:\nPOP\nPOP\nRET\nend function\n# declaration_line 1\nfunction reversed(left: i64, right: i32) -> void:\nPOP\nPOP\nRET\nend function\n# declaration_line 1\nfunction texts(items: array<text>) -> array<text>:\nRET\nend function\n# declaration_line 1\nfunction numbers(items: array<i64>) -> array<i64>:\nRET\nend function\n# declaration_line 1\nfunction same(renamed: i32) -> i32:\nRET\nend function\n";
        let image = package(&authored(functions, "PUSH_CONST 0\nRET"));
        let mut workspace = validation_workspace::new();
        let witness = callable::admit(&image, 0, &mut workspace).unwrap();
        for (index, expected) in [0, 1, 2, 3, 4, 5, 6, 0].iter().enumerate() {
            assert_eq!(witness.program().functions()[index].signature(), *expected);
        }
        // Leading i32 codes are zero; arity must prevent trailing-zero aliasing.
        let functions = "# declaration_line 1\nfunction empty -> void:\nRET\nend function\n# declaration_line 1\nfunction eight(a: i32,b: i32,c: i32,d: i32,e: i32,f: i32,g: i32,h: i32) -> void:\nPOP\nPOP\nPOP\nPOP\nPOP\nPOP\nPOP\nPOP\nRET\nend function\n";
        let image = package(&authored(functions, "PUSH_CONST 0\nRET"));
        let mut workspace = validation_workspace::new();
        let witness = callable::admit(&image, 0, &mut workspace).unwrap();
        assert_ne!(
            witness.program().functions()[0].signature(),
            witness.program().functions()[1].signature()
        );
    });
}

#[test]
fn different_targets_with_the_same_signature_merge_and_rebind() {
    checked(|| {
        for body in [
            "PUSH_CONST 0\nJMP_IF_NOT alternative\nPUSH_FN_PTR first\nJMP ready\nLABEL alternative\nPUSH_FN_PTR second\nLABEL ready\nSTORE_LOCAL chosen\nPUSH_CONST 41\nCALL chosen 1\nRET",
            "PUSH_CONST 0\nJMP_IF_NOT alternative\nPUSH_FN_PTR first\nSTORE_LOCAL chosen\nJMP ready\nLABEL alternative\nPUSH_FN_PTR second\nSTORE_LOCAL chosen\nLABEL ready\nPUSH_CONST 41\nCALL chosen 1\nRET",
            "PUSH_FN_PTR first\nSTORE_LOCAL chosen\nPUSH_FN_PTR second\nSTORE_LOCAL chosen\nPUSH_CONST 41\nCALL chosen 1\nRET",
            "PUSH_FN_PTR first\nSTORE_LOCAL chosen\nLABEL again\nPUSH_FN_PTR second\nSTORE_LOCAL chosen\nPUSH_CONST 0\nJMP_IF again\nPUSH_CONST 41\nCALL chosen 1\nRET",
        ] {
            assert_eq!(validate(&authored(helpers, body)), Ok(()), "{body}");
        }
    });
}

#[test]
fn mismatched_function_signatures_refuse_stores_and_cfg_joins() {
    checked(|| {
        let functions = format!(
            "{helpers}# declaration_line 1\nfunction narrow(value: i32) -> i64:\nRET\nend function\n# declaration_line 1\nfunction text_result(value: i64) -> text:\nPOP\nPUSH_CONST_STR A\nRET\nend function\n"
        );
        for target in ["answer", "narrow", "text_result"] {
            for body in [
                format!(
                    "PUSH_FN_PTR first\nSTORE_LOCAL chosen\nPUSH_FN_PTR {target}\nSTORE_LOCAL chosen\nPUSH_CONST 0\nRET"
                ),
                format!(
                    "PUSH_CONST 0\nJMP_IF_NOT alternative\nPUSH_FN_PTR first\nJMP ready\nLABEL alternative\nPUSH_FN_PTR {target}\nLABEL ready\nPOP\nPUSH_CONST 0\nRET"
                ),
                format!(
                    "PUSH_CONST 0\nJMP_IF_NOT alternative\nPUSH_FN_PTR first\nSTORE_LOCAL chosen\nJMP ready\nLABEL alternative\nPUSH_FN_PTR {target}\nSTORE_LOCAL chosen\nLABEL ready\nPUSH_CONST 0\nRET"
                ),
            ] {
                assert_eq!(
                    validate(&authored(&functions, &body)),
                    Err(error::syntax),
                    "{body}"
                );
            }
        }
    });
}

#[test]
fn local_calls_precede_direct_functions_and_builtins_without_false_effects() {
    checked(|| {
        for body in [
            "PUSH_FN_PTR answer\nSTORE_LOCAL first\nCALL first 0\nRET",
            "PUSH_FN_PTR answer\nSTORE_LOCAL println\nCALL println 0\nRET",
            "PUSH_FN_PTR first\nSTORE_LOCAL console__write\nPUSH_CONST 42\nCALL console__write 1\nRET",
        ] {
            let image = package(&authored(helpers, body));
            let mut workspace = validation_workspace::new();
            let witness = callable::admit(&image, 0, &mut workspace).unwrap();
            assert_eq!(witness.required_services(), 0);
            assert_eq!(witness.granted_services(), 0);
            assert!(
                witness
                    .program()
                    .instructions()
                    .iter()
                    .any(|instruction| matches!(
                        instruction.operation,
                        operation::call_local { .. }
                    ))
            );
            assert!(
                !witness
                    .program()
                    .instructions()
                    .iter()
                    .any(|instruction| matches!(instruction.operation, operation::console { .. }))
            );
        }
        let image = package(&authored(helpers, "CALL answer 0\nRET"));
        let mut workspace = validation_workspace::new();
        assert!(
            callable::admit(&image, 0, &mut workspace)
                .unwrap()
                .program()
                .instructions()
                .iter()
                .any(|instruction| instruction.operation == operation::call(2))
        );
    });
}

#[test]
fn indirect_calls_require_definite_callable_initialization_and_exact_arguments() {
    checked(|| {
        for body in [
            "PUSH_CONST_STR A\nSTORE_LOCAL chosen\nPUSH_CONST 42\nCALL chosen 1\nRET",
            "PUSH_CONST 42\nSTORE_LOCAL first\nPUSH_CONST 42\nCALL first 1\nRET",
            "PUSH_CONST_STR A\nSTORE_LOCAL println\nPUSH_CONST 42\nCALL println 1\nPUSH_CONST 0\nRET",
            "CALL answer 0\nRET\nPUSH_FN_PTR answer\nSTORE_LOCAL answer",
            "PUSH_CONST 42\nCALL println 1\nPUSH_CONST 0\nRET\nPUSH_FN_PTR first\nSTORE_LOCAL println",
            "PUSH_CONST 0\nJMP_IF_NOT ready\nPUSH_FN_PTR first\nSTORE_LOCAL chosen\nLABEL ready\nPUSH_CONST 42\nCALL chosen 1\nRET",
            "PUSH_FN_PTR first\nSTORE_LOCAL chosen\nCALL chosen 0\nRET",
            "PUSH_FN_PTR first\nSTORE_LOCAL chosen\nCALL chosen 1\nRET",
            "PUSH_FN_PTR first\nSTORE_LOCAL chosen\nPUSH_CONST_STR A\nCALL chosen 1\nRET",
            "PUSH_FN_PTR first\nSTORE_LOCAL chosen\nPUSH_CONST 1\nPUSH_CONST 2\nCALL chosen 2\nRET",
        ] {
            assert_eq!(
                validate(&authored(helpers, body)),
                Err(error::syntax),
                "{body}"
            );
        }
    });
}

#[test]
fn dead_callable_forms_keep_references_without_inventing_operand_state() {
    checked(|| {
        assert_eq!(
            validate(&authored(
                helpers,
                "PUSH_CONST 0\nRET\nCALL chosen 8\nPUSH_CONST 1\nSTORE_LOCAL chosen\nPUSH_FN_PTR first"
            )),
            Ok(())
        );
        for instruction in [
            "PUSH_FN_PTR",
            "PUSH_FN_PTR first extra",
            "PUSH_FN_PTR absent",
            "PUSH_FN_PTR println",
            "PUSH_FN_PTR libc__puts",
            "CALL absent 0",
            "CALL $ny.temp.absent.0 0",
            "CALL chosen 0 extra",
            "PUSH_CLOSURE first",
            "CALL_INDIRECT 0",
            "PUSH_FN_PTR first 0",
        ] {
            assert_eq!(
                validate(&authored(
                    helpers,
                    &format!("PUSH_CONST 0\nRET\n{instruction}")
                )),
                Err(error::unsupported),
                "{instruction}"
            );
        }
        assert_eq!(
            validate(&authored(
                helpers,
                "PUSH_CONST 0\nRET\nPUSH_FN_PTR $ny.temp.first.0"
            )),
            Err(error::syntax)
        );
        for count in ["-1", "+1", "0x1", "184467440737095516160"] {
            assert_eq!(
                validate(&authored(
                    helpers,
                    &format!("PUSH_CONST 0\nRET\nCALL first {count}")
                )),
                Err(error::syntax)
            );
        }
        assert_eq!(
            validate(&authored(helpers, "PUSH_CONST 0\nRET\nCALL first 9")),
            Err(error::limit)
        );
        assert_eq!(
            validate(&authored(helpers, "PUSH_CONST 0\nRET\nCALL first 0")),
            Err(error::syntax)
        );
    });
}

#[test]
fn whole_file_console_authority_includes_direct_dead_and_unused_target_bodies() {
    checked(|| {
        for body in [
            "PUSH_CONST 0\nRET\nCALL println 1",
            "PUSH_CONST 42\nCALL println 1\nPUSH_CONST 0\nRET",
        ] {
            let image = package(&authored(helpers, body));
            let mut workspace = validation_workspace::new();
            assert_eq!(
                callable::admit(&image, 0, &mut workspace).err(),
                Some(error::ceiling)
            );
            assert_eq!(
                callable::admit(&image, 1, &mut workspace)
                    .unwrap()
                    .required_services(),
                1
            );
        }
        let functions = format!(
            "{helpers}# declaration_line 1\nfunction output -> i64:\nPUSH_CONST 7\nCALL println 1\nPUSH_CONST 0\nRET\nend function\n"
        );
        for body in [
            "PUSH_FN_PTR output\nSTORE_LOCAL println\nCALL println 0\nRET",
            "PUSH_FN_PTR answer\nSTORE_LOCAL println\nCALL println 0\nRET",
        ] {
            let image = package(&authored(&functions, body));
            let mut workspace = validation_workspace::new();
            assert_eq!(
                callable::admit(&image, 0, &mut workspace).err(),
                Some(error::ceiling)
            );
            assert_eq!(
                callable::admit(&image, 1, &mut workspace)
                    .unwrap()
                    .required_services(),
                1
            );
        }
    });
}

#[test]
fn callable_values_cannot_escape_into_scalar_array_console_or_signature_forms() {
    checked(|| {
        for tail in [
            "CAST i64\nRET",
            "NOT\nRET",
            "CONSOLE_WRITE\nPUSH_CONST 0\nRET",
            "LEN\nRET",
            "NEW_ARRAY_LITERAL 1 i64\nLEN\nRET",
            "NEW_ARRAY_LITERAL 1 text\nLEN\nRET",
            "PUSH_CONST 0\nGET_INDEX\nRET",
            "PUSH_CONST_STR A\nSTR_CONCAT\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST 0\nADD\nRET",
            "RET",
            "JMP_IF end\nLABEL end\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(
                validate(&authored(helpers, &format!("PUSH_FN_PTR first\n{tail}"))),
                Err(error::syntax),
                "{tail}"
            );
        }
        assert_eq!(
            validate(&authored(
                helpers,
                "PUSH_FN_PTR first\nPOP\nPUSH_CONST 0\nRET"
            )),
            Ok(())
        );
        for signature in [
            "fn(i64)->i64",
            "fn(i64) -> i64",
            "function",
            "ptr<fn(i64)->i64>",
        ] {
            let functions = format!(
                "# declaration_line 1\nfunction consumer(value: {signature}) -> void:\nPOP\nRET\nend function\n"
            );
            assert_eq!(
                validate(&authored(&functions, "PUSH_CONST 0\nRET")),
                Err(error::unsupported)
            );
            let functions = format!(
                "# declaration_line 1\nfunction producer -> {signature}:\nPUSH_FN_PTR main\nRET\nend function\n"
            );
            assert_eq!(
                validate(&authored(&functions, "PUSH_CONST 0\nRET")),
                Err(error::unsupported)
            );
        }
    });
}

#[test]
fn callable_targets_keep_independent_frames_and_existing_signature_collisions() {
    checked(|| {
        let captured = "# declaration_line 1\nfunction captured -> i64:\nLOAD_LOCAL outer\nRET\nend function\n";
        assert_eq!(
            validate(&authored(
                captured,
                "PUSH_CONST 42\nSTORE_LOCAL outer\nPUSH_FN_PTR captured\nSTORE_LOCAL chosen\nCALL chosen 0\nRET"
            )),
            Err(error::syntax)
        );
        for functions in [
            "# declaration_line 1\nfunction println -> i64:\nPUSH_CONST 0\nRET\nend function\n",
            "# declaration_line 1\nfunction target(println: i64) -> i64:\nRET\nend function\n",
            "# declaration_line 1\nfunction target(target: i64) -> i64:\nRET\nend function\n",
        ] {
            assert_eq!(
                validate(&authored(functions, "PUSH_CONST 0\nRET")),
                Err(error::unsupported)
            );
        }
        let bad_target =
            "# declaration_line 1\nfunction unused -> i64:\nPUSH_FN_PTR main\nRET\nend function\n";
        assert_eq!(
            validate(&authored(bad_target, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
    });
}

#[test]
fn callable_targets_inherit_text_arrays_temporaries_and_preserve_caller_prefixes() {
    checked(|| {
        let functions = "# declaration_line 1\nfunction join(value: text) -> text:\nPUSH_CONST_STR A\nSTR_CONCAT\nRET\nend function\n# declaration_line 1\nfunction identity(items: array<i64>) -> array<i64>:\nRET\nend function\n";
        let source = authored(
            functions,
            "PUSH_CONST 0x29\nPUSH_FN_PTR join\nSTORE_LOCAL $ny.temp.join.0\nPUSH_CONST_STR A\nCALL $ny.temp.join.0 0001\nPOP\nPUSH_FN_PTR identity\nSTORE_LOCAL chosen\nPUSH_CONST 1\nNEW_ARRAY_LITERAL 1 i64\nCALL chosen 1\nPUSH_CONST 0\nGET_INDEX\nADD\nRET",
        );
        assert_eq!(validate(&source), Ok(()));
    });
}

#[test]
fn callable_encoding_is_atomic_and_workspace_reuse_does_not_leak_resolution_or_effects() {
    checked(|| {
        let invalid = authored(
            helpers,
            "PUSH_CONST_STR A\nSTORE_LOCAL chosen\nCALL chosen 0\nRET",
        );
        let valid = authored(
            helpers,
            "PUSH_FN_PTR answer\nSTORE_LOCAL println\nCALL println 0\nRET",
        );
        let previous = authored("", "PUSH_CONST 42\nCALL println 1\nPUSH_CONST 0\nRET");
        let mut workspace = validation_workspace::new();
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            callable::encode(invalid.as_bytes(), &mut output, &mut workspace),
            Err(error::syntax)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        assert_eq!(
            callable::encode(valid.as_bytes(), &mut output[..1], &mut workspace),
            Err(error::output)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        let image = package(&valid);
        assert_eq!(
            callable::admit(&image, 0, &mut workspace)
                .unwrap()
                .required_services(),
            0
        );
        assert_eq!(
            array::encode(valid.as_bytes(), &mut output, &mut workspace),
            Err(error::unsupported)
        );
        let length = array::encode(previous.as_bytes(), &mut output, &mut workspace).unwrap();
        assert_eq!(
            array::admit(&output[..length], 1, &mut workspace)
                .unwrap()
                .required_services(),
            1
        );
        assert_eq!(
            callable::admit(&image, 0, &mut workspace)
                .unwrap()
                .required_services(),
            0
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_callable_path_records_exact_profile_and_retains_production_intent_policy() {
    checked(|| {
        let source = edition(
            "",
            helpers,
            "PUSH_FN_PTR answer\nSTORE_LOCAL chosen\nCALL chosen 0\nRET",
        );
        let mut workspace = validation_workspace::new();
        let witness = conformance::validate_callable(source.as_bytes(), 0, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 6);
        assert_eq!(witness.intent(), None);
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.granted_services(), 0);
        assert_eq!(
            conformance::validate_array(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::unsupported)
        );
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            callable::encode(source.as_bytes(), &mut output, &mut workspace),
            Err(error::intent)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        let authored = authored(
            helpers,
            "PUSH_FN_PTR answer\nSTORE_LOCAL chosen\nCALL chosen 0\nRET",
        );
        assert_eq!(
            conformance::validate_callable(authored.as_bytes(), 0, &mut workspace)
                .unwrap()
                .intent(),
            Some("Exercise typed internal callables")
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_callable_validation_preserves_metadata_authority_and_initialization_rules() {
    checked(|| {
        for annotation in [
            "@ai_context(\"\")\n",
            "@ai_context(\"one\")\n@ai_context(\"two\")\n",
        ] {
            let source = edition(
                annotation,
                helpers,
                "PUSH_FN_PTR answer\nSTORE_LOCAL chosen\nCALL chosen 0\nRET",
            );
            let mut workspace = validation_workspace::new();
            assert_eq!(
                conformance::validate_callable(source.as_bytes(), 0, &mut workspace).err(),
                Some(error::intent)
            );
        }
        let source = edition(
            "",
            helpers,
            "CALL answer 0\nRET\nPUSH_FN_PTR answer\nSTORE_LOCAL answer",
        );
        let mut workspace = validation_workspace::new();
        assert_eq!(
            conformance::validate_callable(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::syntax)
        );
        let source = edition(
            "",
            helpers,
            "PUSH_FN_PTR answer\nSTORE_LOCAL println\nCALL println 0\nRET\nCONSOLE_WRITE",
        );
        assert_eq!(
            conformance::validate_callable(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        assert_eq!(
            conformance::validate_callable(source.as_bytes(), 1, &mut workspace)
                .unwrap()
                .required_services(),
            1
        );
        assert_eq!(
            conformance::validate_callable(source.as_bytes(), 2, &mut workspace).err(),
            Some(error::ceiling)
        );
    });
}
