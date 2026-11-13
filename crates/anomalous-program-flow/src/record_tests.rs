extern crate std;

use super::*;
use std::{format, string::String, vec, vec::Vec};

const layouts: &str = "# Struct definitions\nstruct geo::point\nx: i64\ny: i64\nend struct\nstruct geo::other\nx: i64\nz: i64\nend struct\n";

fn checked(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}
fn edition(records: &str, helpers: &str, annotation: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n# String constants\nconst A = \"text\"\n{records}{helpers}{annotation}# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}
fn authored(records: &str, helpers: &str, body: &str) -> String {
    edition(
        records,
        helpers,
        "@ai_context(\"Exercise nominal mutable records\")\n",
        body,
    )
}
fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let length = record::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(length);
    output
}
fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    record::encode(source.as_bytes(), &mut output, &mut workspace).map(|_| ())
}

#[test]
fn record_profile_changes_only_both_explicit_tags_for_older_syntax() {
    checked(|| {
        let source = authored("", "", "PUSH_CONST 0\nRET");
        let image = package(&source);
        let mut output = vec![0; maximum_image_bytes];
        let mut workspace = validation_workspace::new();
        let length = callable::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
        output.truncate(length);
        assert_eq!(record::derivation_profile, 7);
        assert_eq!(get32(&image, 12), 7);
        assert_eq!(get32(&image, image_header_bytes + 12), 7);
        put32(&mut output, 12, 7);
        put32(&mut output, image_header_bytes + 12, 7);
        assert_eq!(image, output);
        let witness = record::admit(&image, 1, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 7);
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.intent(), "Exercise nominal mutable records");
        assert_eq!(witness.required_services(), 0);
    });
}

#[test]
fn record_headers_and_prior_profile_grammar_remain_isolated() {
    checked(|| {
        let source = authored(layouts, "", "NEW_STRUCT geo::point\nGET_FIELD x\nRET");
        let image = package(&source);
        for outer in [0, 1, 2, 3, 4, 5, 6, 7, 8, u32::MAX] {
            for inner in [0, 1, 2, 3, 4, 5, 6, 7, 8, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                if outer == 7 && inner == 7 {
                    assert_eq!(
                        record::admit(&changed, 0, &mut workspace)
                            .unwrap()
                            .profile(),
                        7
                    );
                } else {
                    assert_eq!(
                        record::admit(&changed, 0, &mut workspace).err(),
                        Some(error::profile)
                    );
                }
            }
        }
        let mut workspace = validation_workspace::new();
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            anomalous_program_manifest::encode(source.as_bytes(), &mut output),
            Err(error::unsupported)
        );
        assert_eq!(
            anomalous_program_manifest::admit(&image, 0).err(),
            Some(error::profile)
        );
        for profile in [2, 3, 4, 5, 6] {
            let encode_result = match profile {
                2 => encode(source.as_bytes(), &mut output, &mut workspace),
                3 => scalar::encode(source.as_bytes(), &mut output, &mut workspace),
                4 => text::encode(source.as_bytes(), &mut output, &mut workspace),
                5 => array::encode(source.as_bytes(), &mut output, &mut workspace),
                _ => callable::encode(source.as_bytes(), &mut output, &mut workspace),
            };
            assert_eq!(encode_result, Err(error::unsupported));
            assert!(output.iter().all(|byte| *byte == 0xa5));
            let mut changed = image.clone();
            put32(&mut changed, 12, profile);
            put32(&mut changed, image_header_bytes + 12, profile);
            let mut retagged = validation_workspace::new();
            let result = match profile {
                2 => admit(&changed, 0, &mut retagged),
                3 => scalar::admit(&changed, 0, &mut retagged),
                4 => text::admit(&changed, 0, &mut retagged),
                5 => array::admit(&changed, 0, &mut retagged),
                _ => callable::admit(&changed, 0, &mut retagged),
            };
            assert_eq!(result.err(), Some(error::unsupported));
        }
    });
}

