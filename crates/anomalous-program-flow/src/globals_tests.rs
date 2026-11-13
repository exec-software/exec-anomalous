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
fn edition(records: &str, helpers: &str, annotation: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n# String constants\nconst A = \"text\"\n{records}{helpers}{annotation}# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}
fn authored(helpers: &str, body: &str) -> String {
    edition(
        "",
        helpers,
        "@ai_context(\"Exercise explicit module globals\")\n",
        body,
    )
}
fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let length = globals::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(length);
    output
}
fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    globals::encode(source.as_bytes(), &mut output, &mut workspace).map(|_| ())
}

#[test]
fn globals_profile_changes_only_both_explicit_tags_for_previous_syntax() {
    checked(|| {
        let source = authored("", "PUSH_CONST 42\nRET");
        let image = package(&source);
        let mut previous = vec![0; maximum_image_bytes];
        let mut workspace = validation_workspace::new();
        let length = record::encode(source.as_bytes(), &mut previous, &mut workspace).unwrap();
        previous.truncate(length);
        assert_eq!(globals::derivation_profile, 8);
        assert_eq!(get32(&image, 12), 8);
        assert_eq!(get32(&image, image_header_bytes + 12), 8);
        put32(&mut previous, 12, 8);
        put32(&mut previous, image_header_bytes + 12, 8);
        assert_eq!(image, previous);
        let witness = globals::admit(&image, 1, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 8);
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.intent(), "Exercise explicit module globals");
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.granted_services(), 0);
        assert!(witness.program().global_names().is_empty());
    });
}

#[test]
fn globals_headers_require_profile_eight_and_older_entry_points_refuse_it() {
    checked(|| {
        let image = package(&authored("", "PUSH_CONST 0\nRET"));
        for outer in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, u32::MAX] {
            for inner in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                if outer == 8 && inner == 8 {
                    assert_eq!(
                        globals::admit(&changed, 0, &mut workspace)
                            .unwrap()
                            .profile(),
                        8
                    );
                } else {
                    assert_eq!(
                        globals::admit(&changed, 0, &mut workspace).err(),
                        Some(error::profile)
                    );
                }
            }
        }
        assert_eq!(
            anomalous_program_manifest::admit(&image, 0).err(),
            Some(error::profile)
        );
        for profile in 2..=7 {
            let mut workspace = validation_workspace::new();
            let result = match profile {
                2 => admit(&image, 0, &mut workspace),
                3 => scalar::admit(&image, 0, &mut workspace),
                4 => text::admit(&image, 0, &mut workspace),
                5 => array::admit(&image, 0, &mut workspace),
                6 => callable::admit(&image, 0, &mut workspace),
                _ => record::admit(&image, 0, &mut workspace),
            };
            assert_eq!(result.err(), Some(error::profile));
        }
    });
}

#[test]
fn older_profiles_keep_live_and_dead_global_and_text_cast_refusals() {
    checked(|| {
        for (body, first_profile_error) in [
            ("LOAD_GLOBAL value i64\nRET", error::unsupported),
            (
                "PUSH_CONST 1\nSTORE_GLOBAL value i64\nPUSH_CONST 0\nRET",
                error::unsupported,
            ),
            (
                "PUSH_CONST 42\nCAST text\nPOP\nPUSH_CONST 0\nRET",
                error::unsupported,
            ),
            ("PUSH_CONST 0\nRET\nLOAD_GLOBAL value i64", error::syntax),
            ("PUSH_CONST 0\nRET\nSTORE_GLOBAL value i64", error::syntax),
            ("PUSH_CONST 0\nRET\nCAST text", error::syntax),
        ] {
            let source = authored("", body);
            let image = package(&source);
            let mut output = vec![0xa5; maximum_image_bytes];
            assert_eq!(
                anomalous_program_manifest::encode(source.as_bytes(), &mut output),
                Err(first_profile_error)
            );
            for profile in 2..=7 {
                let mut workspace = validation_workspace::new();
                let result = match profile {
                    2 => encode(source.as_bytes(), &mut output, &mut workspace),
                    3 => scalar::encode(source.as_bytes(), &mut output, &mut workspace),
                    4 => text::encode(source.as_bytes(), &mut output, &mut workspace),
                    5 => array::encode(source.as_bytes(), &mut output, &mut workspace),
                    6 => callable::encode(source.as_bytes(), &mut output, &mut workspace),
                    _ => record::encode(source.as_bytes(), &mut output, &mut workspace),
                };
                assert_eq!(result, Err(error::unsupported));
                assert!(output.iter().all(|byte| *byte == 0xa5));
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
                    _ => record::admit(&changed, 0, &mut workspace),
                };
                assert_eq!(result.err(), Some(error::unsupported));
            }
        }
    });
}

