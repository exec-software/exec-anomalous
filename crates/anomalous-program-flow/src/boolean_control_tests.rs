extern crate std;

use super::*;
use std::{format, string::String, vec, vec::Vec};

const intent: &str = "@ai_context(\"Exercise bounded boolean control\")\n";

fn checked(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn authored(prefix: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n# String constants\nconst text_value = \"text\"\n{prefix}{intent}# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let size = boolean_control::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(size);
    output
}

fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0xa5; maximum_image_bytes];
    let result = boolean_control::encode(source.as_bytes(), &mut output, &mut workspace);
    if result.is_err() {
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
    result.map(|_| ())
}

fn joined(name: &str, left: &str, right: &str, tail: &str) -> String {
    format!(
        "PUSH_CONST 0\nJMP_IF other\n{left}\nSTORE_LOCAL {name}\nJMP joined\nLABEL other\n{right}\nSTORE_LOCAL {name}\nLABEL joined\n{tail}"
    )
}

#[test]
fn boolean_control_inherits_storage_and_changes_only_envelope_tags_for_old_source() {
    checked(|| {
        let source = authored(
            "",
            "PUSH_CONST 1\nALLOC i64\nSTORE_LOCAL $ny.local.0.buffer\nLOAD_LOCAL $ny.local.0.buffer\nPUSH_CONST 0\nPUSH_CONST 42\nSET_INDEX\nLOAD_LOCAL $ny.local.0.buffer\nPUSH_CONST 0\nGET_INDEX\nCALL println 1\nLOAD_LOCAL $ny.local.0.buffer\nFREE\nNEW_ARRAY_LITERAL 0 i64\nPUSH_CONST 1\nCALL array__push 2\nPUSH_CONST 0\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let mut old = vec![0; maximum_image_bytes];
        let size = indexed_storage::encode(source.as_bytes(), &mut old, &mut workspace).unwrap();
        old.truncate(size);
        assert_eq!(boolean_control::derivation_profile, 15);
        assert_eq!(get32(&image, 12), 15);
        assert_eq!(get32(&image, image_header_bytes + 12), 15);
        put32(&mut old, 12, 15);
        put32(&mut old, image_header_bytes + 12, 15);
        assert_eq!(image, old);
        let admitted = boolean_control::admit(&image, 1, &mut workspace).unwrap();
        assert_eq!(admitted.profile(), 15);
        assert_eq!(admitted.identity(), sha256(source.as_bytes()));
        assert_eq!(admitted.required_services(), authority::console_write);
        assert!(
            admitted
                .program()
                .instructions()
                .iter()
                .any(|item| item.operation == operation::builtin_console { newline: true })
        );
    });
}

#[test]
fn boolean_control_requires_both_headers_and_preserves_all_previous_profiles() {
    checked(|| {
        let image = package(&authored("", "PUSH_CONST 0\nRET"));
        for outer in [0, 1, 2, 13, 14, 15, 16, u32::MAX] {
            for inner in [0, 1, 14, 15, 16, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                assert_eq!(
                    boolean_control::admit(&changed, 0, &mut workspace).err(),
                    if outer == 15 && inner == 15 {
                        None
                    } else {
                        Some(error::profile)
                    }
                );
            }
        }
        assert_eq!(
            anomalous_program_manifest::admit(&image, 0).err(),
            Some(error::profile)
        );
        for profile in [
            parse_profile::flow,
            parse_profile::scalar,
            parse_profile::text,
            parse_profile::array,
            parse_profile::callable,
            parse_profile::record,
            parse_profile::globals,
            parse_profile::enums,
            parse_profile::source_debug,
            parse_profile::typed_scalars,
            parse_profile::nested_records,
            parse_profile::hygienic_locals,
            parse_profile::indexed_storage,
        ] {
            let mut workspace = validation_workspace::new();
            assert_eq!(
                admit_profile(&image, 0, &mut workspace, profile).err(),
                Some(error::profile)
            );
        }
        for body in [
            "PUSH_CONST 1\nCAST bool\nNOT\nRET",
            "PUSH_CONST 1\nCAST bool\nJMP_IF done\nLABEL done\nPUSH_CONST 0\nRET",
            "PUSH_CONST 0.5\nCALL println 1\nPUSH_CONST 0\nRET",
        ] {
            let source = authored("", body);
            assert_eq!(validate(&source), Ok(()));
            let mut workspace = validation_workspace::new();
            let mut output = vec![0; maximum_image_bytes];
            assert_eq!(
                indexed_storage::encode(source.as_bytes(), &mut output, &mut workspace),
                Err(error::syntax)
            );
        }
    });
}

#[test]
fn boolean_branches_and_not_preserve_integer_results_and_exact_other_operands() {
    checked(|| {
        let helper =
            "# declaration_line 1\nfunction invert(value: bool) -> i64:\nNOT\nRET\nend function\n";
        assert_eq!(
            validate(&authored(
                helper,
                "PUSH_CONST 1\nCAST bool\nCALL invert 1\nRET"
            )),
            Ok(())
        );
        for source in ["PUSH_CONST 1", "PUSH_CONST -7", "PUSH_CONST 1\nCAST bool"] {
            for branch in ["JMP_IF", "JMP_IF_NOT"] {
                assert_eq!(
                    validate(&authored(
                        "",
                        &format!("{source}\n{branch} done\nLABEL done\nPUSH_CONST 42\nRET")
                    )),
                    Ok(())
                );
            }
            assert_eq!(
                validate(&authored("", &format!("{source}\nNOT\nRET"))),
                Ok(())
            );
        }
        let wrong_result = helper.replace("-> i64:", "-> bool:");
        assert_eq!(
            validate(&authored(&wrong_result, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
        for source in [
            "PUSH_CONST 1\nCAST u8",
            "PUSH_CONST 0.5",
            "PUSH_CONST_STR text_value",
            "NEW_ARRAY_LITERAL 0 i64",
            "PUSH_CONST 1\nALLOC i64",
        ] {
            for suffix in ["NOT\nRET", "JMP_IF done\nLABEL done\nPUSH_CONST 0\nRET"] {
                assert_eq!(
                    validate(&authored("", &format!("{source}\n{suffix}"))),
                    Err(error::syntax)
                );
            }
        }
        for suffix in [
            "NEG i64\nRET",
            "PUSH_CONST 1\nADD i64\nRET",
            "PUSH_CONST 1\nCMP_EQ\nRET",
        ] {
            assert_eq!(
                validate(&authored("", &format!("PUSH_CONST 1\nCAST bool\n{suffix}"))),
                Err(error::syntax)
            );
        }
        assert_eq!(
            validate(&authored("", "PUSH_CONST 0\nRET\nNOT bool")),
            Err(error::unsupported)
        );
    });
}

#[test]
fn mixed_local_joins_require_explicit_normalization_independent_of_name_or_predecessor_order() {
    checked(|| {
        for name in [
            "chosen",
            "$ny.temp.logical_or.1",
            "$ny.temp.logical_and.0",
            "$ny.local.0.chosen",
        ] {
            for (left, right) in [
                ("PUSH_CONST 1\nCAST bool", "PUSH_CONST 0"),
                ("PUSH_CONST 0", "PUSH_CONST 1\nCAST bool"),
                ("PUSH_CONST 1\nCAST bool", "PUSH_CONST 2"),
            ] {
                let source = authored(
                    "",
                    &joined(
                        name,
                        left,
                        right,
                        &format!("LOAD_LOCAL {name}\nCAST bool\nCAST i64\nRET"),
                    ),
                );
                assert_eq!(validate(&source), Ok(()), "{name}");
                let mut workspace = validation_workspace::new();
                let mut output = vec![0; maximum_image_bytes];
                assert_eq!(
                    indexed_storage::encode(source.as_bytes(), &mut output, &mut workspace),
                    Err(error::syntax)
                );
            }
        }
        for value in ["PUSH_CONST 1\nCAST bool", "PUSH_CONST 1"] {
            assert_eq!(
                validate(&authored(
                    "",
                    &joined("chosen", value, value, "LOAD_LOCAL chosen\nNOT\nRET")
                )),
                Ok(())
            );
        }
        for right in [
            "PUSH_CONST 1\nCAST u8",
            "PUSH_CONST 0.5",
            "PUSH_CONST_STR text_value",
            "NEW_ARRAY_LITERAL 0 i64",
            "PUSH_CONST 1\nALLOC i64",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &joined(
                        "chosen",
                        "PUSH_CONST 1\nCAST bool",
                        right,
                        "PUSH_CONST 0\nRET"
                    )
                )),
                Err(error::syntax)
            );
        }
    });
}

#[test]
fn mixed_local_reads_cannot_escape_the_adjacent_cast_pair() {
    checked(|| {
        for tail in [
            "LOAD_LOCAL chosen\nRET",
            "LOAD_LOCAL chosen\nPOP\nPUSH_CONST 0\nRET",
            "LOAD_LOCAL chosen\nNOT\nRET",
            "LOAD_LOCAL chosen\nCAST i64\nRET",
            "LOAD_LOCAL chosen\nLABEL gap\nCAST bool\nCAST i64\nRET",
            "LOAD_LOCAL chosen\nNOP\nCAST bool\nCAST i64\nRET",
            "LOAD_LOCAL chosen\nSTORE_LOCAL copied\nPUSH_CONST 0\nRET",
            "LOAD_LOCAL chosen\nJMP_IF done\nLABEL done\nPUSH_CONST 0\nRET",
            "LOAD_LOCAL chosen\nCALL println 1\nPUSH_CONST 0\nRET",
            "LOAD_LOCAL chosen\nCAST bool\nPOP\nLOAD_LOCAL chosen\nPOP\nPUSH_CONST 0\nRET",
            "CALL chosen 0\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &joined("chosen", "PUSH_CONST 1\nCAST bool", "PUSH_CONST 0", tail)
                )),
                Err(error::syntax),
                "{tail}"
            );
        }
        let tail = "LOAD_LOCAL chosen\n# source_debug 11 \"boolean.ny\" 1 \"choice\" \"chosen\" \"i64\" 0\nCAST bool\nCAST i64\nRET";
        assert_eq!(
            validate(&authored(
                "",
                &joined("chosen", "PUSH_CONST 1\nCAST bool", "PUSH_CONST 0", tail)
            )),
            Ok(())
        );
    });
}

