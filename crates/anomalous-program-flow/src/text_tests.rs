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
        "@ai_context(\"Exercise bounded computed text\")\n",
        helpers,
        body,
    )
}

fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let length = text::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(length);
    output
}

fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    text::encode(source.as_bytes(), &mut output, &mut workspace).map(|_| ())
}

#[test]
fn text_profile_changes_only_the_two_explicit_tags_for_older_syntax() {
    checked(|| {
        let source = authored("", "PUSH_CONST 42\nRET");
        let image = package(&source);
        let mut scalar_image = vec![0; maximum_image_bytes];
        let mut workspace = validation_workspace::new();
        let length = scalar::encode(source.as_bytes(), &mut scalar_image, &mut workspace).unwrap();
        scalar_image.truncate(length);
        assert_eq!(text::derivation_profile, 4);
        assert_eq!(get32(&image, 12), 4);
        assert_eq!(get32(&image, image_header_bytes + 12), 4);
        put32(&mut scalar_image, 12, 4);
        put32(&mut scalar_image, image_header_bytes + 12, 4);
        assert_eq!(image, scalar_image);
        let witness = text::admit(&image, 1, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 4);
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.intent(), "Exercise bounded computed text");
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.granted_services(), 0);
    });
}

#[test]
fn both_headers_require_profile_four_and_older_entry_points_refuse_it() {
    checked(|| {
        let source = authored("", "PUSH_CONST 0\nRET");
        let image = package(&source);
        for outer in [0, 1, 2, 3, 4, 5, u32::MAX] {
            for inner in [0, 1, 2, 3, 4, 5, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                if outer == 4 && inner == 4 {
                    assert_eq!(
                        text::admit(&changed, 0, &mut workspace).unwrap().profile(),
                        4
                    );
                } else {
                    assert_eq!(
                        text::admit(&changed, 0, &mut workspace).err(),
                        Some(error::profile)
                    );
                }
            }
        }
        let mut workspace = validation_workspace::new();
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
fn prior_profiles_keep_extension_refusals_after_retagging() {
    checked(|| {
        for (body, profile_one_error) in [
            ("PUSH_CONST 0x2A\nRET", error::unsupported),
            (
                "PUSH_CONST_STR A\nPUSH_CONST_STR B\nSTR_CONCAT\nPOP\nPUSH_CONST 0\nRET",
                error::unsupported,
            ),
            // Profile 1 rejects every instruction after RET before inspecting
            // its opcode; profiles 2 onward retain dead syntax checking.
            ("PUSH_CONST 0\nRET\nSTR_CONCAT", error::syntax),
        ] {
            let source = authored("", body);
            let image = package(&source);
            let mut workspace = validation_workspace::new();
            let mut output = vec![0xa5; maximum_image_bytes];
            assert_eq!(
                scalar::encode(source.as_bytes(), &mut output, &mut workspace),
                Err(error::unsupported)
            );
            assert_eq!(
                encode(source.as_bytes(), &mut output, &mut workspace),
                Err(error::unsupported)
            );
            assert_eq!(
                anomalous_program_manifest::encode(source.as_bytes(), &mut output),
                Err(profile_one_error)
            );
            assert!(output.iter().all(|byte| *byte == 0xa5));
            for profile in [2, 3] {
                let mut changed = image.clone();
                put32(&mut changed, 12, profile);
                put32(&mut changed, image_header_bytes + 12, profile);
                let mut retagged_workspace = validation_workspace::new();
                let result = if profile == 2 {
                    admit(&changed, 0, &mut retagged_workspace)
                } else {
                    scalar::admit(&changed, 0, &mut retagged_workspace)
                };
                assert_eq!(result.err(), Some(error::unsupported));
            }
        }
    });
}

#[test]
fn hexadecimal_literals_keep_signed_i64_value_and_exact_spelling_identity() {
    checked(|| {
        for (literal, expected) in [
            ("0x0", 0),
            ("-0x0", 0),
            ("+0X0", 0),
            ("0x0000002a", 42),
            ("0XFF", 255),
            ("+0xAbCd", 43981),
            ("-0XabCD", -43981),
            ("0xE", 14),
            ("0xdeadBEEF", 3735928559),
            ("0x20000000000001", 9007199254740993),
            ("0x7fffffffffffffff", i64::MAX),
            ("+0X7FFFFFFFFFFFFFFF", i64::MAX),
            ("-0x7fffffffffffffff", -i64::MAX),
            ("-0x8000000000000000", i64::MIN),
            ("-0X0008000000000000000", i64::MIN),
        ] {
            let source = authored("", &format!("PUSH_CONST {literal}\nRET"));
            let image = package(&source);
            let mut workspace = validation_workspace::new();
            let witness = text::admit(&image, 0, &mut workspace).unwrap();
            assert_eq!(
                witness.program().instructions()[0].operation,
                operation::integer(expected),
                "{literal}"
            );
            assert_eq!(witness.edition(), source.as_bytes());
            assert_eq!(witness.identity(), sha256(source.as_bytes()));
        }
        let zeros = format!("+0x{}2A", "0".repeat(1000));
        assert_eq!(integer_literal(&zeros, parse_profile::text), Ok(42));
    });
}

#[test]
fn malformed_and_overflowing_hex_literals_are_closed_and_fallible() {
    checked(|| {
        for literal in [
            "0x",
            "-0X",
            "+0x",
            "0xg",
            "0x1g",
            "0x1_0",
            "0x1p2",
            "0x1.2",
            "0x1+",
            "0x-1",
            "0x+1",
            "++0x1",
            "--0x1",
            "+-0x1",
            "-+0x1",
            "0xＦ",
            "0x١",
            "0x1\u{a0}",
            "0x 1",
            "+ 0x1",
            "0b10",
            "0o10",
        ] {
            assert_eq!(
                validate(&authored("", &format!("PUSH_CONST {literal}\nRET"))),
                Err(error::unsupported),
                "{literal}"
            );
        }
        for literal in [
            "0x8000000000000000",
            "+0X8000000000000000",
            "-0x8000000000000001",
            "0xffffffffffffffff",
            "0x10000000000000000",
        ] {
            assert_eq!(
                validate(&authored("", &format!("PUSH_CONST {literal}\nRET"))),
                Err(error::syntax),
                "{literal}"
            );
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST 0\nRET\nPUSH_CONST {literal}")
                )),
                Err(error::syntax),
                "dead {literal}"
            );
        }
    });
}