#[test]
fn global_slots_are_interned_once_in_whole_file_first_access_order() {
    checked(|| {
        let helper = "# declaration_line 1\nfunction unused -> i64:\nLOAD_GLOBAL shared i64\nRET\nSTORE_GLOBAL module::second i64\nend function\n";
        let source = authored(
            helper,
            "PUSH_CONST 42\nSTORE_GLOBAL shared i64\nLOAD_GLOBAL module::second i64\nPOP\nLOAD_GLOBAL Shared i64\nCAST text\nPOP\nLOAD_GLOBAL shared i64\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = globals::admit(&image, 0, &mut workspace).unwrap();
        let plan = witness.program();
        assert_eq!(plan.global_names(), &["shared", "module::second", "Shared"]);
        assert_eq!(plan.instructions()[0].operation, operation::global_load(0));
        assert_eq!(plan.instructions()[2].operation, operation::global_store(1));
        let start = plan.functions()[plan.main()].start;
        assert_eq!(
            plan.instructions()[start + 1].operation,
            operation::global_store(0)
        );
        assert_eq!(
            plan.instructions()[start + 2].operation,
            operation::global_load(1)
        );
        assert_eq!(
            plan.instructions()[start + 4].operation,
            operation::global_load(2)
        );
        assert_eq!(
            plan.instructions()[start + 5].operation,
            operation::integer_text
        );
    });
}

#[test]
fn global_names_preserve_case_qualification_reserved_words_and_exact_byte_limits() {
    checked(|| {
        for name in [
            "plain",
            "_private",
            "module::submodule::value",
            "i64",
            "text",
            "void",
            "bool",
            "println",
            "main",
            "A",
        ] {
            assert_eq!(
                validate(&authored("", &format!("LOAD_GLOBAL {name} i64\nRET"))),
                Ok(()),
                "{name}"
            );
        }
        for name in [
            "::value",
            "module::",
            "module::::value",
            "module:value",
            "module.value",
            "0value",
            "$ny.temp.value.0",
            "module::0value",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST 0\nRET\nLOAD_GLOBAL {name} i64")
                )),
                Err(error::syntax),
                "{name}"
            );
        }
        for length in [maximum_name_bytes, maximum_name_bytes + 1] {
            for name in [
                "g".repeat(length),
                format!("module::{}", "g".repeat(length - 8)),
            ] {
                assert_eq!(
                    validate(&authored("", &format!("LOAD_GLOBAL {name} i64\nRET"))),
                    if length == maximum_name_bytes {
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
fn global_slot_bound_counts_dead_loads_stores_and_unused_helpers_without_duplicates() {
    checked(|| {
        assert_eq!(maximum_global_slots, 128);
        for count in [maximum_global_slots, maximum_global_slots + 1] {
            let mut tail = String::new();
            for index in 0..count {
                tail.push_str(&format!(
                    "LOAD_GLOBAL module::g{index} i64\nSTORE_GLOBAL module::g{index} i64\n"
                ));
            }
            let source = authored("", &format!("PUSH_CONST 0\nRET\n{tail}"));
            assert_eq!(
                validate(&source),
                if count == maximum_global_slots {
                    Ok(())
                } else {
                    Err(error::limit)
                }
            );
            if count == maximum_global_slots {
                let image = package(&source);
                let mut workspace = validation_workspace::new();
                let witness = globals::admit(&image, 0, &mut workspace).unwrap();
                assert_eq!(witness.program().global_names().len(), 128);
                assert_eq!(
                    witness.program().instructions()[256].operation,
                    operation::global_load(127)
                );
            }
            let helper = format!(
                "# declaration_line 1\nfunction unused -> i64:\nPUSH_CONST 0\nRET\n{tail}end function\n"
            );
            assert_eq!(
                validate(&authored(&helper, "PUSH_CONST 0\nRET")),
                if count == maximum_global_slots {
                    Ok(())
                } else {
                    Err(error::limit)
                }
            );
        }
    });
}

#[test]
fn typed_global_forms_and_integer_text_target_are_closed_even_when_dead() {
    checked(|| {
        for instruction in [
            "LOAD_GLOBAL",
            "LOAD_GLOBAL name",
            "LOAD_GLOBAL name i32",
            "LOAD_GLOBAL name text",
            "LOAD_GLOBAL name array<i64>",
            "LOAD_GLOBAL name i64 extra",
            "STORE_GLOBAL",
            "STORE_GLOBAL name",
            "STORE_GLOBAL name i32",
            "STORE_GLOBAL name bool",
            "STORE_GLOBAL name i64 extra",
            "CAST text extra",
            "CAST string",
            "CAST String",
            "CAST",
            "CAST f64",
            "CAST bool",
            "LOAD_GLOBAL name anything",
        ] {
            assert_eq!(
                validate(&authored("", &format!("PUSH_CONST 0\nRET\n{instruction}"))),
                Err(error::unsupported),
                "{instruction}"
            );
        }
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 0\nRET\nLOAD_GLOBAL name i64\nSTORE_GLOBAL name text"
            )),
            Err(error::unsupported)
        );
        let declaration = "# NIL Textual Module\nmodule root\nglobal name: i64\n@ai_context(\"Intent\")\n# declaration_line 1\nfunction main -> i64:\nPUSH_CONST 0\nRET\nend function\n";
        assert_eq!(validate(declaration), Err(error::unsupported));
    });
}

#[test]
fn initialization_is_runtime_state_without_zero_defaults_or_automatic_init_calls() {
    checked(|| {
        let initializer = "# declaration_line 1\nfunction __globals_init__ -> void:\nPUSH_CONST 42\nSTORE_GLOBAL initialized i64\nRET\nend function\n";
        for body in [
            "LOAD_GLOBAL never_stored i64\nRET",
            "LOAD_GLOBAL initialized i64\nRET",
            "CALL __globals_init__ 0\nCALL __globals_init__ 0\nLOAD_GLOBAL initialized i64\nRET",
            "PUSH_CONST 0\nJMP_IF_NOT ready\nPUSH_CONST 42\nSTORE_GLOBAL maybe i64\nLABEL ready\nLOAD_GLOBAL maybe i64\nRET",
        ] {
            assert_eq!(validate(&authored(initializer, body)), Ok(()));
        }
        let image = package(&authored(initializer, "LOAD_GLOBAL initialized i64\nRET"));
        let mut workspace = validation_workspace::new();
        let witness = globals::admit(&image, 0, &mut workspace).unwrap();
        let main = &witness.program().functions()[witness.program().main()];
        assert_eq!(main.end - main.start, 2);
        assert_eq!(
            witness.program().instructions()[main.start].operation,
            operation::global_load(0)
        );
        assert!(
            !witness
                .program()
                .instructions()
                .iter()
                .any(|instruction| matches!(instruction.operation, operation::call(_)))
        );
    });
}

#[test]
fn globals_do_not_change_local_direct_or_builtin_call_lookup() {
    checked(|| {
        let helper =
            "# declaration_line 1\nfunction helper -> i64:\nPUSH_CONST 7\nRET\nend function\n";
        let source = authored(
            helper,
            "PUSH_CONST 1\nSTORE_GLOBAL helper i64\nPUSH_CONST 2\nSTORE_GLOBAL chosen i64\nPUSH_CONST 3\nSTORE_GLOBAL println i64\nPUSH_CONST 4\nSTORE_LOCAL chosen\nLOAD_LOCAL chosen\nPOP\nCALL helper 0\nCALL println 1\nLOAD_GLOBAL chosen i64\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = globals::admit(&image, 1, &mut workspace).unwrap();
        assert_eq!(witness.required_services(), 1);
        assert!(
            witness
                .program()
                .instructions()
                .iter()
                .any(|instruction| instruction.operation == operation::call(0))
        );
        assert!(
            witness
                .program()
                .instructions()
                .iter()
                .any(|instruction| instruction.operation == operation::console { newline: true })
        );
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 1\nSTORE_GLOBAL chosen i64\nLOAD_LOCAL chosen\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 1\nSTORE_GLOBAL chosen i64\nCALL chosen 0\nRET"
            )),
            Err(error::unsupported)
        );
    });
}

