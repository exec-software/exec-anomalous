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

fn edition(helpers: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n# String constants\nconst S = \"text\"\n{helpers}@ai_context(\"Validate compiler scalar forms\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let length = scalar::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(length);
    output
}

fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    scalar::encode(source.as_bytes(), &mut output, &mut workspace).map(|_| ())
}

#[test]
fn canonical_scalar_image_differs_only_in_explicit_profile_tags() {
    checked(|| {
        let source = edition("", "PUSH_CONST 42\nRET");
        let scalar_image = package(&source);
        let mut flow_image = vec![0; maximum_image_bytes];
        let mut workspace = validation_workspace::new();
        let length = encode(source.as_bytes(), &mut flow_image, &mut workspace).unwrap();
        flow_image.truncate(length);
        assert_eq!(scalar::derivation_profile, 3);
        assert_eq!(derivation_profile, 2);
        assert_eq!(get32(&scalar_image, 12), 3);
        assert_eq!(get32(&scalar_image, image_header_bytes + 12), 3);
        put32(&mut flow_image, 12, 3);
        put32(&mut flow_image, image_header_bytes + 12, 3);
        assert_eq!(scalar_image, flow_image);
        let witness = scalar::admit(&scalar_image, 0, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 3);
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.intent(), "Validate compiler scalar forms");
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.granted_services(), 0);
    });
}

#[test]
fn both_headers_require_the_explicitly_selected_profile() {
    checked(|| {
        let source = edition("", "PUSH_CONST 0\nRET");
        let image = package(&source);
        for outer in [0, 1, 2, 3, 4, u32::MAX] {
            for inner in [0, 1, 2, 3, 4, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                if outer == 3 && inner == 3 {
                    assert_eq!(
                        scalar::admit(&changed, 0, &mut workspace)
                            .unwrap()
                            .profile(),
                        3
                    );
                } else {
                    assert_eq!(
                        scalar::admit(&changed, 0, &mut workspace).err(),
                        Some(error::profile)
                    );
                }
                if outer == 2 && inner == 2 {
                    assert_eq!(admit(&changed, 0, &mut workspace).unwrap().profile(), 2);
                } else {
                    assert_eq!(
                        admit(&changed, 0, &mut workspace).err(),
                        Some(error::profile)
                    );
                }
            }
        }
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
            scalar::admit(&profile_one, 0, &mut workspace).err(),
            Some(error::profile)
        );
    });
}

#[test]
fn profile_two_keeps_raw_syntax_refusals_even_after_retagging() {
    checked(|| {
        for (body, expected) in [
            (
                "PUSH_CONST 1\nSTORE_LOCAL $ny.temp.idx.0\nLOAD_LOCAL $ny.temp.idx.0\nRET",
                error::syntax,
            ),
            ("PUSH_CONST 1\nPUSH_CONST 2\nADD\nRET", error::unsupported),
            ("PUSH_CONST 0\nRET\nADD", error::unsupported),
        ] {
            let source = edition("", body);
            let image = package(&source);
            let mut retagged = image.clone();
            put32(&mut retagged, 12, 2);
            put32(&mut retagged, image_header_bytes + 12, 2);
            let mut workspace = validation_workspace::new();
            let mut output = vec![0; maximum_image_bytes];
            assert_eq!(
                encode(source.as_bytes(), &mut output, &mut workspace),
                Err(expected)
            );
            assert_eq!(admit(&image, 0, &mut workspace).err(), Some(error::profile));
            assert_eq!(admit(&retagged, 0, &mut workspace).err(), Some(expected));
        }
    });
}

