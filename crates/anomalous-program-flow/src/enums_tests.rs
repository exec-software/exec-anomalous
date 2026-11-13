extern crate std;

use super::*;
use std::{format, string::String, vec, vec::Vec};

const layouts: &str = "# Enum definitions\nenum app::state\nIdle\nBusy: i64\nOther: i64\nend enum\nenum app::other\nIdle\nBusy: i64\nend enum\n";
const helpers: &str = "# declaration_line 1\nfunction identity(value: app::state) -> app::state:\nSTORE_LOCAL value\nLOAD_LOCAL value\nRET\nend function\n";

fn checked(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}
fn edition(declarations: &str, functions: &str, annotation: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n# String constants\nconst A = \"text\"\n{declarations}{functions}{annotation}# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}
fn authored(declarations: &str, functions: &str, body: &str) -> String {
    edition(
        declarations,
        functions,
        "@ai_context(\"Exercise nominal enums\")\n",
        body,
    )
}
fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let length = enums::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(length);
    output
}
fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    enums::encode(source.as_bytes(), &mut output, &mut workspace).map(|_| ())
}

#[test]
fn enum_profile_changes_only_both_tags_for_previous_syntax() {
    checked(|| {
        let source = authored(
            "",
            "",
            "PUSH_CONST 41\nSTORE_GLOBAL app::n i64\nLOAD_GLOBAL app::n i64\nCAST text\nPOP\nPUSH_CONST 0\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let mut prior = vec![0; maximum_image_bytes];
        let length = globals::encode(source.as_bytes(), &mut prior, &mut workspace).unwrap();
        prior.truncate(length);
        assert_eq!(enums::derivation_profile, 9);
        assert_eq!(get32(&image, 12), 9);
        assert_eq!(get32(&image, image_header_bytes + 12), 9);
        put32(&mut prior, 12, 9);
        put32(&mut prior, image_header_bytes + 12, 9);
        assert_eq!(image, prior);
        let witness = enums::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 9);
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.required_services(), 0);
        assert_eq!(core::mem::size_of::<kind>(), 2);
    });
}

#[test]
fn enum_headers_and_all_earlier_grammars_remain_isolated() {
    checked(|| {
        let source = authored(
            layouts,
            "",
            "NEW_VARIANT app::state::Idle 0 app::state void\nIS_VARIANT app::state::Busy app::state i64\nRET",
        );
        let image = package(&source);
        for outer in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, u32::MAX] {
            for inner in [0, 1, 8, 9, 10, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                assert_eq!(
                    enums::admit(&changed, 0, &mut workspace).err(),
                    if outer == 9 && inner == 9 {
                        None
                    } else {
                        Some(error::profile)
                    }
                );
            }
        }
        for source in [
            source,
            authored(
                "",
                "",
                "PUSH_CONST 0\nRET\nNEW_VARIANT state::Idle 0 state void",
            ),
            authored(
                "",
                "",
                "PUSH_CONST 0\nRET\nIS_VARIANT state::Idle state void",
            ),
            authored("", "", "PUSH_CONST 0\nRET\nUNWRAP_VARIANT 0 state void"),
        ] {
            let mut output = vec![0xa5; maximum_image_bytes];
            assert_eq!(
                anomalous_program_manifest::encode(source.as_bytes(), &mut output),
                // Profile 1 rejects every instruction following RET before
                // examining its opcode; declarations fail earlier as unsupported.
                Err(if source.contains("# Enum definitions") {
                    error::unsupported
                } else {
                    error::syntax
                })
            );
            for profile in [2, 3, 4, 5, 6, 7, 8] {
                let mut workspace = validation_workspace::new();
                let result = match profile {
                    2 => encode(source.as_bytes(), &mut output, &mut workspace),
                    3 => scalar::encode(source.as_bytes(), &mut output, &mut workspace),
                    4 => text::encode(source.as_bytes(), &mut output, &mut workspace),
                    5 => array::encode(source.as_bytes(), &mut output, &mut workspace),
                    6 => callable::encode(source.as_bytes(), &mut output, &mut workspace),
                    7 => record::encode(source.as_bytes(), &mut output, &mut workspace),
                    _ => globals::encode(source.as_bytes(), &mut output, &mut workspace),
                };
                assert_eq!(result, Err(error::unsupported));
                assert!(output.iter().all(|byte| *byte == 0xa5));
            }
        }
        for profile in [2, 3, 4, 5, 6, 7, 8] {
            let mut changed = image.clone();
            put32(&mut changed, 12, profile);
            put32(&mut changed, image_header_bytes + 12, profile);
            let mut workspace = validation_workspace::new();
            let result = match profile {
                2 => admit(&changed, 0, &mut workspace),
                3 => scalar::admit(&changed, 0, &mut workspace),
                4 => text::admit(&changed, 0, &mut workspace),
                5 => array::admit(&changed, 0, &mut workspace),
                6 => callable::admit(&changed, 0, &mut workspace),
                7 => record::admit(&changed, 0, &mut workspace),
                _ => globals::admit(&changed, 0, &mut workspace),
            };
            assert_eq!(result.err(), Some(error::unsupported));
        }
    });
}

