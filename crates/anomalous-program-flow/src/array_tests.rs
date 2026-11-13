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

fn edition(annotation: &str, helpers: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n# String constants\nconst A = \"left\"\nconst B = \"right\"\n{helpers}{annotation}# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn authored(helpers: &str, body: &str) -> String {
    edition(
        "@ai_context(\"Exercise bounded immutable arrays\")\n",
        helpers,
        body,
    )
}

fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let length = array::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(length);
    output
}

fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    array::encode(source.as_bytes(), &mut output, &mut workspace).map(|_| ())
}

#[test]
fn array_profile_changes_only_the_two_explicit_tags_for_older_syntax() {
    checked(|| {
        let source = authored("", "PUSH_CONST 42\nRET");
        let image = package(&source);
        let mut previous = vec![0; maximum_image_bytes];
        let mut workspace = validation_workspace::new();
        let length = text::encode(source.as_bytes(), &mut previous, &mut workspace).unwrap();
        previous.truncate(length);
        assert_eq!(array::derivation_profile, 5);
        assert_eq!(get32(&image, 12), 5);
        assert_eq!(get32(&image, image_header_bytes + 12), 5);
        put32(&mut previous, 12, 5);
        put32(&mut previous, image_header_bytes + 12, 5);
        assert_eq!(image, previous);
        let witness = array::admit(&image, 1, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 5);
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.intent(), "Exercise bounded immutable arrays");
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.granted_services(), 0);
    });
}