#[test]
fn temporary_names_are_exact_local_keys_with_bounded_decimal_spelling() {
    checked(|| {
        let source = edition(
            "",
            "PUSH_CONST 7\nSTORE_LOCAL $ny.temp.idx.0\nPUSH_CONST 9\nSTORE_LOCAL $ny.temp.idx.1\nLOAD_LOCAL $ny.temp.idx.0\nLOAD_LOCAL $ny.temp.idx.1\nADD\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = scalar::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(witness.program().functions()[0].local_count, 2);
        let operations: Vec<_> = witness
            .program()
            .instructions()
            .iter()
            .map(|i| i.operation)
            .collect();
        assert_eq!(operations[1], operation::store(0));
        assert_eq!(operations[3], operation::store(1));
        assert_eq!(operations[4], operation::load(0));
        assert_eq!(operations[5], operation::load(1));
        for name in [
            "$ny.temp._.0",
            "$ny.temp.logical_and.17",
            "$ny.temp.idx.184467440737095516160",
        ] {
            assert_eq!(
                validate(&edition(
                    "",
                    &format!("PUSH_CONST 0\nSTORE_LOCAL {name}\nLOAD_LOCAL {name}\nRET")
                )),
                Ok(())
            );
        }
    });
}

#[test]
fn malformed_temporary_names_are_rejected_even_in_dead_code() {
    checked(|| {
        for name in [
            "$ny.temp.idx",
            "$ny.temp..0",
            "$ny.temp.idx.",
            "$ny.temp.idx.00",
            "$ny.temp.idx.01",
            "$ny.temp.idx.+1",
            "$ny.temp.idx.-1",
            "$ny.temp.idx.1x",
            "$ny.temp.1idx.0",
            "$ny.temp.query.source.0",
            "$ny.temp.enum.payload.0",
            "$ny.temp.idx.0.extra",
            "$ny.temp.índice.0",
            "$ny.temp.idx.١",
            "$ny.other.idx.0",
            "$ny.generated.idx.0",
            "$ny.temp.idx.0_",
            "$ny.temp.idx.0.",
        ] {
            for body in [
                format!("PUSH_CONST 1\nSTORE_LOCAL {name}\nLOAD_LOCAL {name}\nRET"),
                format!("PUSH_CONST 0\nRET\nSTORE_LOCAL {name}"),
                format!("PUSH_CONST 0\nRET\nLOAD_LOCAL {name}"),
            ] {
                assert_eq!(validate(&edition("", &body)), Err(error::syntax), "{name}");
            }
        }
    });
}

#[test]
fn temporary_spelling_is_not_accepted_for_other_symbols() {
    checked(|| {
        let name = "$ny.temp.idx.0";
        for body in [
            format!("LABEL {name}\nPUSH_CONST 0\nRET"),
            format!("PUSH_CONST 0\nRET\nJMP {name}"),
            format!("PUSH_CONST 0\nRET\nCALL {name} 0"),
            format!("PUSH_CONST_STR {name}\nRET"),
        ] {
            assert_eq!(validate(&edition("", &body)), Err(error::syntax));
        }
        for helpers in [
            format!(
                "# declaration_line 1\nfunction {name} -> i64:\nPUSH_CONST 0\nRET\nend function\n"
            ),
            format!(
                "# declaration_line 1\nfunction helper({name}: i64) -> i64:\nSTORE_LOCAL {name}\nLOAD_LOCAL {name}\nRET\nend function\n"
            ),
        ] {
            assert_eq!(
                validate(&edition(&helpers, "PUSH_CONST 0\nRET")),
                Err(error::syntax)
            );
        }
        let source = edition("", "PUSH_CONST 0\nRET");
        assert_eq!(
            validate(&source.replace("const S =", &format!("const {name} ="))),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&source.replace("module root", &format!("module {name}"))),
            Err(error::syntax)
        );
    });
}

#[test]
fn name_bound_includes_the_entire_temporary_and_never_parses_counter_as_integer() {
    checked(|| {
        let prefix_bound = format!("$ny.temp.{}.0", "x".repeat(maximum_name_bytes - 11));
        let counter_bound = format!("$ny.temp.x.{}", "9".repeat(maximum_name_bytes - 11));
        for name in [prefix_bound, counter_bound] {
            assert_eq!(name.len(), maximum_name_bytes);
            assert_eq!(
                validate(&edition(
                    "",
                    &format!("PUSH_CONST 1\nSTORE_LOCAL {name}\nLOAD_LOCAL {name}\nRET")
                )),
                Ok(())
            );
            let excess = format!("{name}1");
            assert_eq!(
                validate(&edition(
                    "",
                    &format!("PUSH_CONST 0\nRET\nSTORE_LOCAL {excess}")
                )),
                Err(error::limit)
            );
        }
    });
}

#[test]
fn temporary_and_ordinary_names_share_the_same_local_capacity() {
    checked(|| {
        let mut body = String::new();
        for index in 0..maximum_locals {
            let name = if index % 2 == 0 {
                format!("ordinary_{index}")
            } else {
                format!("$ny.temp.idx.{index}")
            };
            body.push_str(&format!("PUSH_CONST {index}\nSTORE_LOCAL {name}\n"));
        }
        assert_eq!(
            validate(&edition(
                "",
                &(body.clone() + "LOAD_LOCAL $ny.temp.idx.127\nRET")
            )),
            Ok(())
        );
        for extra in ["ordinary_extra", "$ny.temp.idx.128"] {
            assert_eq!(
                validate(&edition(
                    "",
                    &(body.clone()
                        + &format!("PUSH_CONST 0\nSTORE_LOCAL {extra}\nPUSH_CONST 0\nRET"))
                )),
                Err(error::limit)
            );
        }
    });
}