#[test]
fn layouts_and_resolved_operations_preserve_nominal_and_variant_identity() {
    checked(|| {
        let source = authored(
            layouts,
            "",
            "PUSH_CONST 9223372036854775807\nNEW_VARIANT app::state::Other 1 app::state i64\nUNWRAP_VARIANT 1 app::state i64\nPOP\nNEW_VARIANT app::other::Idle 0 app::other void\nIS_VARIANT app::other::Busy app::other i64\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = enums::admit(&image, 0, &mut workspace).unwrap();
        let plan = witness.program();
        assert_eq!(plan.enum_layouts().len(), 2);
        assert_eq!(plan.enum_layouts()[0].name, "app::state");
        assert_eq!(plan.enum_layouts()[1].name, "app::other");
        assert_eq!(plan.enum_layouts()[0].context(), None);
        assert_eq!(plan.enum_layouts()[0].variants().len(), 3);
        assert_eq!(plan.enum_layouts()[0].variants()[0].name, "Idle");
        assert!(!plan.enum_layouts()[0].variants()[0].has_payload);
        assert!(plan.enum_layouts()[0].variants()[2].has_payload);
        assert_eq!(
            plan.instructions()[1].operation,
            operation::enum_new {
                layout: 0,
                variant: 2
            }
        );
        assert_eq!(
            plan.instructions()[2].operation,
            operation::enum_unwrap {
                layout: 0,
                payload: true
            }
        );
        assert_eq!(
            plan.instructions()[4].operation,
            operation::enum_new {
                layout: 1,
                variant: 0
            }
        );
        assert_eq!(
            plan.instructions()[5].operation,
            operation::enum_is {
                layout: 1,
                variant: 1
            }
        );
    });
}

#[test]
fn enum_context_is_scoped_and_empty_declarations_are_valid_metadata() {
    checked(|| {
        let declarations = "# Enum definitions\n@ai_context(\"State declaration\")\nenum state\nIdle\nend enum\n@ai_context(\"An empty enum\")\nenum empty\nend enum\n";
        let source = authored(declarations, "", "PUSH_CONST 0\nRET");
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = enums::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(witness.intent(), "Exercise nominal enums");
        assert_eq!(
            witness.program().enum_layouts()[0].context(),
            Some("State declaration")
        );
        assert_eq!(
            witness.program().enum_layouts()[1].context(),
            Some("An empty enum")
        );
        assert!(witness.program().enum_layouts()[1].variants().is_empty());
        assert_eq!(
            validate(&edition(declarations, "", "", "PUSH_CONST 0\nRET")),
            Err(error::intent)
        );
        for body in [
            "PUSH_CONST 0\nRET\nNEW_VARIANT empty::Unit 0 empty void",
            "PUSH_CONST 0\nRET\nUNWRAP_VARIANT 0 empty void",
        ] {
            assert_eq!(
                validate(&authored(declarations, "", body)),
                Err(error::unsupported)
            );
        }
        for (context, expected) in [
            (String::new(), Err(error::intent)),
            ("x".repeat(512), Ok(())),
            ("x".repeat(513), Err(error::limit)),
        ] {
            let declarations =
                format!("# Enum definitions\n@ai_context(\"{context}\")\nenum empty\nend enum\n");
            assert_eq!(
                validate(&authored(&declarations, "", "PUSH_CONST 0\nRET")),
                expected
            );
        }
        let duplicate = "# Enum definitions\n@ai_context(\"first\")\n@ai_context(\"second\")\nenum empty\nend enum\n";
        assert_eq!(
            validate(&authored(duplicate, "", "PUSH_CONST 0\nRET")),
            Err(error::intent)
        );
    });
}

#[test]
fn enum_declaration_grammar_names_and_alias_families_are_closed() {
    checked(|| {
        for (declarations, expected) in [
            ("enum state\nIdle\nend enum\n", error::syntax),
            ("# Enum definitions\n# Enum definitions\n", error::syntax),
            (
                "# Enum definitions\n# declaration_line 1\nenum state\nend enum\n",
                error::syntax,
            ),
            (
                "# Enum definitions\nenum state\nIdle: void\nend enum\n",
                error::unsupported,
            ),
            (
                "# Enum definitions\nenum state\nBusy: text\nend enum\n",
                error::unsupported,
            ),
            (
                "# Enum definitions\nenum state\nBusy: i32\nend enum\n",
                error::unsupported,
            ),
            (
                "# Enum definitions\nenum state\nBusy: i64, i64\nend enum\n",
                error::unsupported,
            ),
            (
                "# Enum definitions\nenum state\nIdle\nIdle\nend enum\n",
                error::syntax,
            ),
            (
                "# Enum definitions\nenum state\nend enum\nenum state\nend enum\n",
                error::syntax,
            ),
            (
                "# Enum definitions\nenum app.state\nend enum\n",
                error::syntax,
            ),
            (
                "# Enum definitions\nenum ::state\nend enum\n",
                error::syntax,
            ),
            (
                "# Enum definitions\nenum state::\nend enum\n",
                error::syntax,
            ),
            (
                "# Enum definitions\nenum state<i64>\nend enum\n",
                error::syntax,
            ),
            (
                "# Enum definitions\nenum state\nBusy.Value\nend enum\n",
                error::syntax,
            ),
            ("# Enum definitions\nenum state\nBusy: i64\n", error::syntax),
            (
                "# Enum definitions\nenum state\nend struct\n",
                error::syntax,
            ),
            (
                "# Struct definitions\nstruct state\nend struct\n# Enum definitions\nenum state\nend enum\n",
                error::syntax,
            ),
            (
                "# Enum definitions\nenum state\nend enum\n# Struct definitions\nstruct state\nend struct\n",
                error::syntax,
            ),
        ] {
            assert_eq!(
                validate(&authored(declarations, "", "PUSH_CONST 0\nRET")),
                Err(expected),
                "{declarations}"
            );
        }
        for name in [
            "void",
            "I64",
            "bOoL",
            "array",
            "Option",
            "option",
            "RESULT",
            "app::Option",
            "app::deep::rEsUlT",
        ] {
            let declarations = format!("# Enum definitions\nenum {name}\nend enum\n");
            assert_eq!(
                validate(&authored(&declarations, "", "PUSH_CONST 0\nRET")),
                Err(error::unsupported),
                "{name}"
            );
        }
        let declarations = "# Enum definitions\nenum optionish\nSome\nNone\nend enum\nenum State\nIdle\nidle\nend enum\nenum state\nend enum\n";
        assert_eq!(
            validate(&authored(declarations, "", "PUSH_CONST 0\nRET")),
            Ok(())
        );
        let functions = "# declaration_line 1\nfunction helper -> void:\nRET\nend function\n# Enum definitions\nenum late\nend enum\n";
        assert_eq!(
            validate(&authored("", functions, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
    });
}

#[test]
fn enum_layout_variant_aggregate_and_combined_name_limits_are_exact() {
    checked(|| {
        for (count, expected) in [(64, Ok(())), (65, Err(error::limit))] {
            let mut declarations = String::from("# Enum definitions\n");
            for index in 0..count {
                declarations.push_str(&format!("enum e{index}\nend enum\n"));
            }
            assert_eq!(
                validate(&authored(&declarations, "", "PUSH_CONST 0\nRET")),
                expected
            );
        }
        for (count, expected) in [(32, Ok(())), (33, Err(error::limit))] {
            let mut declarations = String::from("# Enum definitions\nenum e\n");
            for index in 0..count {
                declarations.push_str(&format!("v{index}: i64\n"));
            }
            declarations.push_str("end enum\n");
            assert_eq!(
                validate(&authored(&declarations, "", "PUSH_CONST 0\nRET")),
                expected
            );
        }
        for (extra, expected) in [(false, Ok(())), (true, Err(error::limit))] {
            let mut declarations = String::from("# Enum definitions\n");
            for layout in 0..8 {
                declarations.push_str(&format!("enum e{layout}\n"));
                for index in 0..32 {
                    declarations.push_str(&format!("v{index}\n"));
                }
                declarations.push_str("end enum\n");
            }
            if extra {
                declarations.push_str("enum extra\nv\nend enum\n");
            }
            assert_eq!(
                validate(&authored(
                    &declarations,
                    "",
                    "NEW_VARIANT e7::v31 0 e7 void\nUNWRAP_VARIANT 0 e7 void\nPUSH_CONST 0\nRET"
                )),
                expected
            );
        }
        for (length, expected) in [(128, Ok(())), (129, Err(error::limit))] {
            let name = "x".repeat(length);
            assert_eq!(
                validate(&authored(
                    &format!("# Enum definitions\nenum {name}\nend enum\n"),
                    "",
                    "PUSH_CONST 0\nRET"
                )),
                expected
            );
        }
        for (length, expected) in [(125, Ok(())), (126, Err(error::limit))] {
            let variant = "v".repeat(length);
            let declarations = format!("# Enum definitions\nenum e\n{variant}\nend enum\n");
            let body =
                format!("NEW_VARIANT e::{variant} 0 e void\nIS_VARIANT e::{variant} e void\nRET");
            assert_eq!(validate(&authored(&declarations, "", &body)), expected);
        }
    });
}

#[test]
fn enum_instruction_metadata_and_references_are_checked_even_when_dead() {
    checked(|| {
        for (instruction, expected) in [
            ("NEW_VARIANT", error::unsupported),
            ("NEW_VARIANT app::state::Idle", error::unsupported),
            (
                "NEW_VARIANT app::state::Idle 0 app::state void extra",
                error::unsupported,
            ),
            (
                "NEW_VARIANT app::state::Idle 0 missing void",
                error::unsupported,
            ),
            (
                "NEW_VARIANT app::state::Missing 0 app::state void",
                error::unsupported,
            ),
            (
                "NEW_VARIANT missing::Idle 0 app::state void",
                error::unsupported,
            ),
            (
                "NEW_VARIANT app::other::Idle 0 app::state void",
                error::syntax,
            ),
            (
                "NEW_VARIANT app::state::Idle 1 app::state i64",
                error::syntax,
            ),
            (
                "NEW_VARIANT app::state::Busy 0 app::state void",
                error::syntax,
            ),
            (
                "NEW_VARIANT app::state::Busy 0 app::state i64",
                error::syntax,
            ),
            (
                "NEW_VARIANT app::state::Busy 1 app::state text",
                error::unsupported,
            ),
            (
                "NEW_VARIANT app::state::Busy 2 app::state i64",
                error::unsupported,
            ),
            (
                "NEW_VARIANT app::state::Busy -1 app::state i64",
                error::syntax,
            ),
            (
                "NEW_VARIANT app::state::Busy +1 app::state i64",
                error::syntax,
            ),
            (
                "NEW_VARIANT app::state::Busy 1x app::state i64",
                error::syntax,
            ),
            ("NEW_VARIANT app.state.Busy 1 app::state i64", error::syntax),
            ("IS_VARIANT app::state::Idle app::state", error::unsupported),
            ("IS_VARIANT app::state::Idle app::state i64", error::syntax),
            ("IS_VARIANT app::other::Busy app::state i64", error::syntax),
            (
                "IS_VARIANT app::state::Missing app::state void",
                error::unsupported,
            ),
            (
                "IS_VARIANT app::state::Busy app::state i32",
                error::unsupported,
            ),
            ("UNWRAP_VARIANT 0 app::state", error::unsupported),
            ("UNWRAP_VARIANT 0 missing void", error::unsupported),
            ("UNWRAP_VARIANT 2 app::state i64", error::unsupported),
            ("UNWRAP_VARIANT one app::state i64", error::syntax),
            ("UNWRAP_VARIANT 1 app::state void", error::syntax),
            ("UNWRAP_VARIANT 0 app::state i64", error::syntax),
            ("UNWRAP_VARIANT 1 app::state array<i64>", error::unsupported),
        ] {
            assert_eq!(
                validate(&authored(
                    layouts,
                    "",
                    &format!("PUSH_CONST 0\nRET\n{instruction}")
                )),
                Err(expected),
                "{instruction}"
            );
        }
        for instruction in [
            "NEW_VARIANT app::state::Busy 1 app::state i64",
            "IS_VARIANT app::state::Busy app::state i64",
            "UNWRAP_VARIANT 1 app::state i64",
        ] {
            assert_eq!(
                validate(&authored(
                    layouts,
                    "",
                    &format!("PUSH_CONST 0\nRET\n{instruction}")
                )),
                Ok(())
            );
        }
        for count in ["0", "000"] {
            assert_eq!(
                validate(&authored(
                    layouts,
                    "",
                    &format!(
                        "NEW_VARIANT app::state::Idle {count} app::state void\nUNWRAP_VARIANT {count} app::state void\nPUSH_CONST 0\nRET"
                    )
                )),
                Ok(())
            );
        }
        for count in ["1", "0001"] {
            assert_eq!(
                validate(&authored(
                    layouts,
                    "",
                    &format!(
                        "PUSH_CONST 7\nNEW_VARIANT app::state::Busy {count} app::state i64\nUNWRAP_VARIANT {count} app::state i64\nRET"
                    )
                )),
                Ok(())
            );
        }
    });
}

#[test]
fn enum_cfg_requires_exact_nominal_receivers_and_integer_payloads() {
    checked(|| {
        for body in [
            "NEW_VARIANT app::state::Busy 1 app::state i64\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST_STR A\nNEW_VARIANT app::state::Busy 1 app::state i64\nPOP\nPUSH_CONST 0\nRET",
            "NEW_VARIANT app::state::Idle 0 app::state void\nNEW_VARIANT app::state::Busy 1 app::state i64\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST 1\nIS_VARIANT app::state::Idle app::state void\nRET",
            "PUSH_CONST_STR A\nUNWRAP_VARIANT 1 app::state i64\nRET",
            "NEW_VARIANT app::other::Idle 0 app::other void\nIS_VARIANT app::state::Idle app::state void\nRET",
            "PUSH_CONST 1\nNEW_VARIANT app::other::Busy 1 app::other i64\nUNWRAP_VARIANT 1 app::state i64\nRET",
            "NEW_VARIANT app::state::Idle 0 app::state void\nUNWRAP_VARIANT 0 app::state void\nRET",
            "PUSH_CONST 1\nNEW_VARIANT app::state::Busy 1 app::state i64\nUNWRAP_VARIANT 1 app::state i64\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(
                validate(&authored(layouts, "", body)),
                Err(error::syntax),
                "{body}"
            );
        }
        for body in [
            "PUSH_CONST 7\nNEW_VARIANT app::state::Idle 0 app::state void\nUNWRAP_VARIANT 0 app::state void\nRET",
            "PUSH_CONST -9223372036854775808\nNEW_VARIANT app::state::Busy 1 app::state i64\nUNWRAP_VARIANT 1 app::state i64\nRET",
            "PUSH_CONST 7\nNEW_VARIANT app::state::Busy 1 app::state i64\nIS_VARIANT app::state::Other app::state i64\nRET",
        ] {
            assert_eq!(validate(&authored(layouts, "", body)), Ok(()));
        }
    });
}

#[test]
fn unwrap_admits_nominal_shape_union_and_leaves_selected_shape_to_runtime() {
    checked(|| {
        for body in [
            "NEW_VARIANT app::state::Idle 0 app::state void\nUNWRAP_VARIANT 1 app::state i64\nRET",
            "PUSH_CONST 7\nNEW_VARIANT app::state::Busy 1 app::state i64\nUNWRAP_VARIANT 0 app::state void\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(validate(&authored(layouts, "", body)), Ok(()));
        }
        for (declarations, instruction) in [
            (
                "# Enum definitions\nenum e\nUnit\nend enum\n",
                "UNWRAP_VARIANT 1 e i64",
            ),
            (
                "# Enum definitions\nenum e\nPayload: i64\nend enum\n",
                "UNWRAP_VARIANT 0 e void",
            ),
        ] {
            assert_eq!(
                validate(&authored(
                    declarations,
                    "",
                    &format!("PUSH_CONST 0\nRET\n{instruction}")
                )),
                Err(error::unsupported)
            );
        }
    });
}

#[test]
fn enum_cfg_joins_locals_and_definite_initialization_preserve_nominal_identity() {
    checked(|| {
        let body = "PUSH_CONST 1\nJMP_IF_NOT right\nNEW_VARIANT app::state::Idle 0 app::state void\nJMP merged\nLABEL right\nPUSH_CONST 7\nNEW_VARIANT app::state::Busy 1 app::state i64\nLABEL merged\nSTORE_LOCAL value\nLOAD_LOCAL value\nIS_VARIANT app::state::Idle app::state void\nRET";
        assert_eq!(validate(&authored(layouts, "", body)), Ok(()));
        assert_eq!(
            validate(&authored(
                layouts,
                "",
                &body.replace(
                    "app::state::Busy 1 app::state",
                    "app::other::Busy 1 app::other"
                )
            )),
            Err(error::syntax)
        );
        for body in [
            "NEW_VARIANT app::state::Idle 0 app::state void\nSTORE_LOCAL value\nNEW_VARIANT app::other::Idle 0 app::other void\nSTORE_LOCAL value\nPUSH_CONST 0\nRET",
            "PUSH_CONST 0\nJMP_IF_NOT done\nNEW_VARIANT app::state::Idle 0 app::state void\nSTORE_LOCAL value\nLABEL done\nLOAD_LOCAL value\nIS_VARIANT app::state::Idle app::state void\nRET",
        ] {
            assert_eq!(validate(&authored(layouts, "", body)), Err(error::syntax));
        }
    });
}

#[test]
fn enum_helpers_and_callable_signatures_keep_exact_disjoint_nominal_types() {
    checked(|| {
        let functions = format!(
            "{helpers}# declaration_line 1\nfunction same(value: app::state) -> app::state:\nRET\nend function\n# declaration_line 1\nfunction other(value: app::other) -> app::other:\nRET\nend function\n# declaration_line 1\nfunction point(value: point) -> point:\nRET\nend function\n"
        );
        let declarations =
            format!("{layouts}# Struct definitions\nstruct point\nx: i64\nend struct\n");
        let body = "PUSH_CONST 7\nNEW_VARIANT app::state::Busy 1 app::state i64\nCALL identity 1\nSTORE_LOCAL value\nPUSH_CONST 0\nJMP_IF_NOT alternate\nPUSH_FN_PTR identity\nJMP merged\nLABEL alternate\nPUSH_FN_PTR same\nLABEL merged\nSTORE_LOCAL target\nLOAD_LOCAL value\nCALL target 1\nUNWRAP_VARIANT 1 app::state i64\nRET";
        let source = authored(&declarations, &functions, body);
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = enums::admit(&image, 0, &mut workspace).unwrap();
        let functions = witness.program().functions();
        assert_eq!(functions[0].parameter_kinds(), &[kind::enumeration(0)]);
        assert_eq!(functions[0].return_kind, Some(kind::enumeration(0)));
        assert_eq!(functions[0].signature(), functions[1].signature());
        assert_ne!(functions[0].signature(), functions[2].signature());
        assert_ne!(functions[0].signature(), functions[3].signature());
        assert_eq!(
            validate(&source.replace("PUSH_FN_PTR same", "PUSH_FN_PTR other")),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&source.replace("PUSH_FN_PTR same", "PUSH_FN_PTR point")),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                layouts,
                helpers,
                "NEW_VARIANT app::other::Idle 0 app::other void\nCALL identity 1\nPOP\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        let bad_return = "# declaration_line 1\nfunction broken -> app::state:\nNEW_VARIANT app::other::Idle 0 app::other void\nRET\nend function\n";
        assert_eq!(
            validate(&authored(layouts, bad_return, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
    });
}

#[test]
fn enum_values_do_not_coerce_into_existing_value_families_or_authority() {
    checked(|| {
        let declarations =
            format!("{layouts}# Struct definitions\nstruct point\nx: i64\nend struct\n");
        for tail in [
            "PUSH_CONST 1\nADD i64",
            "PUSH_CONST 1\nCMP_EQ",
            "CAST i32",
            "CAST text",
            "CALL println 1",
            "STORE_GLOBAL app::g i64",
            "NEW_ARRAY_LITERAL 1 i64",
            "NEW_ARRAY_LITERAL 1 text",
            "LEN",
            "PUSH_CONST 0\nGET_INDEX",
            "GET_FIELD x",
            "PUSH_CONST 1\nSET_FIELD x",
            "STORE_LOCAL target\nCALL target 0",
            "JMP_IF_NOT done\nLABEL done",
        ] {
            let body = format!(
                "NEW_VARIANT app::state::Idle 0 app::state void\n{tail}\nPOP\nPUSH_CONST 0\nRET"
            );
            assert_eq!(
                validate(&authored(&declarations, "", &body)),
                Err(error::syntax),
                "{tail}"
            );
        }
        assert_eq!(
            validate(&authored(
                layouts,
                "",
                "NEW_VARIANT app::state::Idle 0 app::state void\nRET"
            )),
            Err(error::syntax)
        );
        let functions = "# declaration_line 1\nfunction bad(value: array<app::state>) -> void:\nPOP\nRET\nend function\n";
        assert_eq!(
            validate(&authored(layouts, functions, "PUSH_CONST 0\nRET")),
            Err(error::unsupported)
        );
        let functions = "# declaration_line 1\nfunction unused -> void:\nPUSH_CONST_STR A\nCALL println 1\nRET\nend function\n";
        let source = authored(layouts, functions, "PUSH_CONST 0\nRET");
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        assert_eq!(
            enums::admit(&image, 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        assert_eq!(
            enums::admit(&image, authority::console_write, &mut workspace)
                .unwrap()
                .required_services(),
            authority::console_write
        );
    });
}

#[test]
fn enum_encoding_is_atomic_and_workspace_reuse_clears_metadata_and_effects() {
    checked(|| {
        let source = authored(
            layouts,
            "",
            "NEW_VARIANT app::state::Idle 0 app::state void\nIS_VARIANT app::state::Idle app::state void\nCALL println 1\nPUSH_CONST 0\nRET",
        );
        let malformed = authored(
            layouts,
            "",
            "PUSH_CONST 0\nRET\nNEW_VARIANT app::state::Unknown 0 app::state void",
        );
        let empty = authored("", "", "PUSH_CONST 0\nRET");
        let image = package(&source);
        let empty_image = package(&empty);
        let mut workspace = validation_workspace::new();
        assert_eq!(
            enums::admit(&image, 1, &mut workspace)
                .unwrap()
                .program()
                .enum_layouts()
                .len(),
            2
        );
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            enums::encode(malformed.as_bytes(), &mut output, &mut workspace),
            Err(error::unsupported)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        let mut tiny = [0xa5; 1];
        assert_eq!(
            enums::encode(source.as_bytes(), &mut tiny, &mut workspace),
            Err(error::output)
        );
        assert_eq!(tiny, [0xa5]);
        let witness = enums::admit(&empty_image, 0, &mut workspace).unwrap();
        assert!(witness.program().enum_layouts().is_empty());
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.intent(), "Exercise nominal enums");
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_enum_context_remains_distinct_from_production_main_intent() {
    checked(|| {
        let declarations =
            "# Enum definitions\n@ai_context(\"Enum metadata\")\nenum state\nUnit\nend enum\n";
        let source = edition(
            declarations,
            "",
            "",
            "NEW_VARIANT state::Unit 0 state void\nIS_VARIANT state::Unit state void\nRET",
        );
        let mut workspace = validation_workspace::new();
        let raw = conformance::validate_enums(source.as_bytes(), 0, &mut workspace).unwrap();
        assert_eq!(raw.profile(), 9);
        assert_eq!(raw.edition(), source.as_bytes());
        assert_eq!(raw.identity(), sha256(source.as_bytes()));
        assert_eq!(raw.intent(), None);
        assert_eq!(
            raw.program().enum_layouts()[0].context(),
            Some("Enum metadata")
        );
        assert_eq!(validate(&source), Err(error::intent));
        assert_eq!(
            conformance::validate_globals(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::unsupported)
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_enum_validation_retains_metadata_cfg_policy_and_whole_file_ceiling() {
    checked(|| {
        for (body, ceiling, expected) in [
            (
                "PUSH_CONST 0\nRET\nIS_VARIANT app::state::Missing app::state void",
                1,
                Some(error::unsupported),
            ),
            (
                "PUSH_CONST 0\nIS_VARIANT app::state::Idle app::state void\nRET",
                1,
                Some(error::syntax),
            ),
            (
                "PUSH_CONST 0\nRET\nPUSH_CONST_STR A\nCALL println 1",
                0,
                Some(error::ceiling),
            ),
            ("PUSH_CONST 0\nRET", 2, Some(error::ceiling)),
            (
                "NEW_VARIANT app::state::Idle 0 app::state void\nUNWRAP_VARIANT 1 app::state i64\nRET",
                0,
                None,
            ),
        ] {
            let source = edition(layouts, "", "", body);
            let mut workspace = validation_workspace::new();
            assert_eq!(
                conformance::validate_enums(source.as_bytes(), ceiling, &mut workspace).err(),
                expected
            );
        }
        let source = edition(layouts, "", "@ai_context(\"\")\n", "PUSH_CONST 0\nRET");
        let mut workspace = validation_workspace::new();
        assert_eq!(
            conformance::validate_enums(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::intent)
        );
    });
}