#[test]
fn both_headers_require_profile_five_and_every_older_entry_point_refuses_it() {
    checked(|| {
        let image = package(&authored("", "PUSH_CONST 0\nRET"));
        for outer in [0, 1, 2, 3, 4, 5, 6, u32::MAX] {
            for inner in [0, 1, 2, 3, 4, 5, 6, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                if outer == 5 && inner == 5 {
                    assert_eq!(
                        array::admit(&changed, 0, &mut workspace).unwrap().profile(),
                        5
                    );
                } else {
                    assert_eq!(
                        array::admit(&changed, 0, &mut workspace).err(),
                        Some(error::profile)
                    );
                }
            }
        }
        let mut workspace = validation_workspace::new();
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
fn older_profiles_refuse_live_and_dead_array_forms_even_when_retagged() {
    checked(|| {
        for (body, profile_one_error) in [
            ("NEW_ARRAY_LITERAL 0 i64\nLEN\nRET", error::unsupported),
            (
                "NEW_ARRAY_LITERAL 0 text\nPUSH_CONST 0\nGET_INDEX\nPOP\nPUSH_CONST 0\nRET",
                error::unsupported,
            ),
            ("PUSH_CONST 0\nRET\nNEW_ARRAY_LITERAL 0 i64", error::syntax),
            ("PUSH_CONST 0\nRET\nLEN", error::syntax),
            ("PUSH_CONST 0\nRET\nGET_INDEX", error::syntax),
        ] {
            let source = authored("", body);
            let image = package(&source);
            let mut workspace = validation_workspace::new();
            let mut output = vec![0xa5; maximum_image_bytes];
            assert_eq!(
                anomalous_program_manifest::encode(source.as_bytes(), &mut output),
                Err(profile_one_error)
            );
            assert_eq!(
                encode(source.as_bytes(), &mut output, &mut workspace),
                Err(error::unsupported)
            );
            assert_eq!(
                scalar::encode(source.as_bytes(), &mut output, &mut workspace),
                Err(error::unsupported)
            );
            assert_eq!(
                text::encode(source.as_bytes(), &mut output, &mut workspace),
                Err(error::unsupported)
            );
            assert!(output.iter().all(|byte| *byte == 0xa5));
            for profile in [2, 3, 4] {
                let mut changed = image.clone();
                put32(&mut changed, 12, profile);
                put32(&mut changed, image_header_bytes + 12, profile);
                let mut workspace = validation_workspace::new();
                let result = match profile {
                    2 => admit(&changed, 0, &mut workspace),
                    3 => scalar::admit(&changed, 0, &mut workspace),
                    _ => text::admit(&changed, 0, &mut workspace),
                };
                assert_eq!(result.err(), Some(error::unsupported));
            }
        }
    });
}

#[test]
fn array_plan_preserves_element_types_counts_and_exact_operations() {
    checked(|| {
        let source = authored(
            "",
            "PUSH_CONST 17\nPUSH_CONST 29\nNEW_ARRAY_LITERAL 2 i64\nLEN\nPOP\nPUSH_CONST_STR A\nPUSH_CONST_STR B\nNEW_ARRAY_LITERAL 2 text\nPUSH_CONST 1\nGET_INDEX\nPOP\nPUSH_CONST 0\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = array::admit(&image, 0, &mut workspace).unwrap();
        let instructions = witness.program().instructions();
        assert_eq!(instructions[0].operation, operation::integer(17));
        assert_eq!(instructions[1].operation, operation::integer(29));
        assert_eq!(
            instructions[2].operation,
            operation::array_literal {
                count: 2,
                element: kind::integer
            }
        );
        assert_eq!(instructions[3].operation, operation::array_len);
        assert_eq!(
            instructions[7].operation,
            operation::array_literal {
                count: 2,
                element: kind::text
            }
        );
        assert_eq!(instructions[9].operation, operation::array_get);
    });
}

#[test]
fn literal_count_is_bounded_even_in_dead_code_and_zero_is_typed() {
    checked(|| {
        for element in ["i64", "text"] {
            for count in ["0", "000"] {
                assert_eq!(
                    validate(&authored(
                        "",
                        &format!("NEW_ARRAY_LITERAL {count} {element}\nLEN\nRET")
                    )),
                    Ok(())
                );
            }
            let operand = if element == "i64" {
                "PUSH_CONST 42"
            } else {
                "PUSH_CONST_STR A"
            };
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("{operand}\nNEW_ARRAY_LITERAL 0001 {element}\nLEN\nRET")
                )),
                Ok(())
            );
        }
        let full = format!(
            "{}NEW_ARRAY_LITERAL 256 i64\nLEN\nRET",
            "PUSH_CONST 1\n".repeat(maximum_array_literal_values)
        );
        assert_eq!(maximum_array_literal_values, 256);
        assert_eq!(validate(&authored("", &full)), Ok(()));
        for count in ["257", "1024"] {
            for prefix in ["", "PUSH_CONST 0\nRET\n"] {
                assert_eq!(
                    validate(&authored(
                        "",
                        &format!("{prefix}NEW_ARRAY_LITERAL {count} i64\nLEN\nRET")
                    )),
                    Err(error::limit)
                );
            }
        }
        for count in ["-1", "+1", "0x1", "1.0", "184467440737095516160"] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST 0\nRET\nNEW_ARRAY_LITERAL {count} i64")
                )),
                Err(error::syntax)
            );
        }
        let overflow_stack = format!(
            "{}NEW_ARRAY_LITERAL 0 text\nPOP\nNEW_ARRAY_LITERAL 256 i64\nLEN\nRET",
            "PUSH_CONST 1\n".repeat(maximum_stack_values)
        );
        assert_eq!(validate(&authored("", &overflow_stack)), Err(error::limit));
    });
}

#[test]
fn literal_requires_exact_homogeneous_scalars_without_consuming_the_stack_prefix() {
    checked(|| {
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 41\nPUSH_CONST_STR A\nNEW_ARRAY_LITERAL 1 text\nLEN\nADD\nRET"
            )),
            Ok(())
        );
        for body in [
            "NEW_ARRAY_LITERAL 1 i64\nLEN\nRET",
            "PUSH_CONST 1\nNEW_ARRAY_LITERAL 2 i64\nLEN\nRET",
            "PUSH_CONST_STR A\nNEW_ARRAY_LITERAL 1 i64\nLEN\nRET",
            "PUSH_CONST 1\nNEW_ARRAY_LITERAL 1 text\nLEN\nRET",
            "PUSH_CONST 1\nPUSH_CONST_STR A\nNEW_ARRAY_LITERAL 2 i64\nLEN\nRET",
            "PUSH_CONST_STR A\nPUSH_CONST 1\nNEW_ARRAY_LITERAL 2 text\nLEN\nRET",
            "NEW_ARRAY_LITERAL 0 text\nNEW_ARRAY_LITERAL 1 text\nLEN\nRET",
            "NEW_ARRAY_LITERAL 0 i64\nNEW_ARRAY_LITERAL 1 i64\nLEN\nRET",
        ] {
            assert_eq!(validate(&authored("", body)), Err(error::syntax), "{body}");
        }
    });
}