#[test]
fn global_store_consumes_one_integer_and_rejects_other_kinds_on_reachable_paths() {
    checked(|| {
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 41\nPUSH_CONST 1\nSTORE_GLOBAL value i64\nLOAD_GLOBAL value i64\nADD\nRET"
            )),
            Ok(())
        );
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 1\nCAST i32\nSTORE_GLOBAL value i64\nLOAD_GLOBAL value i64\nRET"
            )),
            Ok(())
        );
        for body in [
            "STORE_GLOBAL value i64\nPUSH_CONST 0\nRET",
            "PUSH_CONST 42\nSTORE_GLOBAL value i64\nRET",
            "PUSH_CONST_STR A\nSTORE_GLOBAL value i64\nPUSH_CONST 0\nRET",
            "NEW_ARRAY_LITERAL 0 i64\nSTORE_GLOBAL value i64\nPUSH_CONST 0\nRET",
            "PUSH_FN_PTR main\nSTORE_GLOBAL value i64\nPUSH_CONST 0\nRET",
            "PUSH_CONST 42\nCAST text\nSTORE_GLOBAL value i64\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(validate(&authored("", body)), Err(error::syntax), "{body}");
        }
        let source = edition(
            "# Struct definitions\nstruct point\nx: i64\nend struct\n",
            "",
            "@ai_context(\"Intent\")\n",
            "NEW_STRUCT point\nSTORE_GLOBAL value i64\nPUSH_CONST 0\nRET",
        );
        assert_eq!(validate(&source), Err(error::syntax));
    });
}

