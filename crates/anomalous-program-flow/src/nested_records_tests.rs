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

fn authored(prefix: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n{prefix}@ai_context(\"Exercise bounded nested records\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn identity(name: &str, value_type: &str) -> String {
    format!(
        "# declaration_line 1\nfunction {name}(value: {value_type}) -> {value_type}:\nRET\nend function\n"
    )
}

fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let length = nested_records::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(length);
    output
}

fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0xa5; maximum_image_bytes];
    let result = nested_records::encode(source.as_bytes(), &mut output, &mut workspace);
    if result.is_err() {
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
    result.map(|_| ())
}

const records: &str = "# Struct definitions\nstruct leaf\nx: i64\nend struct\nstruct other\nx: i64\nend struct\nstruct holder\nchild: leaf\ncount: i64\nend struct\n";

#[test]
fn nested_record_profile_changes_only_both_tags_for_inherited_syntax() {
    checked(|| {
        let source = authored(
            "# Struct definitions\nstruct point\nx: i64\nend struct\n",
            "# source_debug 0 \"scalar.ny\" 0\nPUSH_CONST -0.0\nCAST f64\nPOP\nPUSH_CONST 1\nCAST bool\nCAST i64\nPOP\nNEW_STRUCT point\nPUSH_CONST 42\nSET_FIELD x\nGET_FIELD x\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let mut prior = vec![0; maximum_image_bytes];
        let length = typed_scalars::encode(source.as_bytes(), &mut prior, &mut workspace).unwrap();
        prior.truncate(length);
        assert_eq!(nested_records::derivation_profile, 12);
        assert_eq!(get32(&image, 12), 12);
        assert_eq!(get32(&image, image_header_bytes + 12), 12);
        put32(&mut prior, 12, 12);
        put32(&mut prior, image_header_bytes + 12, 12);
        assert_eq!(image, prior);
        let witness = nested_records::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 12);
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.required_services(), 0);
        assert_eq!(
            witness.program().record_layouts()[0].field_kinds(),
            &[kind::integer]
        );
        assert_eq!(
            witness.program().instructions()[0].operation,
            operation::float(1u64 << 63)
        );
        assert_eq!(
            witness
                .program()
                .source_debug(0)
                .unwrap()
                .instruction_index(),
            0
        );
    });
}

#[test]
fn nested_record_headers_and_previous_profiles_remain_isolated() {
    checked(|| {
        let image = package(&authored(records, "PUSH_CONST 0\nRET"));
        for outer in [0, 1, 7, 10, 11, 12, 13, u32::MAX] {
            for inner in [0, 1, 11, 12, 13, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                assert_eq!(
                    nested_records::admit(&changed, 0, &mut workspace).err(),
                    if outer == 12 && inner == 12 {
                        None
                    } else {
                        Some(error::profile)
                    }
                );
            }
        }
        for (prefix, record_error) in [
            (records, error::unsupported),
            (
                "# Struct definitions\nstruct box<i64>\nx: i64\nend struct\n",
                error::syntax,
            ),
            (
                "# Enum definitions\nenum choice<i64>\nUnit\nend enum\n",
                error::syntax,
            ),
        ] {
            let source = authored(prefix, "PUSH_CONST 0\nRET");
            let mut output = vec![0xa5; maximum_image_bytes];
            assert_eq!(
                anomalous_program_manifest::encode(source.as_bytes(), &mut output),
                Err(error::unsupported)
            );
            assert!(output.iter().all(|byte| *byte == 0xa5));
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
            ] {
                let expected =
                    if profile.tag() < 7 || (prefix.contains("enum ") && profile.tag() < 9) {
                        error::unsupported
                    } else {
                        record_error
                    };
                let mut workspace = validation_workspace::new();
                let mut output = vec![0xa5; maximum_image_bytes];
                assert_eq!(
                    encode_profile(source.as_bytes(), &mut output, &mut workspace, profile),
                    Err(expected),
                    "profile {}",
                    profile.tag()
                );
                assert!(output.iter().all(|byte| *byte == 0xa5));
            }
        }
    });
}