#[test]
fn array_instruction_forms_and_element_types_are_closed() {
    checked(|| {
        for instruction in [
            "NEW_ARRAY_LITERAL",
            "NEW_ARRAY_LITERAL 0",
            "NEW_ARRAY_LITERAL 0 i64 extra",
            "NEW_ARRAY_LITERAL 0 i32",
            "NEW_ARRAY_LITERAL 0 u8",
            "NEW_ARRAY_LITERAL 0 bool",
            "NEW_ARRAY_LITERAL 0 array<i64>",
            "NEW_ARRAY_LITERAL 0 ptr<i64>",
            "NEW_ARRAY_LITERAL 0 anything",
            "NEW_ARRAY_LITERAL 0 Text",
            "LEN i64",
            "LEN 1",
            "GET_INDEX 0",
            "GET_INDEX i64",
            "NEW_ARRAY 0",
            "SET_INDEX",
            "FREE",
            "ARRAY_PUSH",
        ] {
            assert_eq!(
                validate(&authored("", &format!("PUSH_CONST 0\nRET\n{instruction}"))),
                Err(error::unsupported),
                "{instruction}"
            );
        }
    });
}

#[test]
fn array_reads_require_array_then_integer_and_produce_the_element_kind() {
    checked(|| {
        for index in ["-1", "0", "9223372036854775807"] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("NEW_ARRAY_LITERAL 0 i64\nPUSH_CONST {index}\nGET_INDEX\nRET")
                )),
                Ok(())
            );
            assert_eq!(
                validate(&authored(
                    "",
                    &format!(
                        "NEW_ARRAY_LITERAL 0 text\nPUSH_CONST {index}\nGET_INDEX\nCONSOLE_WRITE\nPUSH_CONST 0\nRET"
                    )
                )),
                Ok(())
            );
        }
        for body in [
            "LEN\nRET",
            "PUSH_CONST 1\nLEN\nRET",
            "PUSH_CONST_STR A\nLEN\nRET",
            "GET_INDEX\nRET",
            "NEW_ARRAY_LITERAL 0 i64\nGET_INDEX\nRET",
            "PUSH_CONST 1\nPUSH_CONST 0\nGET_INDEX\nRET",
            "PUSH_CONST_STR A\nPUSH_CONST 0\nGET_INDEX\nRET",
            "PUSH_CONST 0\nNEW_ARRAY_LITERAL 0 i64\nGET_INDEX\nRET",
            "NEW_ARRAY_LITERAL 0 i64\nPUSH_CONST_STR A\nGET_INDEX\nRET",
            "NEW_ARRAY_LITERAL 0 text\nPUSH_CONST 0\nGET_INDEX\nRET",
            "NEW_ARRAY_LITERAL 0 i64\nPUSH_CONST 0\nGET_INDEX\nPUSH_CONST_STR A\nSTR_CONCAT\nPOP\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(validate(&authored("", body)), Err(error::syntax), "{body}");
        }
    });
}

#[test]
fn arrays_do_not_pass_as_scalar_arithmetic_conditions_casts_or_console_payloads() {
    checked(|| {
        for operation in [
            "CAST i64",
            "NEG i64",
            "NOT",
            "CONSOLE_WRITE",
            "CONSOLE_WRITELN",
            "CALL println 1",
            "INTEROP_CALL intrinsic console.write 1 void",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("NEW_ARRAY_LITERAL 0 i64\n{operation}\nPUSH_CONST 0\nRET")
                )),
                Err(error::syntax),
                "{operation}"
            );
        }
        for operation in [
            "ADD",
            "ADD i64",
            "SUB i64",
            "CMP_EQ",
            "CMP_NE",
            "STR_CONCAT",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!(
                        "NEW_ARRAY_LITERAL 0 text\nNEW_ARRAY_LITERAL 0 text\n{operation}\nPOP\nPUSH_CONST 0\nRET"
                    )
                )),
                Err(error::syntax),
                "{operation}"
            );
        }
        assert_eq!(
            validate(&authored(
                "",
                "NEW_ARRAY_LITERAL 0 i64\nJMP_IF ready\nLABEL ready\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                "",
                "NEW_ARRAY_LITERAL 0 text\nPOP\nPUSH_CONST 0\nRET"
            )),
            Ok(())
        );
    });
}