#[test]
fn temporary_definite_initialization_and_join_kinds_remain_strict() {
    checked(|| {
        for body in [
            "PUSH_CONST 1\nJMP_IF skip\nPUSH_CONST 7\nSTORE_LOCAL $ny.temp.idx.0\nLABEL skip\nLOAD_LOCAL $ny.temp.idx.0\nRET",
            "PUSH_CONST 1\nJMP_IF text\nPUSH_CONST 7\nSTORE_LOCAL $ny.temp.idx.0\nJMP joined\nLABEL text\nPUSH_CONST_STR S\nSTORE_LOCAL $ny.temp.idx.0\nLABEL joined\nPUSH_CONST 0\nRET",
            "PUSH_CONST 1\nSTORE_LOCAL $ny.temp.idx.0\nPUSH_CONST_STR S\nSTORE_LOCAL $ny.temp.idx.0\nPUSH_CONST 0\nRET",
            "LABEL head\nPUSH_CONST 0\nJMP_IF_NOT done\nPUSH_CONST 1\nSTORE_LOCAL $ny.temp.idx.0\nJMP head\nLABEL done\nLOAD_LOCAL $ny.temp.idx.0\nRET",
        ] {
            assert_eq!(validate(&edition("", body)), Err(error::syntax));
        }
        assert_eq!(
            validate(&edition(
                "",
                "PUSH_CONST 1\nJMP_IF other\nPUSH_CONST 7\nSTORE_LOCAL $ny.temp.logical_and.0\nJMP joined\nLABEL other\nPUSH_CONST 0\nSTORE_LOCAL $ny.temp.logical_and.0\nLABEL joined\nLOAD_LOCAL $ny.temp.logical_and.0\nRET"
            )),
            Ok(())
        );
    });
}