#[test]
fn field_views_resolve_forward_types_and_global_ids_in_declaration_order() {
    checked(|| {
        let prefix = "# Struct definitions\nstruct box\nvalue: leaf\ncount: i64\nend struct\nstruct leaf\nvalue: i64\nend struct\nstruct alternate\ncount: leaf\nend struct\n";
        let image = package(&authored(prefix, "PUSH_CONST 0\nRET"));
        let mut workspace = validation_workspace::new();
        let witness = nested_records::admit(&image, 0, &mut workspace).unwrap();
        let layouts = witness.program().record_layouts();
        assert_eq!(layouts[0].field_ids(), &[0, 1]);
        assert_eq!(layouts[0].field_kinds(), &[kind::record(1), kind::integer]);
        assert_eq!(layouts[1].field_ids(), &[0]);
        assert_eq!(layouts[1].field_kind(0), Some(kind::integer));
        assert_eq!(layouts[1].field_kind(1), None);
        assert_eq!(layouts[2].field_ids(), &[1]);
        assert_eq!(layouts[2].field_kind(1), Some(kind::record(1)));
        assert_eq!(layouts[2].field_kind(0), None);
        assert_eq!(layouts[0].field_kind(u8::MAX), None);
    });
}

#[test]
fn self_mutual_and_forward_field_dependencies_need_no_recursive_resolution() {
    checked(|| {
        let prefix = "# Struct definitions\nstruct node\nnext: node\nother: peer\nend struct\nstruct peer\nparent: node\nend struct\n";
        assert_eq!(
            validate(&authored(
                prefix,
                "NEW_STRUCT node\nSTORE_LOCAL node\nNEW_STRUCT peer\nSTORE_LOCAL peer\nLOAD_LOCAL node\nLOAD_LOCAL node\nSET_FIELD next\nLOAD_LOCAL peer\nSET_FIELD other\nPOP\nLOAD_LOCAL peer\nLOAD_LOCAL node\nSET_FIELD parent\nGET_FIELD parent\nGET_FIELD next\nPOP\nPUSH_CONST 0\nRET"
            )),
            Ok(())
        );
        // Initialization is a runtime property, not a forged integer handle or
        // an admission-time constant-folding rule.
        assert_eq!(
            validate(&authored(
                prefix,
                "NEW_STRUCT node\nGET_FIELD next\nPOP\nPUSH_CONST 0\nRET"
            )),
            Ok(())
        );
    });
}