#[test]
fn mixed_local_policy_preserves_stack_joins_initialization_and_function_signatures() {
    checked(|| {
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 0\nJMP_IF other\nPUSH_CONST 1\nCAST bool\nJMP joined\nLABEL other\nPUSH_CONST 0\nLABEL joined\nCAST bool\nCAST i64\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 0\nJMP_IF other\nPUSH_CONST 1\nCAST bool\nSTORE_LOCAL chosen\nJMP joined\nLABEL other\nNOP\nLABEL joined\nLOAD_LOCAL chosen\nCAST bool\nCAST i64\nRET"
            )),
            Err(error::syntax)
        );
        let helper =
            "# declaration_line 1\nfunction identity(value: bool) -> bool:\nRET\nend function\n";
        let tail = "LOAD_LOCAL chosen\nCAST bool\nCALL identity 1\nCAST i64\nRET";
        assert_eq!(
            validate(&authored(
                helper,
                &joined("chosen", "PUSH_CONST 1\nCAST bool", "PUSH_CONST 0", tail)
            )),
            Ok(())
        );
        assert_eq!(
            validate(&authored(
                helper,
                &joined(
                    "chosen",
                    "PUSH_CONST 1\nCAST bool",
                    "PUSH_CONST 0",
                    "LOAD_LOCAL chosen\nCALL identity 1\nCAST i64\nRET"
                )
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                "",
                &joined(
                    "chosen",
                    "PUSH_CONST 1\nCAST bool",
                    "PUSH_CONST 0",
                    "PUSH_CONST 42\nSTORE_LOCAL chosen\nLOAD_LOCAL chosen\nRET"
                )
            )),
            Ok(())
        );
    });
}