#[test]
fn temporary_names_remain_function_local() {
    checked(|| {
        let helpers = "# declaration_line 1\nfunction helper -> text:\nPUSH_CONST_STR S\nSTORE_LOCAL $ny.temp.idx.0\nLOAD_LOCAL $ny.temp.idx.0\nRET\nend function\n";
        let source = edition(
            helpers,
            "PUSH_CONST 5\nSTORE_LOCAL $ny.temp.idx.0\nCALL helper 0\nPOP\nLOAD_LOCAL $ny.temp.idx.0\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = scalar::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(witness.program().functions()[0].local_count, 1);
        assert_eq!(witness.program().functions()[1].local_count, 1);
        assert_eq!(
            witness.program().functions()[0].return_kind,
            Some(kind::text)
        );
    });
}

#[test]
fn bare_add_normalizes_to_existing_i64_operation_without_changing_edition() {
    checked(|| {
        for (left, right) in [(i64::MAX, 1), (i64::MIN, -1), (0, 0)] {
            let source = edition(
                "",
                &format!("PUSH_CONST {left}\nPUSH_CONST {right}\nADD\nRET"),
            );
            let image = package(&source);
            let mut workspace = validation_workspace::new();
            let witness = scalar::admit(&image, 0, &mut workspace).unwrap();
            assert_eq!(witness.edition(), source.as_bytes());
            assert_eq!(witness.identity(), sha256(source.as_bytes()));
            let instructions = witness.program().instructions();
            assert_eq!(instructions[0].operation, operation::integer(left));
            assert_eq!(instructions[1].operation, operation::integer(right));
            assert_eq!(
                instructions[2].operation,
                operation::binary {
                    opcode: "ADD",
                    width: "i64"
                }
            );
            assert_eq!(source.lines().nth(instructions[2].line - 1), Some("ADD"));
        }
    });
}

#[test]
fn reachable_bare_add_requires_two_integers_and_does_not_widen_other_operators() {
    checked(|| {
        for body in [
            "ADD\nRET",
            "PUSH_CONST 1\nADD\nRET",
            "PUSH_CONST_STR S\nPUSH_CONST 1\nADD\nRET",
            "PUSH_CONST 1\nPUSH_CONST_STR S\nADD\nRET",
            "PUSH_CONST_STR S\nPUSH_CONST_STR S\nADD\nRET",
        ] {
            assert_eq!(validate(&edition("", body)), Err(error::syntax));
        }
        for opcode in ["SUB", "MUL", "DIV", "MOD", "NEG", "BIT_AND", "SHL"] {
            assert_eq!(
                validate(&edition(
                    "",
                    &format!("PUSH_CONST 1\nPUSH_CONST 2\n{opcode}\nRET")
                )),
                Err(error::unsupported)
            );
        }
        for body in [
            "PUSH_CONST 1.5\nPUSH_CONST 2\nADD\nRET",
            "PUSH_CONST 0x10\nRET",
            "NEW_ARRAY_LITERAL 0 i64\nRET",
            "PUSH_CONST_STR S\nPUSH_CONST_STR S\nSTR_CONCAT\nRET",
        ] {
            assert!(validate(&edition("", body)).is_err());
        }
    });
}

#[test]
fn dead_add_is_syntax_only_but_live_targets_and_unused_functions_are_checked() {
    checked(|| {
        assert_eq!(validate(&edition("", "PUSH_CONST 0\nRET\nADD")), Ok(()));
        assert_eq!(
            validate(&edition(
                "",
                "JMP target\nPUSH_CONST 0\nRET\nLABEL target\nADD\nRET"
            )),
            Err(error::syntax)
        );
        let helpers = "# declaration_line 1\nfunction unused -> i64:\nPUSH_CONST_STR S\nPUSH_CONST 1\nADD\nRET\nend function\n";
        assert_eq!(
            validate(&edition(helpers, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
    });
}

#[test]
fn dead_and_unused_console_authority_still_requires_the_ceiling() {
    checked(|| {
        let helpers = "# declaration_line 1\nfunction unused -> i64:\nPUSH_CONST_STR S\nCONSOLE_WRITE\nPUSH_CONST 0\nRET\nend function\n";
        for source in [
            edition("", "PUSH_CONST 0\nRET\nADD\nCONSOLE_WRITE"),
            edition(helpers, "PUSH_CONST 0\nRET"),
        ] {
            let image = package(&source);
            let mut workspace = validation_workspace::new();
            assert_eq!(
                scalar::admit(&image, 0, &mut workspace).err(),
                Some(error::ceiling)
            );
            let witness = scalar::admit(&image, authority::console_write, &mut workspace).unwrap();
            assert_eq!(witness.required_services(), authority::console_write);
            assert_eq!(witness.granted_services(), authority::console_write);
        }
    });
}

#[test]
fn scalar_encoding_failure_never_publishes_partial_image_bytes() {
    checked(|| {
        let valid = edition("", "PUSH_CONST 1\nPUSH_CONST 2\nADD\nRET");
        let invalid = edition("", "PUSH_CONST_STR S\nPUSH_CONST 1\nADD\nRET");
        let mut workspace = validation_workspace::new();
        let mut small = [0xa5; 32];
        assert_eq!(
            scalar::encode(valid.as_bytes(), &mut small, &mut workspace),
            Err(error::output)
        );
        assert_eq!(small, [0xa5; 32]);
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            scalar::encode(invalid.as_bytes(), &mut output, &mut workspace),
            Err(error::syntax)
        );
        assert!(output.iter().all(|&byte| byte == 0xa5));
    });
}

#[test]
fn workspace_reuse_cannot_leak_profile_authority_or_parser_policy() {
    checked(|| {
        let scalar_source = edition(
            "",
            "PUSH_CONST 1\nSTORE_LOCAL $ny.temp.idx.0\nLOAD_LOCAL $ny.temp.idx.0\nCONSOLE_WRITE\nPUSH_CONST 0\nRET",
        );
        let flow_source = edition("", "PUSH_CONST 9\nRET");
        let scalar_image = package(&scalar_source);
        let mut flow_image = vec![0; maximum_image_bytes];
        let mut output = vec![0; maximum_image_bytes];
        let mut workspace = validation_workspace::new();
        let length = encode(flow_source.as_bytes(), &mut flow_image, &mut workspace).unwrap();
        flow_image.truncate(length);
        for _ in 0..3 {
            assert_eq!(
                scalar::admit(&scalar_image, authority::console_write, &mut workspace)
                    .unwrap()
                    .profile(),
                3
            );
            assert_eq!(
                scalar::admit(&scalar_image, 0, &mut workspace).err(),
                Some(error::ceiling)
            );
            assert_eq!(
                encode(scalar_source.as_bytes(), &mut output, &mut workspace),
                Err(error::syntax)
            );
            {
                let witness = admit(&flow_image, 0, &mut workspace).unwrap();
                assert_eq!(witness.profile(), 2);
                assert_eq!(witness.required_services(), 0);
                assert_eq!(witness.program().functions()[0].local_count, 0);
            }
            assert_eq!(
                scalar::admit(&flow_image, 0, &mut workspace).err(),
                Some(error::profile)
            );
            assert_eq!(
                admit(&scalar_image, 0, &mut workspace).err(),
                Some(error::profile)
            );
        }
    });
}