#[test]
fn integer_text_requires_integer_input_and_preserves_text_calls_arrays_and_joins() {
    checked(|| {
        for body in [
            "CAST text\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST_STR A\nCAST text\nPOP\nPUSH_CONST 0\nRET",
            "NEW_ARRAY_LITERAL 0 i64\nCAST text\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_FN_PTR main\nCAST text\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST 42\nCAST text\nRET",
            "PUSH_CONST 42\nCAST text\nCAST i64\nRET",
            "PUSH_CONST 0\nJMP_IF_NOT other\nPUSH_CONST 42\nCAST text\nJMP ready\nLABEL other\nPUSH_CONST 42\nLABEL ready\nPOP\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(validate(&authored("", body)), Err(error::syntax), "{body}");
        }
        let helper = "# declaration_line 1\nfunction describe(value: i64) -> text:\nCAST text\nRET\nend function\n";
        let body = "PUSH_CONST -9223372036854775808\nCALL describe 1\nPUSH_CONST 0\nCAST text\nSTR_CONCAT\nNEW_ARRAY_LITERAL 1 text\nSTORE_LOCAL text_values\nLOAD_LOCAL text_values\nPUSH_CONST 0\nGET_INDEX\nCONSOLE_WRITE\nPUSH_CONST 42\nRET";
        assert_eq!(validate(&authored(helper, body)), Ok(()));
        let matching = "PUSH_CONST 0\nJMP_IF_NOT other\nPUSH_CONST 42\nCAST text\nJMP ready\nLABEL other\nPUSH_CONST_STR A\nLABEL ready\nPOP\nPUSH_CONST 0\nRET";
        assert_eq!(validate(&authored("", matching)), Ok(()));
        let record = edition(
            "# Struct definitions\nstruct point\nend struct\n",
            "",
            "@ai_context(\"Intent\")\n",
            "NEW_STRUCT point\nCAST text\nPOP\nPUSH_CONST 0\nRET",
        );
        assert_eq!(validate(&record), Err(error::syntax));
    });
}