#[test]
fn typed_arrays_flow_through_parameters_returns_locals_and_matching_joins() {
    checked(|| {
        let helpers = "# declaration_line 1\nfunction identity(items: array<text>) -> array<text>:\nSTORE_LOCAL items\nLOAD_LOCAL items\nRET\nend function\n# declaration_line 1\nfunction integers -> array<i64>:\nPUSH_CONST 41\nNEW_ARRAY_LITERAL 1 i64\nRET\nend function\n";
        let source = authored(
            helpers,
            "PUSH_CONST 0\nJMP_IF_NOT alternate\nPUSH_CONST_STR A\nNEW_ARRAY_LITERAL 1 text\nCALL identity 1\nJMP ready\nLABEL alternate\nNEW_ARRAY_LITERAL 0 text\nLABEL ready\nSTORE_LOCAL $ny.temp.items.0\nLOAD_LOCAL $ny.temp.items.0\nLEN\nPOP\nCALL integers 0\nPUSH_CONST 0\nGET_INDEX\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = array::admit(&image, 0, &mut workspace).unwrap();
        let functions = witness.program().functions();
        assert_eq!(functions[0].parameter_kinds(), &[kind::array_text]);
        assert_eq!(functions[0].return_kind, Some(kind::array_text));
        assert_eq!(functions[1].return_kind, Some(kind::array_integer));
        let mixed = "# declaration_line 1\nfunction mixed(number: i64, items: array<text>) -> i64:\nSTORE_LOCAL items\nSTORE_LOCAL number\nLOAD_LOCAL number\nLOAD_LOCAL items\nLEN\nADD\nRET\nend function\n";
        assert_eq!(
            validate(&authored(
                mixed,
                "PUSH_CONST 41\nNEW_ARRAY_LITERAL 0 text\nCALL mixed 2\nRET"
            )),
            Ok(())
        );
        assert_eq!(
            validate(&authored(
                mixed,
                "NEW_ARRAY_LITERAL 0 text\nPUSH_CONST 41\nCALL mixed 2\nRET"
            )),
            Err(error::syntax)
        );
    });
}

#[test]
fn array_signature_types_are_exact_profile_five_only_and_main_stays_integer() {
    checked(|| {
        for spelling in [
            "array<i32>",
            "array<bool>",
            "array<array<i64>>",
            "ptr<i64>",
            "vec<i64>",
            "i64[]",
            "[i64]",
            "array< i64 >",
        ] {
            let helper = format!(
                "# declaration_line 1\nfunction helper(items: {spelling}) -> void:\nPOP\nRET\nend function\n"
            );
            assert_eq!(
                validate(&authored(&helper, "PUSH_CONST 0\nRET")),
                Err(error::unsupported),
                "{spelling}"
            );
            let helper = format!(
                "# declaration_line 1\nfunction helper -> {spelling}:\nNEW_ARRAY_LITERAL 0 i64\nRET\nend function\n"
            );
            assert_eq!(
                validate(&authored(&helper, "PUSH_CONST 0\nRET")),
                Err(error::unsupported),
                "{spelling}"
            );
        }
        let helper = "# declaration_line 1\nfunction helper(items: array<i64>) -> array<i64>:\nRET\nend function\n";
        let source = authored(helper, "PUSH_CONST 0\nRET");
        assert_eq!(validate(&source), Ok(()));
        let mut workspace = validation_workspace::new();
        let mut output = vec![0; maximum_image_bytes];
        assert_eq!(
            encode(source.as_bytes(), &mut output, &mut workspace),
            Err(error::unsupported)
        );
        assert_eq!(
            scalar::encode(source.as_bytes(), &mut output, &mut workspace),
            Err(error::unsupported)
        );
        assert_eq!(
            text::encode(source.as_bytes(), &mut output, &mut workspace),
            Err(error::unsupported)
        );
        for kind in ["array<i64>", "array<text>"] {
            let changed = authored("", "NEW_ARRAY_LITERAL 0 i64\nRET").replace(
                "function main -> i64:",
                &format!("function main -> {kind}:"),
            );
            assert_eq!(validate(&changed), Err(error::intent));
        }
    });
}

#[test]
fn mismatched_array_calls_returns_locals_and_joins_refuse() {
    checked(|| {
        let helper = "# declaration_line 1\nfunction identity(items: array<i64>) -> array<i64>:\nRET\nend function\n";
        for body in [
            "PUSH_CONST 0\nCALL identity 1\nLEN\nRET",
            "NEW_ARRAY_LITERAL 0 text\nCALL identity 1\nLEN\nRET",
        ] {
            assert_eq!(validate(&authored(helper, body)), Err(error::syntax));
        }
        for result in ["i64", "array<text>"] {
            let helper = format!(
                "# declaration_line 1\nfunction unused -> {result}:\nNEW_ARRAY_LITERAL 0 i64\nRET\nend function\n"
            );
            assert_eq!(
                validate(&authored(&helper, "PUSH_CONST 0\nRET")),
                Err(error::syntax)
            );
        }
        for body in [
            "NEW_ARRAY_LITERAL 0 i64\nSTORE_LOCAL items\nNEW_ARRAY_LITERAL 0 text\nSTORE_LOCAL items\nPUSH_CONST 0\nRET",
            "PUSH_CONST 0\nJMP_IF_NOT alternate\nNEW_ARRAY_LITERAL 0 i64\nJMP ready\nLABEL alternate\nNEW_ARRAY_LITERAL 0 text\nLABEL ready\nLEN\nRET",
            "PUSH_CONST 0\nJMP_IF_NOT alternate\nNEW_ARRAY_LITERAL 0 i64\nSTORE_LOCAL items\nJMP ready\nLABEL alternate\nNEW_ARRAY_LITERAL 0 text\nSTORE_LOCAL items\nLABEL ready\nLOAD_LOCAL items\nLEN\nRET",
            "PUSH_CONST 0\nJMP_IF_NOT ready\nNEW_ARRAY_LITERAL 0 i64\nSTORE_LOCAL items\nLABEL ready\nLOAD_LOCAL items\nLEN\nRET",
            "NEW_ARRAY_LITERAL 0 i64\nLABEL again\nPOP\nNEW_ARRAY_LITERAL 0 text\nPUSH_CONST 0\nJMP_IF again\nLEN\nRET",
        ] {
            assert_eq!(validate(&authored("", body)), Err(error::syntax), "{body}");
        }
    });
}

#[test]
fn dead_array_forms_keep_whole_file_authority_and_reference_validation() {
    checked(|| {
        let image = package(&authored(
            "",
            "PUSH_CONST 0\nRET\nNEW_ARRAY_LITERAL 2 text\nLEN\nGET_INDEX\nCONSOLE_WRITE",
        ));
        let mut workspace = validation_workspace::new();
        assert_eq!(
            array::admit(&image, 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        assert_eq!(
            array::admit(&image, 1, &mut workspace)
                .unwrap()
                .required_services(),
            1
        );
        for (tail, expected) in [
            ("CALL absent 0", error::unsupported),
            ("JMP absent", error::syntax),
            ("PUSH_CONST_STR absent", error::syntax),
            ("INTEROP_CALL libc puts 1 i64", error::unsupported),
            ("SET_INDEX", error::unsupported),
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST 0\nRET\nNEW_ARRAY_LITERAL 0 text\n{tail}")
                )),
                Err(expected)
            );
        }
        let unused = "# declaration_line 1\nfunction unused -> i64:\nNEW_ARRAY_LITERAL 0 text\nCONSOLE_WRITE\nPUSH_CONST 0\nRET\nend function\n";
        assert_eq!(
            validate(&authored(unused, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
    });
}

#[test]
fn array_profile_inherits_hex_computed_text_and_scalar_temporaries() {
    checked(|| {
        let source = authored(
            "",
            "PUSH_CONST_STR A\nPUSH_CONST_STR B\nSTR_CONCAT\nNEW_ARRAY_LITERAL 1 text\nSTORE_LOCAL $ny.temp.items.0\nLOAD_LOCAL $ny.temp.items.0\nPUSH_CONST 0x0\nGET_INDEX\nCONSOLE_WRITE\nPUSH_CONST 0x29\nPUSH_CONST 1\nADD\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        assert_eq!(
            array::admit(&image, 1, &mut workspace)
                .unwrap()
                .required_services(),
            1
        );
        assert_eq!(
            text::admit(&image, 1, &mut workspace).err(),
            Some(error::profile)
        );
    });
}

#[test]
fn array_encoding_is_atomic_and_workspace_reuse_clears_profile_and_authority() {
    checked(|| {
        let invalid = authored("", "NEW_ARRAY_LITERAL 1 i64\nLEN\nRET");
        let pure = authored("", "PUSH_CONST 0\nRET");
        let extension = authored(
            "",
            "NEW_ARRAY_LITERAL 0 text\nPUSH_CONST 0\nGET_INDEX\nCONSOLE_WRITE\nPUSH_CONST 0\nRET",
        );
        let mut output = vec![0xa5; maximum_image_bytes];
        let mut workspace = validation_workspace::new();
        assert_eq!(
            array::encode(invalid.as_bytes(), &mut output, &mut workspace),
            Err(error::syntax)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        assert_eq!(
            array::encode(extension.as_bytes(), &mut output[..1], &mut workspace),
            Err(error::output)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        let image = package(&extension);
        assert_eq!(
            array::admit(&image, 1, &mut workspace)
                .unwrap()
                .required_services(),
            1
        );
        assert_eq!(
            text::encode(extension.as_bytes(), &mut output, &mut workspace),
            Err(error::unsupported)
        );
        let length = text::encode(pure.as_bytes(), &mut output, &mut workspace).unwrap();
        let witness = text::admit(&output[..length], 1, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 4);
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.granted_services(), 0);
        assert_eq!(
            array::admit(&image, 0, &mut workspace).err(),
            Some(error::ceiling)
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_array_validation_records_exact_profile_without_production_admission() {
    checked(|| {
        let source = edition(
            "",
            "",
            "PUSH_CONST 42\nNEW_ARRAY_LITERAL 1 i64\nPUSH_CONST 0\nGET_INDEX\nRET",
        );
        let mut workspace = validation_workspace::new();
        let witness = conformance::validate_array(source.as_bytes(), 1, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 5);
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.intent(), None);
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.granted_services(), 0);
        assert_eq!(
            conformance::validate_text(source.as_bytes(), 1, &mut workspace).err(),
            Some(error::unsupported)
        );
        assert_eq!(
            conformance::validate_scalar(source.as_bytes(), 1, &mut workspace).err(),
            Some(error::unsupported)
        );
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            array::encode(source.as_bytes(), &mut output, &mut workspace),
            Err(error::intent)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        let pure = edition("", "", "PUSH_CONST 42\nRET");
        assert_eq!(
            conformance::validate_text(pure.as_bytes(), 0, &mut workspace)
                .unwrap()
                .profile(),
            4
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_array_purpose_preserves_present_metadata_cfg_and_whole_file_authority() {
    checked(|| {
        for annotation in [
            "@ai_context(\"\")\n",
            "@ai_context(\"one\")\n@ai_context(\"two\")\n",
        ] {
            let source = edition(annotation, "", "NEW_ARRAY_LITERAL 0 i64\nLEN\nRET");
            let mut workspace = validation_workspace::new();
            assert_eq!(
                conformance::validate_array(source.as_bytes(), 1, &mut workspace).err(),
                Some(error::intent)
            );
        }
        let mut workspace = validation_workspace::new();
        let source = edition(
            "",
            "",
            "NEW_ARRAY_LITERAL 0 text\nCONSOLE_WRITE\nPUSH_CONST 0\nRET",
        );
        assert_eq!(
            conformance::validate_array(source.as_bytes(), 1, &mut workspace).err(),
            Some(error::syntax)
        );
        let source = edition(
            "",
            "",
            "PUSH_CONST 0\nRET\nNEW_ARRAY_LITERAL 2 text\nCONSOLE_WRITE",
        );
        assert_eq!(
            conformance::validate_array(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        let witness = conformance::validate_array(source.as_bytes(), 1, &mut workspace).unwrap();
        assert_eq!(witness.intent(), None);
        assert_eq!(witness.required_services(), 1);
        assert_eq!(witness.granted_services(), 1);
        assert_eq!(
            conformance::validate_array(source.as_bytes(), 2, &mut workspace).err(),
            Some(error::ceiling)
        );
        let source = authored("", "NEW_ARRAY_LITERAL 0 i64\nLEN\nRET");
        assert_eq!(
            conformance::validate_array(source.as_bytes(), 0, &mut workspace)
                .unwrap()
                .intent(),
            Some("Exercise bounded immutable arrays")
        );
    });
}