#[test]
fn nested_get_and_set_keep_exact_kinds_and_leave_the_owner_on_write() {
    checked(|| {
        let helpers = identity("copy", "leaf");
        assert_eq!(
            validate(&authored(
                &format!("{records}{helpers}"),
                "NEW_STRUCT holder\nNEW_STRUCT leaf\nPUSH_CONST 42\nSET_FIELD x\nCALL copy 1\nSET_FIELD child\nPUSH_CONST 1\nSET_FIELD count\nGET_FIELD child\nGET_FIELD x\nRET"
            )),
            Ok(())
        );
        for input in [
            "PUSH_CONST 0",
            "PUSH_CONST 1\nCAST bool",
            "PUSH_CONST 1\nCAST u8",
            "PUSH_CONST 0.5",
            "PUSH_CONST_STR S",
            "NEW_STRUCT other",
            "NEW_ARRAY_LITERAL 0 i64",
            "PUSH_FN_PTR copy",
            "NEW_VARIANT state::Unit 0 state void",
        ] {
            let prefix = format!(
                "# String constants\nconst S = \"leaf\"\n{records}# Enum definitions\nenum state\nUnit\nend enum\n{helpers}"
            );
            assert_eq!(
                validate(&authored(
                    &prefix,
                    &format!("NEW_STRUCT holder\n{input}\nSET_FIELD child\nPOP\nPUSH_CONST 0\nRET")
                )),
                Err(error::syntax),
                "{input}"
            );
        }
        assert_eq!(
            validate(&authored(
                records,
                "NEW_STRUCT holder\nNEW_STRUCT leaf\nSET_FIELD count\nPOP\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                records,
                "NEW_STRUCT other\nGET_FIELD child\nPOP\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
    });
}

#[test]
fn nested_field_dependencies_and_unsupported_field_families_are_checked_unused() {
    checked(|| {
        for field_type in [
            "missing", "i32", "bool", "u8", "f64", "text", "state", "box<u8>",
        ] {
            let prefix = format!(
                "# Struct definitions\nstruct box<i64>\nvalue: i64\nend struct\nstruct holder\nchild: {field_type}\nend struct\n# Enum definitions\nenum state\nUnit\nend enum\n"
            );
            assert_eq!(
                validate(&authored(&prefix, "PUSH_CONST 0\nRET")),
                Err(error::unsupported),
                "{field_type}"
            );
        }
        let duplicate = "# Struct definitions\nstruct node\nnext: node\nnext: i64\nend struct\n";
        assert_eq!(
            validate(&authored(duplicate, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(records, "PUSH_CONST 0\nRET\nGET_FIELD absent")),
            Err(error::unsupported)
        );
    });
}

#[test]
fn concrete_identity_accepts_exactly_one_primitive_argument_without_instantiation() {
    checked(|| {
        let mut prefix = String::from("# Struct definitions\n");
        for argument in ["i32", "i64", "bool", "u8", "f64", "text"] {
            prefix.push_str(&format!(
                "struct app::box<{argument}>\nvalue: i64\nend struct\n"
            ));
        }
        prefix.push_str("# Enum definitions\n");
        for argument in ["i32", "i64", "bool", "u8", "f64", "text"] {
            prefix.push_str(&format!(
                "enum app::choice<{argument}>\nValue: i64\nUnit\nend enum\n"
            ));
        }
        let image = package(&authored(
            &prefix,
            "NEW_STRUCT app::box<text>\nPUSH_CONST 42\nSET_FIELD value\nGET_FIELD value\nNEW_VARIANT app::choice::Value 1 app::choice<f64> i64\nUNWRAP_VARIANT 1 app::choice<f64> i64\nRET",
        ));
        let mut workspace = validation_workspace::new();
        let witness = nested_records::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(witness.program().record_layouts().len(), 6);
        assert_eq!(witness.program().enum_layouts().len(), 6);
        for layout in witness.program().record_layouts() {
            assert_eq!(layout.field_kinds(), &[kind::integer]);
        }
    });
}

#[test]
fn generic_spelling_and_reserved_bases_remain_strict_and_bounded() {
    checked(|| {
        for name in [
            "box<>",
            "box<",
            "box<i64",
            "box<i64>x",
            "box<i64>>",
            "box<i64,u8>",
            "box<box<i64>>",
            "box<T>",
            "box<void>",
            "box<byte>",
            "box< i64>",
            "box<i64 >",
            "box<array<i64>>",
            "box<i64>::child",
            "i64<i64>",
            "BoOl<i64>",
        ] {
            assert_eq!(
                validate(&authored(
                    &format!("# Struct definitions\nstruct {name}\nend struct\n"),
                    "PUSH_CONST 0\nRET"
                )),
                Err(error::unsupported),
                "{name}"
            );
        }
        for name in ["bad.name<i64>", "::box<i64>", "box::<i64>", "box>"] {
            assert_eq!(
                validate(&authored(
                    &format!("# Struct definitions\nstruct {name}\nend struct\n"),
                    "PUSH_CONST 0\nRET"
                )),
                Err(error::syntax),
                "{name}"
            );
        }
        for name in ["Option<i64>", "app::RESULT<u8>", "BOOL<i64>"] {
            assert_eq!(
                validate(&authored(
                    &format!("# Enum definitions\nenum {name}\nUnit\nend enum\n"),
                    "PUSH_CONST 0\nRET"
                )),
                Err(error::unsupported)
            );
        }
        let base = "b".repeat(123);
        let exact = format!("# Enum definitions\nenum {base}<i64>\nTag\nend enum\n");
        assert_eq!(
            validate(&authored(
                &exact,
                &format!(
                    "NEW_VARIANT {base}::Tag 0 {base}<i64> void\nIS_VARIANT {base}::Tag {base}<i64> void\nRET"
                )
            )),
            Ok(())
        );
        assert_eq!(
            validate(&authored(
                &exact.replace("Tag", "Tags"),
                "PUSH_CONST 0\nRET"
            )),
            Err(error::limit)
        );
        assert_eq!(
            validate(&authored(
                &format!("# Struct definitions\nstruct {base}b<i64>\nend struct\n"),
                "PUSH_CONST 0\nRET"
            )),
            Err(error::limit)
        );
    });
}

#[test]
fn wrapper_suffixes_do_not_reinterpret_plain_or_qualified_nominal_names() {
    checked(|| {
        for base in ["ptr", "ref", "owned", "borrow", "fn", "tuple"] {
            for spelling in [String::from(base), base.to_ascii_uppercase()] {
                let prefix = format!(
                    "# Struct definitions\nstruct {spelling}<i64>\nvalue: i64\nend struct\n"
                );
                assert_eq!(
                    validate(&authored(&prefix, "PUSH_CONST 0\nRET")),
                    Err(error::unsupported)
                );
                let prefix = format!(
                    "# Struct definitions\nstruct holder\nchild: {spelling}<i64>\nend struct\n"
                );
                assert_eq!(
                    validate(&authored(&prefix, "PUSH_CONST 0\nRET")),
                    Err(error::unsupported)
                );
                let prefix = format!("# Enum definitions\nenum {spelling}<i64>\nUnit\nend enum\n");
                assert_eq!(
                    validate(&authored(&prefix, "PUSH_CONST 0\nRET")),
                    Err(error::unsupported)
                );
                let prefix =
                    format!("# Struct definitions\nstruct {spelling}\nvalue: i64\nend struct\n");
                let plain = authored(
                    &prefix,
                    &format!("NEW_STRUCT {spelling}\nGET_FIELD value\nRET"),
                );
                assert_eq!(validate(&plain), Ok(()));
                let mut workspace = validation_workspace::new();
                let mut output = vec![0; maximum_image_bytes];
                assert!(
                    typed_scalars::encode(plain.as_bytes(), &mut output, &mut workspace).is_ok()
                );
                let prefix = format!(
                    "# Struct definitions\nstruct app::{spelling}<i64>\nvalue: i64\nend struct\n"
                );
                assert_eq!(
                    validate(&authored(
                        &prefix,
                        &format!("NEW_STRUCT app::{spelling}<i64>\nGET_FIELD value\nRET")
                    )),
                    Ok(())
                );
            }
        }
        let prefix = "# Enum definitions\nenum choice<i64>\nUnit\nend enum\n";
        let tag = format!("{}<i64>::Unit", "c".repeat(128));
        assert_eq!(
            validate(&authored(
                prefix,
                &format!("NEW_VARIANT {tag} 0 choice<i64> void\nPOP\nPUSH_CONST 0\nRET")
            )),
            Err(error::limit)
        );
    });
}

#[test]
fn generic_enum_erased_tags_resolve_only_through_exact_concrete_metadata() {
    checked(|| {
        let prefix = "# Enum definitions\nenum choice<i64>\nValue: i64\nUnit\nend enum\nenum choice<u8>\nValue: i64\nUnit\nend enum\n";
        assert_eq!(
            validate(&authored(
                prefix,
                "PUSH_CONST 42\nNEW_VARIANT choice::Value 1 choice<i64> i64\nUNWRAP_VARIANT 1 choice<i64> i64\nRET"
            )),
            Ok(())
        );
        for operation in [
            "IS_VARIANT choice::Value choice<u8> i64",
            "UNWRAP_VARIANT 1 choice<u8> i64",
        ] {
            assert_eq!(
                validate(&authored(
                    prefix,
                    &format!(
                        "PUSH_CONST 42\nNEW_VARIANT choice::Value 1 choice<i64> i64\n{operation}\nRET"
                    )
                )),
                Err(error::syntax)
            );
        }
        for (body, expected) in [
            (
                "NEW_VARIANT choice<i64>::Unit 0 choice<i64> void\nPOP\nPUSH_CONST 0\nRET",
                error::unsupported,
            ),
            (
                "NEW_VARIANT foreign::Unit 0 choice<i64> void\nPOP\nPUSH_CONST 0\nRET",
                error::syntax,
            ),
            (
                "NEW_VARIANT choice::Absent 0 choice<i64> void\nPOP\nPUSH_CONST 0\nRET",
                error::unsupported,
            ),
            (
                "NEW_VARIANT choice::Value 0 choice<i64> void\nPOP\nPUSH_CONST 0\nRET",
                error::syntax,
            ),
        ] {
            assert_eq!(validate(&authored(prefix, body)), Err(expected));
        }
    });
}

#[test]
fn concrete_record_fields_and_helper_signatures_keep_nominal_arguments_distinct() {
    checked(|| {
        let layouts = "# Struct definitions\nstruct box<i64>\nx: i64\nend struct\nstruct box<u8>\nx: i64\nend struct\nstruct owner\nchild: box<i64>\nend struct\n# Enum definitions\nenum choice<i64>\nUnit\nend enum\nenum choice<u8>\nUnit\nend enum\n";
        let mut prefix = String::from(layouts);
        for (name, ty) in [
            ("first", "box<i64>"),
            ("second", "box<u8>"),
            ("enum_first", "choice<i64>"),
            ("enum_second", "choice<u8>"),
            ("same", "box<i64>"),
        ] {
            prefix.push_str(&identity(name, ty));
        }
        let image = package(&authored(
            &prefix,
            "NEW_STRUCT owner\nNEW_STRUCT box<i64>\nCALL first 1\nPUSH_FN_PTR same\nSTORE_LOCAL target\nCALL target 1\nSET_FIELD child\nGET_FIELD child\nGET_FIELD x\nRET",
        ));
        let mut workspace = validation_workspace::new();
        let witness = nested_records::admit(&image, 0, &mut workspace).unwrap();
        for (index, signature) in [0, 1, 2, 3, 0].iter().enumerate() {
            assert_eq!(witness.program().functions()[index].signature(), *signature);
        }
        assert_eq!(
            validate(&authored(
                &prefix,
                "NEW_STRUCT owner\nNEW_STRUCT box<u8>\nSET_FIELD child\nPOP\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                &prefix,
                "NEW_STRUCT box<u8>\nCALL first 1\nPOP\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                &prefix,
                "PUSH_CONST 0\nJMP_IF_NOT other\nPUSH_FN_PTR first\nJMP merged\nLABEL other\nPUSH_FN_PTR second\nLABEL merged\nPOP\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                &format!("{layouts}struct choice<i64>\nend struct\n"),
                "PUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
    });
}

#[test]
fn nested_record_cfg_joins_do_not_merge_structurally_identical_children() {
    checked(|| {
        assert_eq!(
            validate(&authored(
                records,
                "PUSH_CONST 0\nJMP_IF_NOT alternate\nNEW_STRUCT holder\nGET_FIELD child\nJMP merged\nLABEL alternate\nNEW_STRUCT other\nLABEL merged\nPOP\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                records,
                "NEW_STRUCT holder\nGET_FIELD child\nSTORE_LOCAL item\nNEW_STRUCT other\nSTORE_LOCAL item\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                records,
                "PUSH_CONST 0\nJMP_IF_NOT alternate\nNEW_STRUCT holder\nGET_FIELD child\nJMP merged\nLABEL alternate\nNEW_STRUCT leaf\nLABEL merged\nGET_FIELD x\nRET"
            )),
            Ok(())
        );
    });
}

#[test]
fn nested_schemas_keep_layout_field_and_total_field_capacity_limits() {
    checked(|| {
        let mut layouts = String::from("# Struct definitions\n");
        for index in 0..maximum_record_layouts {
            layouts.push_str(&format!("struct r{index}\nnext: r{index}\nend struct\n"));
        }
        assert_eq!(validate(&authored(&layouts, "PUSH_CONST 0\nRET")), Ok(()));
        assert_eq!(
            validate(&authored(
                &(layouts + "struct extra\nend struct\n"),
                "PUSH_CONST 0\nRET"
            )),
            Err(error::limit)
        );
        let mut fields = String::from("# Struct definitions\n");
        for layout in 0..8 {
            fields.push_str(&format!("struct r{layout}\n"));
            for field in 0..maximum_record_fields {
                fields.push_str(&format!("f{layout}_{field}: r{layout}\n"));
            }
            fields.push_str("end struct\n");
        }
        let image = package(&authored(&fields, "PUSH_CONST 0\nRET"));
        let mut workspace = validation_workspace::new();
        let witness = nested_records::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(
            witness.program().record_layouts()[7].field_kind(u8::MAX),
            Some(kind::record(7))
        );
        assert_eq!(
            validate(&authored(
                &(fields + "struct extra\nf: r0\nend struct\n"),
                "PUSH_CONST 0\nRET"
            )),
            Err(error::limit)
        );
        let mut fields = String::from("# Struct definitions\nstruct node\n");
        for field in 0..=maximum_record_fields {
            fields.push_str(&format!("f{field}: node\n"));
        }
        fields.push_str("end struct\n");
        assert_eq!(
            validate(&authored(&fields, "PUSH_CONST 0\nRET")),
            Err(error::limit)
        );
    });
}

#[test]
fn field_kind_workspace_reuse_cannot_leak_nested_types_to_prior_profiles() {
    checked(|| {
        let nested = authored(
            "# Struct definitions\nstruct node\nnext: node\nend struct\n",
            "PUSH_CONST 0\nRET",
        );
        let prior = authored(
            "# Struct definitions\nstruct node\nnext: i64\nend struct\n",
            "NEW_STRUCT node\nGET_FIELD next\nRET",
        );
        let mut workspace = validation_workspace::new();
        let mut output = vec![0; maximum_image_bytes];
        let length =
            nested_records::encode(nested.as_bytes(), &mut output, &mut workspace).unwrap();
        let image = output[..length].to_vec();
        assert_eq!(
            nested_records::admit(&image, 0, &mut workspace)
                .unwrap()
                .program()
                .record_layouts()[0]
                .field_kinds(),
            &[kind::record(0)]
        );
        let length = typed_scalars::encode(prior.as_bytes(), &mut output, &mut workspace).unwrap();
        let image = output[..length].to_vec();
        assert_eq!(
            typed_scalars::admit(&image, 0, &mut workspace)
                .unwrap()
                .program()
                .record_layouts()[0]
                .field_kinds(),
            &[kind::integer]
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn nested_record_raw_validation_preserves_intent_authority_and_metadata_boundaries() {
    checked(|| {
        let source = authored(records, "# source_debug 0 \"nested.ny\" 1 \"child\" \"missing\" \"i64\" 0\nNEW_STRUCT holder\nGET_FIELD child\nPOP\nPUSH_CONST 0\nRET\nPUSH_CONST 7\nCONSOLE_WRITELN").replace("@ai_context(\"Exercise bounded nested records\")\n", "");
        assert_eq!(validate(&source), Err(error::intent));
        let mut workspace = validation_workspace::new();
        assert_eq!(
            conformance::validate_nested_records(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        let raw =
            conformance::validate_nested_records(source.as_bytes(), 1, &mut workspace).unwrap();
        assert_eq!(raw.profile(), 12);
        assert_eq!(raw.intent(), None);
        assert_eq!(raw.required_services(), 1);
        assert_eq!(raw.identity(), sha256(source.as_bytes()));
        assert_eq!(
            raw.program().record_layouts()[2].field_kinds(),
            &[kind::record(0), kind::integer]
        );
        assert_eq!(raw.program().source_debug(0).unwrap().binding_count(), 1);
    });
}