#[test]
fn loop_local_merges_converge_in_both_directions_and_block_orders() {
    checked(|| {
        for (initial, replacement) in [
            ("PUSH_CONST 0\nCAST bool", "PUSH_CONST 1"),
            ("PUSH_CONST 0", "PUSH_CONST 1\nCAST bool"),
        ] {
            let body = format!(
                "{initial}\nSTORE_LOCAL chosen\nLABEL cycle\nLOAD_LOCAL chosen\nCAST bool\nPOP\n{replacement}\nSTORE_LOCAL chosen\nPUSH_CONST 0\nJMP_IF cycle\nPUSH_CONST 42\nRET"
            );
            assert_eq!(validate(&authored("", &body)), Ok(()));
            let body = format!(
                "{initial}\nSTORE_LOCAL chosen\nJMP header\nLABEL body\n{replacement}\nSTORE_LOCAL chosen\nJMP header\nLABEL header\nLOAD_LOCAL chosen\nCAST bool\nPOP\nPUSH_CONST 0\nJMP_IF body\nPUSH_CONST 42\nRET"
            );
            assert_eq!(validate(&authored("", &body)), Ok(()));
        }
    });
}

#[test]
fn speculative_stores_cannot_create_their_own_union_or_hide_sequential_type_changes() {
    checked(|| {
        for prefix in [
            "",
            "PUSH_CONST 0\nCAST bool\nSTORE_LOCAL chosen\nLABEL cycle\n",
        ] {
            let suffix = if prefix.is_empty() {
                ""
            } else {
                "PUSH_CONST 0\nJMP_IF cycle\n"
            };
            for (left, right) in [
                ("PUSH_CONST 0\nCAST bool", "PUSH_CONST 1"),
                ("PUSH_CONST 0", "PUSH_CONST 1\nCAST bool"),
            ] {
                let body = format!(
                    "{prefix}{left}\nSTORE_LOCAL chosen\n{right}\nSTORE_LOCAL chosen\n{suffix}PUSH_CONST 42\nRET"
                );
                assert_eq!(validate(&authored("", &body)), Err(error::syntax));
            }
        }
        let body = joined(
            "chosen",
            "PUSH_CONST 1\nCAST bool",
            "PUSH_CONST 0",
            "PUSH_CONST 1\nCAST bool\nSTORE_LOCAL chosen\nPUSH_CONST 0\nSTORE_LOCAL chosen\nPUSH_CONST 42\nRET",
        );
        assert_eq!(validate(&authored("", &body)), Err(error::syntax));
    });
}