#[test]
fn record_layouts_keep_nominal_identity_field_order_and_shared_name_ids() {
    checked(|| {
        let source = authored(
            layouts,
            "",
            "NEW_STRUCT geo::point\nPUSH_CONST 42\nSET_FIELD y\nGET_FIELD x\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = record::admit(&image, 0, &mut workspace).unwrap();
        let plan = witness.program();
        assert_eq!(plan.record_field_names(), &["x", "y", "z"]);
        assert_eq!(plan.record_layouts()[0].name, "geo::point");
        assert_eq!(plan.record_layouts()[0].field_ids(), &[0, 1]);
        assert_eq!(plan.record_layouts()[1].field_ids(), &[0, 2]);
        assert_eq!(plan.record_layouts()[0].context(), None);
        assert_eq!(plan.instructions()[0].operation, operation::record_new(0));
        assert_eq!(plan.instructions()[2].operation, operation::record_set(1));
        assert_eq!(plan.instructions()[3].operation, operation::record_get(0));
        assert_eq!(core::mem::size_of::<kind>(), 2);
    });
}

#[test]
fn record_context_is_scoped_separately_from_authored_main_intent() {
    checked(|| {
        let records = "# Struct definitions\n@ai_context(\"A screen point\")\nstruct point\nx: i64\nend struct\n@ai_context(\"Another record\")\nstruct empty\nend struct\n";
        let source = authored(records, "", "NEW_STRUCT point\nGET_FIELD x\nRET");
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = record::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(witness.intent(), "Exercise nominal mutable records");
        assert_eq!(
            witness.program().record_layouts()[0].context(),
            Some("A screen point")
        );
        assert_eq!(
            witness.program().record_layouts()[1].context(),
            Some("Another record")
        );
        let source = edition(records, "", "", "PUSH_CONST 0\nRET");
        assert_eq!(validate(&source), Err(error::intent));
        for (annotation, expected) in [
            ("@ai_context(\"\")\n", error::intent),
            (
                "@ai_context(\"one\")\n@ai_context(\"two\")\n",
                error::intent,
            ),
            ("@ai_context(\"escaped\\n\")\n", error::intent),
            ("@unknown(\"text\")\n", error::unsupported),
        ] {
            let records =
                format!("# Struct definitions\n{annotation}struct point\nx: i64\nend struct\n");
            assert_eq!(
                validate(&authored(&records, "", "PUSH_CONST 0\nRET")),
                Err(expected)
            );
        }
        for count in [maximum_intent_bytes, maximum_intent_bytes + 1] {
            let records = format!(
                "# Struct definitions\n@ai_context(\"{}\")\nstruct point\nend struct\n",
                "x".repeat(count)
            );
            assert_eq!(
                validate(&authored(&records, "", "PUSH_CONST 0\nRET")),
                if count == maximum_intent_bytes {
                    Ok(())
                } else {
                    Err(error::limit)
                }
            );
        }
    });
}

#[test]
fn record_declarations_have_closed_order_names_and_i64_fields() {
    checked(|| {
        for (records, expected) in [
            ("struct point\nx: i64\nend struct\n", error::syntax),
            (
                "# Struct definitions\n# Struct definitions\n",
                error::syntax,
            ),
            (
                "# Struct definitions\n# declaration_line 1\nstruct point\nend struct\n",
                error::syntax,
            ),
            (
                "# Struct definitions\nstruct point\nx i64\nend struct\n",
                error::syntax,
            ),
            (
                "# Struct definitions\nstruct point\nx: i32\nend struct\n",
                error::unsupported,
            ),
            (
                "# Struct definitions\nstruct point\nx: text\nend struct\n",
                error::unsupported,
            ),
            (
                "# Struct definitions\nstruct point\nx: array<i64>\nend struct\n",
                error::unsupported,
            ),
            (
                "# Struct definitions\nstruct point\nx: point\nend struct\n",
                error::unsupported,
            ),
            (
                "# Struct definitions\nstruct point\nx: i64\nx: i64\nend struct\n",
                error::syntax,
            ),
            (
                "# Struct definitions\nstruct point\nend struct\nstruct point\nend struct\n",
                error::syntax,
            ),
            (
                "# Struct definitions\nstruct point\nx: i64\n",
                error::syntax,
            ),
        ] {
            assert_eq!(
                validate(&authored(records, "", "PUSH_CONST 0\nRET")),
                Err(expected),
                "{records}"
            );
        }
        for name in [
            "::point",
            "geo::",
            "geo::::point",
            "geo.point",
            "0point",
            "geo::0point",
        ] {
            assert_eq!(
                validate(&authored(
                    &format!("# Struct definitions\nstruct {name}\nend struct\n"),
                    "",
                    "PUSH_CONST 0\nRET"
                )),
                Err(error::syntax)
            );
        }
        for name in [
            "i32", "i64", "text", "void", "bool", "BOOL", "Array", "anything", "pointer", "usize",
        ] {
            assert_eq!(
                validate(&authored(
                    &format!("# Struct definitions\nstruct {name}\nend struct\n"),
                    "",
                    "PUSH_CONST 0\nRET"
                )),
                Err(error::unsupported)
            );
        }
        assert_eq!(
            validate(&authored(
                "# Struct definitions\nstruct geo::bool\nend struct\n",
                "",
                "NEW_STRUCT geo::bool\nPOP\nPUSH_CONST 0\nRET"
            )),
            Ok(())
        );
        let after = format!(
            "{}struct late\nend struct\n",
            authored(layouts, "", "PUSH_CONST 0\nRET")
        );
        assert_eq!(validate(&after), Err(error::syntax));
    });
}

#[test]
fn declared_layout_and_field_limits_include_empty_and_repeated_name_layouts() {
    checked(|| {
        for count in [maximum_record_layouts, maximum_record_layouts + 1] {
            let mut records = String::from("# Struct definitions\n");
            for index in 0..count {
                records.push_str(&format!("struct r{index}\nend struct\n"));
            }
            assert_eq!(
                validate(&authored(
                    &records,
                    "",
                    "NEW_STRUCT r63\nPOP\nPUSH_CONST 0\nRET"
                )),
                if count == maximum_record_layouts {
                    Ok(())
                } else {
                    Err(error::limit)
                }
            );
        }
        for count in [maximum_record_fields, maximum_record_fields + 1] {
            let mut records = String::from("# Struct definitions\nstruct r\n");
            for index in 0..count {
                records.push_str(&format!("f{index}: i64\n"));
            }
            records.push_str("end struct\n");
            assert_eq!(
                validate(&authored(&records, "", "NEW_STRUCT r\nGET_FIELD f31\nRET")),
                if count == maximum_record_fields {
                    Ok(())
                } else {
                    Err(error::limit)
                }
            );
        }
        for unique in [false, true] {
            for count in [
                maximum_declared_record_fields,
                maximum_declared_record_fields + 1,
            ] {
                let mut records = String::from("# Struct definitions\n");
                for index in 0..count {
                    if index % maximum_record_fields == 0 {
                        records.push_str(&format!("struct r{}\n", index / maximum_record_fields));
                    }
                    records.push_str(&format!(
                        "f{}: i64\n",
                        if unique {
                            index
                        } else {
                            index % maximum_record_fields
                        }
                    ));
                    if index % maximum_record_fields == maximum_record_fields - 1
                        || index + 1 == count
                    {
                        records.push_str("end struct\n");
                    }
                }
                assert_eq!(
                    validate(&authored(&records, "", "PUSH_CONST 0\nRET")),
                    if count == maximum_declared_record_fields {
                        Ok(())
                    } else {
                        Err(error::limit)
                    }
                );
            }
        }
    });
}

#[test]
fn field_name_ids_can_reach_255_and_name_byte_limits_are_exact() {
    checked(|| {
        let mut records = String::from("# Struct definitions\n");
        for layout in 0..8 {
            records.push_str(&format!("struct r{layout}\n"));
            for field in 0..32 {
                records.push_str(&format!("f{}: i64\n", layout * 32 + field));
            }
            records.push_str("end struct\n");
        }
        let image = package(&authored(
            &records,
            "",
            "NEW_STRUCT r7\nGET_FIELD f255\nRET",
        ));
        let mut workspace = validation_workspace::new();
        let witness = record::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(
            witness.program().instructions()[1].operation,
            operation::record_get(255)
        );
        for length in [maximum_name_bytes, maximum_name_bytes + 1] {
            let name = "x".repeat(length);
            let records = format!("# Struct definitions\nstruct {name}\nfield: i64\nend struct\n");
            assert_eq!(
                validate(&authored(&records, "", "PUSH_CONST 0\nRET")),
                if length == maximum_name_bytes {
                    Ok(())
                } else {
                    Err(error::limit)
                }
            );
            let records = format!("# Struct definitions\nstruct record\n{name}: i64\nend struct\n");
            assert_eq!(
                validate(&authored(&records, "", "PUSH_CONST 0\nRET")),
                if length == maximum_name_bytes {
                    Ok(())
                } else {
                    Err(error::limit)
                }
            );
        }
    });
}

#[test]
fn record_operations_enforce_exact_shapes_and_whole_file_references() {
    checked(|| {
        for operation in [
            "NEW_STRUCT",
            "NEW_STRUCT absent",
            "NEW_STRUCT geo::point extra",
            "GET_FIELD",
            "GET_FIELD absent",
            "GET_FIELD x extra",
            "SET_FIELD",
            "SET_FIELD absent",
            "SET_FIELD x extra",
            "SET_FIELD_PTR x",
            "FREE",
            "NEW_CLASS geo::point",
        ] {
            assert_eq!(
                validate(&authored(
                    layouts,
                    "",
                    &format!("PUSH_CONST 0\nRET\n{operation}")
                )),
                Err(error::unsupported),
                "{operation}"
            );
        }
        assert_eq!(
            validate(&authored(
                layouts,
                "",
                "PUSH_CONST 0\nRET\nNEW_STRUCT geo::point\nGET_FIELD z\nSET_FIELD x"
            )),
            Ok(())
        );
        for operation in [
            "NEW_STRUCT geo:::point",
            "GET_FIELD geo::x",
            "SET_FIELD $ny.temp.x.0",
        ] {
            assert_eq!(
                validate(&authored(
                    layouts,
                    "",
                    &format!("PUSH_CONST 0\nRET\n{operation}")
                )),
                Err(error::syntax)
            );
        }
    });
}

#[test]
fn field_cfg_requires_nominal_receiver_integer_assignment_and_preserves_set_owner() {
    checked(|| {
        assert_eq!(
            validate(&authored(
                layouts,
                "",
                "NEW_STRUCT geo::point\nPUSH_CONST 42\nSET_FIELD x\nPUSH_CONST 7\nSET_FIELD y\nGET_FIELD x\nRET"
            )),
            Ok(())
        );
        for body in [
            "GET_FIELD x\nRET",
            "PUSH_CONST 1\nGET_FIELD x\nRET",
            "NEW_STRUCT geo::point\nGET_FIELD z\nRET",
            "NEW_STRUCT geo::other\nPUSH_CONST 1\nSET_FIELD y\nGET_FIELD x\nRET",
            "NEW_STRUCT geo::point\nSET_FIELD x\nGET_FIELD x\nRET",
            "NEW_STRUCT geo::point\nPUSH_CONST_STR A\nSET_FIELD x\nGET_FIELD x\nRET",
            "NEW_STRUCT geo::point\nNEW_STRUCT geo::point\nSET_FIELD x\nGET_FIELD x\nRET",
            "NEW_STRUCT geo::point\nNEW_ARRAY_LITERAL 0 i64\nSET_FIELD x\nGET_FIELD x\nRET",
            "NEW_STRUCT geo::point\nPUSH_CONST 1\nSET_FIELD x\nRET",
        ] {
            assert_eq!(
                validate(&authored(layouts, "", body)),
                Err(error::syntax),
                "{body}"
            );
        }
    });
}

#[test]
fn nominal_identity_is_preserved_through_locals_joins_and_definite_initialization() {
    checked(|| {
        let matching = "PUSH_CONST 0\nJMP_IF_NOT other\nNEW_STRUCT geo::point\nJMP ready\nLABEL other\nNEW_STRUCT geo::point\nLABEL ready\nSTORE_LOCAL selected\nLOAD_LOCAL selected\nGET_FIELD x\nRET";
        assert_eq!(validate(&authored(layouts, "", matching)), Ok(()));
        for body in [
            matching.replace(
                "LABEL other\nNEW_STRUCT geo::point",
                "LABEL other\nNEW_STRUCT geo::other",
            ),
            String::from(
                "NEW_STRUCT geo::point\nSTORE_LOCAL selected\nNEW_STRUCT geo::other\nSTORE_LOCAL selected\nPUSH_CONST 0\nRET",
            ),
            String::from(
                "PUSH_CONST 0\nJMP_IF_NOT ready\nNEW_STRUCT geo::point\nSTORE_LOCAL selected\nLABEL ready\nLOAD_LOCAL selected\nGET_FIELD x\nRET",
            ),
            String::from(
                "NEW_STRUCT geo::point\nLABEL again\nPOP\nNEW_STRUCT geo::other\nPUSH_CONST 0\nJMP_IF again\nPOP\nPUSH_CONST 0\nRET",
            ),
        ] {
            assert_eq!(validate(&authored(layouts, "", &body)), Err(error::syntax));
        }
    });
}

#[test]
fn direct_and_indirect_signatures_preserve_exact_nominal_types() {
    checked(|| {
        let functions = "# declaration_line 1\nfunction identity(value: geo::point) -> geo::point:\nRET\nend function\n# declaration_line 1\nfunction same(renamed: geo::point) -> geo::point:\nRET\nend function\n# declaration_line 1\nfunction different(value: geo::other) -> geo::other:\nRET\nend function\n";
        let body = "PUSH_FN_PTR identity\nSTORE_LOCAL chosen\nPUSH_FN_PTR same\nSTORE_LOCAL chosen\nNEW_STRUCT geo::point\nCALL chosen 1\nCALL identity 1\nGET_FIELD x\nRET";
        let image = package(&authored(layouts, functions, body));
        let mut workspace = validation_workspace::new();
        let witness = record::admit(&image, 0, &mut workspace).unwrap();
        let functions_checked = witness.program().functions();
        assert_eq!(functions_checked[0].parameter_kinds(), &[kind::record(0)]);
        assert_eq!(functions_checked[0].return_kind, Some(kind::record(0)));
        assert_eq!(
            functions_checked[0].signature(),
            functions_checked[1].signature()
        );
        assert_ne!(
            functions_checked[0].signature(),
            functions_checked[2].signature()
        );
        for body in [
            "NEW_STRUCT geo::other\nCALL identity 1\nGET_FIELD x\nRET",
            "PUSH_FN_PTR identity\nSTORE_LOCAL chosen\nPUSH_FN_PTR different\nSTORE_LOCAL chosen\nPUSH_CONST 0\nRET",
            "PUSH_FN_PTR identity\nSTORE_LOCAL chosen\nNEW_STRUCT geo::other\nCALL chosen 1\nGET_FIELD x\nRET",
        ] {
            assert_eq!(
                validate(&authored(layouts, functions, body)),
                Err(error::syntax)
            );
        }
        let wrong_return = "# declaration_line 1\nfunction wrong -> geo::point:\nNEW_STRUCT geo::other\nRET\nend function\n";
        assert_eq!(
            validate(&authored(layouts, wrong_return, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
        let unknown = "# declaration_line 1\nfunction wrong(value: geo::absent) -> i64:\nPOP\nPUSH_CONST 0\nRET\nend function\n";
        assert_eq!(
            validate(&authored(layouts, unknown, "PUSH_CONST 0\nRET")),
            Err(error::unsupported)
        );
    });
}

#[test]
fn records_cannot_act_as_scalars_arrays_function_values_or_main_results() {
    checked(|| {
        for tail in [
            "CAST i64\nRET",
            "NOT\nRET",
            "CONSOLE_WRITE\nPUSH_CONST 0\nRET",
            "LEN\nRET",
            "NEW_ARRAY_LITERAL 1 i64\nLEN\nRET",
            "PUSH_CONST 0\nGET_INDEX\nRET",
            "PUSH_CONST_STR A\nSTR_CONCAT\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST 0\nADD\nRET",
            "PUSH_CONST 0\nCMP_EQ\nRET",
            "STORE_LOCAL chosen\nCALL chosen 0\nRET",
        ] {
            assert_eq!(
                validate(&authored(
                    layouts,
                    "",
                    &format!("NEW_STRUCT geo::point\n{tail}")
                )),
                Err(error::syntax),
                "{tail}"
            );
        }
        let source = authored(layouts, "", "NEW_STRUCT geo::point\nRET")
            .replace("function main -> i64:", "function main -> geo::point:");
        assert_eq!(validate(&source), Err(error::intent));
        for ty in [
            "array<geo::point>",
            "ptr<geo::point>",
            "ref<geo::point>",
            "fn(geo::point)->i64",
        ] {
            let functions = format!(
                "# declaration_line 1\nfunction consume(value: {ty}) -> void:\nPOP\nRET\nend function\n"
            );
            assert_eq!(
                validate(&authored(layouts, &functions, "PUSH_CONST 0\nRET")),
                Err(error::unsupported)
            );
        }
    });
}

#[test]
fn whole_file_record_bodies_still_contribute_authority_and_validate_independently() {
    checked(|| {
        let image = package(&authored(
            layouts,
            "",
            "PUSH_CONST 0\nRET\nGET_FIELD x\nCONSOLE_WRITE",
        ));
        let mut workspace = validation_workspace::new();
        assert_eq!(
            record::admit(&image, 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        assert_eq!(
            record::admit(&image, 1, &mut workspace)
                .unwrap()
                .required_services(),
            1
        );
        let functions = "# declaration_line 1\nfunction unused(value: geo::point) -> i64:\nGET_FIELD z\nRET\nend function\n";
        assert_eq!(
            validate(&authored(layouts, functions, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
        for tail in ["CALL missing 0", "INTEROP_CALL libc puts 1 i64"] {
            assert_eq!(
                validate(&authored(
                    layouts,
                    "",
                    &format!("PUSH_CONST 0\nRET\n{tail}")
                )),
                Err(error::unsupported)
            );
        }
    });
}

#[test]
fn record_encoding_is_atomic_and_workspace_reuse_clears_layouts_context_and_authority() {
    checked(|| {
        let invalid = authored(layouts, "", "NEW_STRUCT geo::point\nGET_FIELD z\nRET");
        let valid = authored(layouts, "", "NEW_STRUCT geo::point\nGET_FIELD x\nRET");
        let pure = authored("", "", "PUSH_CONST 0\nRET");
        let mut workspace = validation_workspace::new();
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            record::encode(invalid.as_bytes(), &mut output, &mut workspace),
            Err(error::syntax)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        assert_eq!(
            record::encode(valid.as_bytes(), &mut output[..1], &mut workspace),
            Err(error::output)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        let image = package(&valid);
        assert_eq!(
            record::admit(&image, 0, &mut workspace)
                .unwrap()
                .program()
                .record_layouts()
                .len(),
            2
        );
        let length = callable::encode(pure.as_bytes(), &mut output, &mut workspace).unwrap();
        let witness = callable::admit(&output[..length], 0, &mut workspace).unwrap();
        assert!(witness.program().record_layouts().is_empty());
        assert!(witness.program().record_field_names().is_empty());
        assert_eq!(witness.required_services(), 0);
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_record_context_never_becomes_main_intent_or_a_production_witness() {
    checked(|| {
        let records = "# Struct definitions\n@ai_context(\"Geometry metadata\")\nstruct point\nx: i64\nend struct\n";
        let source = edition(records, "", "", "NEW_STRUCT point\nGET_FIELD x\nRET");
        let mut workspace = validation_workspace::new();
        let witness = conformance::validate_record(source.as_bytes(), 0, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 7);
        assert_eq!(witness.intent(), None);
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(
            witness.program().record_layouts()[0].context(),
            Some("Geometry metadata")
        );
        assert_eq!(witness.required_services(), 0);
        assert_eq!(
            conformance::validate_callable(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::unsupported)
        );
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            record::encode(source.as_bytes(), &mut output, &mut workspace),
            Err(error::intent)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_record_validation_retains_metadata_cfg_and_ceiling_checks() {
    checked(|| {
        let source = edition(layouts, "", "", "NEW_STRUCT geo::point\nGET_FIELD z\nRET");
        let mut workspace = validation_workspace::new();
        assert_eq!(
            conformance::validate_record(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::syntax)
        );
        let source = edition(layouts, "", "", "PUSH_CONST 0\nRET\nCONSOLE_WRITE");
        assert_eq!(
            conformance::validate_record(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        assert_eq!(
            conformance::validate_record(source.as_bytes(), 1, &mut workspace)
                .unwrap()
                .required_services(),
            1
        );
        assert_eq!(
            conformance::validate_record(source.as_bytes(), 2, &mut workspace).err(),
            Some(error::ceiling)
        );
        let source = edition(
            "# Struct definitions\n@ai_context(\"\")\nstruct empty\nend struct\n",
            "",
            "",
            "PUSH_CONST 0\nRET",
        );
        assert_eq!(
            conformance::validate_record(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::intent)
        );
    });
}