#[test]
fn whole_file_global_forms_preserve_dead_effects_references_and_independent_helper_cfg() {
    checked(|| {
        let image = package(&authored(
            "",
            "PUSH_CONST 0\nRET\nSTORE_GLOBAL value i64\nCAST text\nCONSOLE_WRITE",
        ));
        let mut workspace = validation_workspace::new();
        assert_eq!(
            globals::admit(&image, 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        assert_eq!(
            globals::admit(&image, 1, &mut workspace)
                .unwrap()
                .required_services(),
            1
        );
        let helper = "# declaration_line 1\nfunction unused -> i64:\nPUSH_CONST_STR A\nSTORE_GLOBAL value i64\nPUSH_CONST 0\nRET\nend function\n";
        assert_eq!(
            validate(&authored(helper, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
        for (tail, expected) in [
            ("CALL absent 0", error::unsupported),
            ("JMP absent", error::syntax),
            ("INTEROP_CALL libc puts 1 i64", error::unsupported),
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST 0\nRET\nSTORE_GLOBAL value i64\n{tail}")
                )),
                Err(expected)
            );
        }
    });
}

#[test]
fn globals_inherit_nominal_records_context_and_exact_callable_signatures() {
    checked(|| {
        let records = "# Struct definitions\n@ai_context(\"Record metadata\")\nstruct point\nx: i64\nend struct\n";
        let helper = "# declaration_line 1\nfunction update(value: point) -> point:\nLOAD_GLOBAL shared i64\nSET_FIELD x\nRET\nend function\n";
        let source = edition(
            records,
            helper,
            "@ai_context(\"Main intent\")\n",
            "PUSH_CONST 0x2A\nSTORE_GLOBAL shared i64\nPUSH_FN_PTR update\nSTORE_LOCAL $ny.temp.selected.0\nNEW_STRUCT point\nCALL $ny.temp.selected.0 1\nGET_FIELD x\nCAST text\nCONSOLE_WRITE\nLOAD_GLOBAL shared i64\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = globals::admit(&image, 1, &mut workspace).unwrap();
        assert_eq!(witness.intent(), "Main intent");
        assert_eq!(
            witness.program().record_layouts()[0].context(),
            Some("Record metadata")
        );
        assert_eq!(
            witness.program().functions()[0].parameter_kinds(),
            &[kind::record(0)]
        );
        assert_eq!(
            witness.program().functions()[0].return_kind,
            Some(kind::record(0))
        );
        assert_eq!(witness.program().global_names(), &["shared"]);
    });
}

#[test]
fn encoding_is_atomic_and_workspace_reuse_clears_global_names_and_effects() {
    checked(|| {
        let invalid = authored(
            "",
            "PUSH_CONST_STR A\nSTORE_GLOBAL value i64\nPUSH_CONST 0\nRET",
        );
        let valid = authored(
            "",
            "LOAD_GLOBAL value i64\nCAST text\nCONSOLE_WRITE\nPUSH_CONST 0\nRET",
        );
        let pure = authored("", "PUSH_CONST 0\nRET");
        let mut workspace = validation_workspace::new();
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            globals::encode(invalid.as_bytes(), &mut output, &mut workspace),
            Err(error::syntax)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        assert_eq!(
            globals::encode(valid.as_bytes(), &mut output[..1], &mut workspace),
            Err(error::output)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        let image = package(&valid);
        assert_eq!(
            globals::admit(&image, 1, &mut workspace)
                .unwrap()
                .program()
                .global_names(),
            &["value"]
        );
        let length = record::encode(pure.as_bytes(), &mut output, &mut workspace).unwrap();
        let witness = record::admit(&output[..length], 0, &mut workspace).unwrap();
        assert!(witness.program().global_names().is_empty());
        assert_eq!(witness.required_services(), 0);
        assert_eq!(
            globals::admit(&image, 0, &mut workspace).err(),
            Some(error::ceiling)
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_globals_keep_selected_profile_and_production_main_intent_requirement() {
    checked(|| {
        let source = edition(
            "",
            "",
            "",
            "LOAD_GLOBAL value i64\nCAST text\nPOP\nPUSH_CONST 0\nRET",
        );
        let mut workspace = validation_workspace::new();
        let witness = conformance::validate_globals(source.as_bytes(), 0, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 8);
        assert_eq!(witness.intent(), None);
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.program().global_names(), &["value"]);
        assert_eq!(witness.required_services(), 0);
        assert_eq!(
            conformance::validate_record(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::unsupported)
        );
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            globals::encode(source.as_bytes(), &mut output, &mut workspace),
            Err(error::intent)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_globals_preserve_record_context_cfg_present_metadata_and_whole_file_ceiling() {
    checked(|| {
        let records =
            "# Struct definitions\n@ai_context(\"Record context\")\nstruct point\nend struct\n";
        let source = edition(records, "", "", "LOAD_GLOBAL value i64\nRET\nCONSOLE_WRITE");
        let mut workspace = validation_workspace::new();
        assert_eq!(
            conformance::validate_globals(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        let witness = conformance::validate_globals(source.as_bytes(), 1, &mut workspace).unwrap();
        assert_eq!(witness.intent(), None);
        assert_eq!(
            witness.program().record_layouts()[0].context(),
            Some("Record context")
        );
        assert_eq!(witness.required_services(), 1);
        assert_eq!(
            conformance::validate_globals(source.as_bytes(), 2, &mut workspace).err(),
            Some(error::ceiling)
        );
        let source = edition("", "", "@ai_context(\"\")\n", "LOAD_GLOBAL value i64\nRET");
        assert_eq!(
            conformance::validate_globals(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::intent)
        );
        let source = edition(
            "",
            "",
            "",
            "PUSH_CONST_STR A\nCAST text\nPOP\nPUSH_CONST 0\nRET",
        );
        assert_eq!(
            conformance::validate_globals(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::syntax)
        );
    });
}