#[test]
fn float_output_is_limited_to_existing_builtin_call_fallbacks() {
    checked(|| {
        for (name, newline) in [
            ("println", true),
            ("core__print", true),
            ("console__writeln", true),
            ("console__write", false),
            ("console__sys_write", false),
        ] {
            let source = authored(
                "",
                &format!("PUSH_CONST -0.0\nCALL {name} 1\nPUSH_CONST 0\nRET"),
            );
            let image = package(&source);
            let mut workspace = validation_workspace::new();
            let admitted = boolean_control::admit(&image, 1, &mut workspace).unwrap();
            assert_eq!(
                admitted.program().instructions()[1].operation,
                operation::builtin_console { newline }
            );
            assert_eq!(
                admitted.program().instructions()[0].operation,
                operation::float(1u64 << 63)
            );
            assert_eq!(admitted.required_services(), authority::console_write);
            assert_eq!(
                boolean_control::admit(&image, 0, &mut workspace).err(),
                Some(error::ceiling)
            );
        }
        for opcode in [
            "CONSOLE_WRITE",
            "CONSOLE_WRITELN",
            "INTEROP_CALL intrinsic console.write 1 void",
            "INTEROP_CALL intrinsic console.writeln 1 void",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST 0.5\n{opcode}\nPUSH_CONST 0\nRET")
                )),
                Err(error::syntax)
            );
            let source = authored(
                "",
                &format!("PUSH_CONST_STR text_value\n{opcode}\nPUSH_CONST 0\nRET"),
            );
            let image = package(&source);
            let mut workspace = validation_workspace::new();
            assert!(matches!(
                boolean_control::admit(&image, 1, &mut workspace)
                    .unwrap()
                    .program()
                    .instructions()[1]
                    .operation,
                operation::console { .. }
            ));
        }
        for value in [
            "PUSH_CONST 1\nCAST bool",
            "PUSH_CONST 1\nCAST u8",
            "NEW_ARRAY_LITERAL 0 i64",
            "PUSH_CONST 1\nALLOC i64",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("{value}\nCALL println 1\nPUSH_CONST 0\nRET")
                )),
                Err(error::syntax)
            );
        }
    });
}