#[test]
fn decimal_spelling_and_refusals_are_identical_in_every_flow_profile() {
    for literal in [
        "0",
        "-0",
        "00042",
        "-00042",
        "9223372036854775807",
        "-9223372036854775808",
        "+1",
        "1e2",
        "1.0",
        "9223372036854775808",
        "-9223372036854775809",
        "",
        "-",
    ] {
        for profile in [
            parse_profile::flow,
            parse_profile::scalar,
            parse_profile::text,
        ] {
            assert_eq!(
                integer_literal(literal, profile),
                decimal(literal),
                "{literal}"
            );
        }
    }
}

#[test]
fn text_profile_retains_scalar_temporaries_and_bare_integer_add() {
    checked(|| {
        let source = authored(
            "",
            "PUSH_CONST 0x28\nSTORE_LOCAL $ny.temp.idx.0\nLOAD_LOCAL $ny.temp.idx.0\nPUSH_CONST 2\nADD\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = text::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(witness.program().functions()[0].local_count, 1);
        assert_eq!(
            witness.program().instructions()[4].operation,
            operation::binary {
                opcode: "ADD",
                width: "i64"
            }
        );
        for body in [
            "PUSH_CONST_STR A\nPUSH_CONST_STR B\nADD\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST 1\nSTORE_LOCAL $ny.temp.query.source.0\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(validate(&authored("", body)), Err(error::syntax));
        }
    });
}

#[test]
fn concat_returns_text_through_helpers_locals_and_stack_joins() {
    checked(|| {
        let helper = "# declaration_line 1\nfunction join(left: text, right: text) -> text:\nSTORE_LOCAL right\nSTORE_LOCAL left\nLOAD_LOCAL left\nLOAD_LOCAL right\nSTR_CONCAT\nRET\nend function\n";
        let body = "PUSH_CONST 0\nJMP_IF_NOT alternative\nPUSH_CONST_STR A\nPUSH_CONST_STR B\nCALL join 2\nJMP ready\nLABEL alternative\nPUSH_CONST_STR B\nPUSH_CONST_STR A\nSTR_CONCAT\nLABEL ready\nSTORE_LOCAL joined\nLOAD_LOCAL joined\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET";
        let source = authored(helper, body);
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = text::admit(&image, authority::console_write, &mut workspace).unwrap();
        assert_eq!(witness.required_services(), authority::console_write);
        assert_eq!(
            witness.program().functions()[0].return_kind,
            Some(kind::text)
        );
        assert_eq!(
            witness
                .program()
                .instructions()
                .iter()
                .filter(|instruction| instruction.operation == operation::concat)
                .count(),
            2
        );
    });
}

#[test]
fn reachable_concat_requires_two_text_values_and_preserves_result_kind() {
    checked(|| {
        for body in [
            "STR_CONCAT\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST_STR A\nSTR_CONCAT\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST 1\nPUSH_CONST_STR A\nSTR_CONCAT\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST_STR A\nPUSH_CONST 1\nSTR_CONCAT\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST 1\nPUSH_CONST 2\nSTR_CONCAT\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST_STR A\nPUSH_CONST_STR B\nSTR_CONCAT\nRET",
            "PUSH_CONST_STR A\nPUSH_CONST_STR B\nSTR_CONCAT\nCAST i64\nRET",
            "PUSH_CONST 0\nJMP_IF_NOT alternate\nPUSH_CONST_STR A\nPUSH_CONST_STR B\nSTR_CONCAT\nJMP joined\nLABEL alternate\nPUSH_CONST 1\nLABEL joined\nPOP\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(validate(&authored("", body)), Err(error::syntax), "{body}");
        }
    });
}

#[test]
fn concat_has_one_closed_instruction_form_and_is_not_an_integer_alias() {
    checked(|| {
        for instruction in [
            "STR_CONCAT text",
            "STR_CONCAT 2",
            "STR_CONCAT A B",
            "STR_CONCAT i64",
            "TO_STRING",
            "STR_CONCAT extra extra extra extra extra",
        ] {
            for body in [
                format!(
                    "PUSH_CONST_STR A\nPUSH_CONST_STR B\n{instruction}\nPOP\nPUSH_CONST 0\nRET"
                ),
                format!("PUSH_CONST 0\nRET\n{instruction}"),
            ] {
                assert_eq!(
                    validate(&authored("", &body)),
                    Err(error::unsupported),
                    "{instruction}"
                );
            }
        }
    });
}

#[test]
fn dead_concat_retains_complete_effect_reference_and_syntax_checks() {
    checked(|| {
        assert_eq!(
            validate(&authored("", "PUSH_CONST 0\nRET\nSTR_CONCAT")),
            Ok(())
        );
        for (body, expected) in [
            (
                "PUSH_CONST 0\nRET\nSTR_CONCAT\nINTEROP_CALL libc puts 1 i64",
                error::unsupported,
            ),
            (
                "PUSH_CONST 0\nRET\nSTR_CONCAT\nCALL absent 0",
                error::unsupported,
            ),
            ("PUSH_CONST 0\nRET\nSTR_CONCAT\nJMP absent", error::syntax),
        ] {
            assert_eq!(validate(&authored("", body)), Err(expected));
        }
        let image = package(&authored(
            "",
            "PUSH_CONST 0\nRET\nSTR_CONCAT\nCONSOLE_WRITE",
        ));
        let mut workspace = validation_workspace::new();
        assert_eq!(
            text::admit(&image, 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        assert_eq!(
            text::admit(&image, 1, &mut workspace)
                .unwrap()
                .required_services(),
            1
        );
        let invalid_helper = "# declaration_line 1\nfunction unused -> text:\nPUSH_CONST 1\nPUSH_CONST_STR A\nSTR_CONCAT\nRET\nend function\n";
        assert_eq!(
            validate(&authored(invalid_helper, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
    });
}

#[test]
fn text_encoding_refuses_atomically_and_keeps_existing_structural_limits() {
    checked(|| {
        let mut workspace = validation_workspace::new();
        let mut output = vec![0xa5; maximum_image_bytes];
        let bad = authored("", "PUSH_CONST_STR A\nSTR_CONCAT\nPOP\nPUSH_CONST 0\nRET");
        assert_eq!(
            text::encode(bad.as_bytes(), &mut output, &mut workspace),
            Err(error::syntax)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        let valid = authored(
            "",
            "PUSH_CONST_STR A\nPUSH_CONST_STR B\nSTR_CONCAT\nPOP\nPUSH_CONST 0\nRET",
        );
        assert_eq!(
            text::encode(valid.as_bytes(), &mut output[..1], &mut workspace),
            Err(error::output)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        for count in [maximum_instructions, maximum_instructions + 1] {
            let body = format!(
                "PUSH_CONST_STR A\nPUSH_CONST_STR B\nSTR_CONCAT\nPOP\n{}PUSH_CONST 0\nRET",
                "NOP\n".repeat(count - 6)
            );
            assert_eq!(
                validate(&authored("", &body)),
                if count == maximum_instructions {
                    Ok(())
                } else {
                    Err(error::limit)
                }
            );
        }
        let oversized = format!("{valid}{}", "\n".repeat(maximum_edition_bytes));
        assert_eq!(validate(&oversized), Err(error::edition));
    });
}

#[test]
fn validation_workspace_reuse_cannot_carry_text_grammar_or_authority_to_scalar() {
    checked(|| {
        let extension = authored(
            "",
            "PUSH_CONST_STR A\nPUSH_CONST_STR B\nSTR_CONCAT\nCONSOLE_WRITE\nPUSH_CONST 0x2A\nRET",
        );
        let pure = authored("", "PUSH_CONST 42\nRET");
        let image = package(&extension);
        let mut workspace = validation_workspace::new();
        let mut output = vec![0; maximum_image_bytes];
        assert_eq!(
            text::admit(&image, 1, &mut workspace)
                .unwrap()
                .required_services(),
            1
        );
        assert_eq!(
            scalar::encode(extension.as_bytes(), &mut output, &mut workspace),
            Err(error::unsupported)
        );
        let length = scalar::encode(pure.as_bytes(), &mut output, &mut workspace).unwrap();
        let witness = scalar::admit(&output[..length], 1, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 3);
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.granted_services(), 0);
        assert_eq!(
            text::admit(&image, 0, &mut workspace).err(),
            Some(error::ceiling)
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_text_witness_records_its_selected_profile_without_production_admission() {
    checked(|| {
        let source = edition(
            "",
            "",
            "PUSH_CONST_STR A\nPUSH_CONST_STR B\nSTR_CONCAT\nPOP\nPUSH_CONST 0x2A\nRET",
        );
        let mut workspace = validation_workspace::new();
        let checked = conformance::validate_text(source.as_bytes(), 1, &mut workspace).unwrap();
        assert_eq!(checked.profile(), 4);
        assert_eq!(checked.intent(), None);
        assert_eq!(checked.edition(), source.as_bytes());
        assert_eq!(checked.identity(), sha256(source.as_bytes()));
        assert_eq!(checked.required_services(), 0);
        assert_eq!(checked.granted_services(), 0);
        assert_eq!(
            conformance::validate_scalar(source.as_bytes(), 1, &mut workspace).err(),
            Some(error::unsupported)
        );
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            text::encode(source.as_bytes(), &mut output, &mut workspace),
            Err(error::intent)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        let scalar_source = edition("", "", "PUSH_CONST 42\nRET");
        assert_eq!(
            conformance::validate_scalar(scalar_source.as_bytes(), 0, &mut workspace)
                .unwrap()
                .profile(),
            3
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_text_keeps_strict_present_metadata_effects_and_control_flow() {
    checked(|| {
        let valid_body =
            "PUSH_CONST_STR A\nPUSH_CONST_STR B\nSTR_CONCAT\nCONSOLE_WRITE\nPUSH_CONST 0x0\nRET";
        let source = edition("", "", valid_body);
        let mut workspace = validation_workspace::new();
        assert_eq!(
            conformance::validate_text(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        assert_eq!(
            conformance::validate_text(source.as_bytes(), 2, &mut workspace).err(),
            Some(error::ceiling)
        );
        assert_eq!(
            conformance::validate_text(source.as_bytes(), 1, &mut workspace)
                .unwrap()
                .required_services(),
            1
        );
        for (annotation, expected) in [
            ("@ai_context(\"\")\n", error::intent),
            ("@other(\"unknown\")\n", error::unsupported),
            (
                "@ai_context(\"first\")\n@ai_context(\"second\")\n",
                error::intent,
            ),
        ] {
            let source = edition(annotation, "", valid_body);
            let mut workspace = validation_workspace::new();
            assert_eq!(
                conformance::validate_text(source.as_bytes(), 1, &mut workspace).err(),
                Some(expected)
            );
        }
        let invalid = edition(
            "",
            "",
            "PUSH_CONST_STR A\nPUSH_CONST 1\nSTR_CONCAT\nPOP\nPUSH_CONST 0\nRET",
        );
        let mut workspace = validation_workspace::new();
        assert_eq!(
            conformance::validate_text(invalid.as_bytes(), 0, &mut workspace).err(),
            Some(error::syntax)
        );
    });
}