#[test]
fn float_builtin_call_retains_local_precedence_and_exact_helper_signatures() {
    checked(|| {
        let helper =
            "# declaration_line 1\nfunction identity(value: f64) -> f64:\nRET\nend function\n";
        let source = authored(
            helper,
            "PUSH_FN_PTR identity\nSTORE_LOCAL println\nPUSH_CONST 0.5\nCALL println 1\nPOP\nPUSH_CONST 0\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let admitted = boolean_control::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(admitted.required_services(), 0);
        assert!(
            admitted
                .program()
                .instructions()
                .iter()
                .any(|item| item.operation
                    == operation::call_local {
                        local: 0,
                        arguments: 1
                    })
        );
        assert_eq!(
            validate(&authored(
                helper,
                "PUSH_CONST 0.5\nCALL identity 1\nCALL println 1\nPUSH_CONST 0\nRET"
            )),
            Ok(())
        );
        assert_eq!(
            validate(&authored(
                helper,
                "PUSH_CONST 1\nCALL identity 1\nCALL println 1\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 0.5\nCALL println 1\nPUSH_CONST 0\nRET\nPUSH_CONST 0\nSTORE_LOCAL println"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored("", "PUSH_CONST 0\nRET\nCALL println 2")),
            Err(error::unsupported)
        );
    });
}

#[test]
fn dead_builtin_effects_and_workspace_reuse_cannot_hide_authority_or_store_conflicts() {
    checked(|| {
        let invalid = authored(
            "",
            "PUSH_CONST 1\nCAST bool\nSTORE_LOCAL chosen\nPUSH_CONST 0\nSTORE_LOCAL chosen\nPUSH_CONST 0\nRET",
        );
        let valid = authored(
            "",
            "PUSH_CONST 42\nSTORE_LOCAL chosen\nLOAD_LOCAL chosen\nRET\nPUSH_CONST 0.5\nCALL println 1",
        );
        let mut workspace = validation_workspace::new();
        let mut output = vec![0; maximum_image_bytes];
        assert_eq!(
            boolean_control::encode(invalid.as_bytes(), &mut output, &mut workspace),
            Err(error::syntax)
        );
        let size = boolean_control::encode(valid.as_bytes(), &mut output, &mut workspace).unwrap();
        let image = output[..size].to_vec();
        assert_eq!(
            boolean_control::admit(&image, 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        assert_eq!(
            boolean_control::admit(&image, 1, &mut workspace)
                .unwrap()
                .required_services(),
            authority::console_write
        );
        let source = authored("", "PUSH_CONST 42\nRET");
        assert!(indexed_storage::encode(source.as_bytes(), &mut output, &mut workspace).is_ok());
        assert_eq!(
            boolean_control::encode(invalid.as_bytes(), &mut output, &mut workspace),
            Err(error::syntax)
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_boolean_control_validation_keeps_intent_authority_and_metadata_separate() {
    checked(|| {
        let source = authored("", "# source_debug 0 \"boolean.ny\" 0\nPUSH_CONST 0.5\nCALL println 1\nPUSH_CONST 1\nCAST bool\nNOT\nRET").replace(intent, "");
        assert_eq!(validate(&source), Err(error::intent));
        let mut workspace = validation_workspace::new();
        assert_eq!(
            conformance::validate_boolean_control(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        let raw =
            conformance::validate_boolean_control(source.as_bytes(), 1, &mut workspace).unwrap();
        assert_eq!(raw.profile(), 15);
        assert_eq!(raw.intent(), None);
        assert_eq!(raw.required_services(), authority::console_write);
        assert_eq!(raw.identity(), sha256(source.as_bytes()));
        assert_eq!(
            raw.program().source_debug(0).unwrap().instruction_index(),
            0
        );
        assert_eq!(
            conformance::validate_indexed_storage(source.as_bytes(), 1, &mut workspace).err(),
            Some(error::syntax)
        );
    });
}
