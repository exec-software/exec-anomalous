extern crate std;

use super::*;
use std::{format, string::String, vec, vec::Vec};

const intent: &str = "@ai_context(\"Exercise exact hygienic storage identities\")\n";
const helper: &str =
    "# declaration_line 1\nfunction identity(value: i64) -> i64:\nRET\nend function\n";

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
        "# NIL Textual Module\nmodule root\n{prefix}{intent}# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let size = hygienic_locals::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(size);
    output
}

fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0xa5; maximum_image_bytes];
    let result = hygienic_locals::encode(source.as_bytes(), &mut output, &mut workspace);
    if result.is_err() {
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
    result.map(|_| ())
}

#[test]
fn hygienic_profile_inherits_previous_syntax_and_changes_only_both_tags() {
    checked(|| {
        let source = authored(
            "# Struct definitions\nstruct child<i64>\nnumber: i64\nend struct\nstruct parent\nchild: child<i64>\nend struct\n# Enum definitions\nenum choice<i64>\nValue: i64\nend enum\n",
            "# source_debug 0 \"inherited.ny\" 0\nPUSH_CONST -0.0\nCAST f64\nPOP\nPUSH_CONST 1\nCAST bool\nCAST u8\nCAST i64\nSTORE_GLOBAL app::number i64\nLOAD_GLOBAL app::number i64\nCAST text\nPOP\nNEW_ARRAY_LITERAL 0 i64\nLEN\nPOP\nNEW_STRUCT parent\nNEW_STRUCT child<i64>\nPUSH_CONST 42\nSET_FIELD number\nSET_FIELD child\nGET_FIELD child\nGET_FIELD number\nNEW_VARIANT choice::Value 1 choice<i64> i64\nUNWRAP_VARIANT 1 choice<i64> i64\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let mut prior = vec![0; maximum_image_bytes];
        let size = nested_records::encode(source.as_bytes(), &mut prior, &mut workspace).unwrap();
        prior.truncate(size);
        assert_eq!(hygienic_locals::derivation_profile, 13);
        assert_eq!(get32(&image, 12), 13);
        assert_eq!(get32(&image, image_header_bytes + 12), 13);
        put32(&mut prior, 12, 13);
        put32(&mut prior, image_header_bytes + 12, 13);
        assert_eq!(image, prior);
        let admitted = hygienic_locals::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(admitted.profile(), 13);
        assert_eq!(admitted.edition(), source.as_bytes());
        assert_eq!(admitted.identity(), sha256(source.as_bytes()));
        assert_eq!(
            admitted.program().record_layouts()[1].field_kinds(),
            &[kind::record(0)]
        );
        assert_eq!(
            admitted
                .program()
                .source_debug(0)
                .unwrap()
                .instruction_index(),
            0
        );
    });
}

#[test]
fn hygienic_envelope_and_all_previous_grammars_remain_isolated() {
    checked(|| {
        let source = authored(
            "",
            "PUSH_CONST 42\nSTORE_LOCAL $ny.local.0.code\nLOAD_LOCAL $ny.local.0.code\nRET",
        );
        let image = package(&source);
        for outer in [0, 1, 2, 11, 12, 13, 14, u32::MAX] {
            for inner in [0, 1, 12, 13, 14, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                assert_eq!(
                    hygienic_locals::admit(&changed, 0, &mut workspace).err(),
                    if outer == 13 && inner == 13 {
                        None
                    } else {
                        Some(error::profile)
                    }
                );
            }
        }
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            anomalous_program_manifest::encode(source.as_bytes(), &mut output),
            Err(error::syntax)
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
        ] {
            let mut workspace = validation_workspace::new();
            assert_eq!(
                encode_profile(source.as_bytes(), &mut output, &mut workspace, profile),
                Err(error::syntax),
                "profile {}",
                profile.tag()
            );
            assert!(output.iter().all(|byte| *byte == 0xa5));
        }
    });
}

#[test]
fn full_spelling_preserves_plain_temporary_and_repeated_binding_identity() {
    checked(|| {
        let source = authored(
            "",
            "PUSH_CONST 7\nSTORE_LOCAL code\nPUSH_CONST 1\nCAST bool\nSTORE_LOCAL $ny.local.0.code\nPUSH_CONST 0.5\nSTORE_LOCAL $ny.local.1.code\nPUSH_CONST 255\nCAST u8\nSTORE_LOCAL $ny.temp.code.0\nPUSH_CONST 0\nCAST bool\nSTORE_LOCAL $ny.local.0.code\nLOAD_LOCAL $ny.local.1.code\nCAST f64\nPOP\nLOAD_LOCAL $ny.temp.code.0\nCAST i64\nPOP\nLOAD_LOCAL $ny.local.0.code\nCAST i64\nLOAD_LOCAL code\nADD i64\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let admitted = hygienic_locals::admit(&image, 0, &mut workspace).unwrap();
        let function = &admitted.program().functions()[0];
        assert_eq!(function.local_count, 4);
        assert_eq!(
            &function.local_names[..4],
            &[
                "code",
                "$ny.local.0.code",
                "$ny.local.1.code",
                "$ny.temp.code.0"
            ]
        );
        let stores: Vec<_> = admitted
            .program()
            .instructions()
            .iter()
            .filter_map(|item| {
                if let operation::store(local) = item.operation {
                    Some(local)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(stores, [0, 1, 2, 3, 1]);
    });
}

#[test]
fn hygienic_spelling_is_canonical_bounded_text_not_a_numeric_index() {
    checked(|| {
        for name in [
            "$ny.local.0.x",
            "$ny.local.9._",
            "$ny.local.10.Name_2",
            "$ny.local.18446744073709551616.value",
            "$ny.local.999.println",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST 42\nSTORE_LOCAL {name}\nLOAD_LOCAL {name}\nRET")
                )),
                Ok(())
            );
        }
        for name in [
            "$ny.local.",
            "$ny.local.0",
            "$ny.local..x",
            "$ny.local.0.",
            "$ny.local.00.x",
            "$ny.local.01.x",
            "$ny.local.+1.x",
            "$ny.local.-1.x",
            "$ny.local.1_0.x",
            "$ny.local.0x1.x",
            "$ny.local.1.2x",
            "$ny.local.1.x.y",
            "$ny.local.1.x::y",
            "$ny.local.1.x-y",
            "$ny.local.１.x",
            "$ny.local.1.é",
            "$ny.local.1.$ny.temp.x.0",
            "$NY.local.0.x",
            "$ny.locality.0.x",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST 42\nSTORE_LOCAL {name}\nLOAD_LOCAL {name}\nRET")
                )),
                Err(error::syntax),
                "{name}"
            );
        }
        let longest_name = format!("$ny.local.0.{}", "x".repeat(116));
        let longest_counter = format!("$ny.local.{}.x", "9".repeat(116));
        for name in [longest_name, longest_counter] {
            assert_eq!(name.len(), maximum_name_bytes);
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST 42\nSTORE_LOCAL {name}\nLOAD_LOCAL {name}\nRET")
                )),
                Ok(())
            );
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST 42\nSTORE_LOCAL {name}x\nPUSH_CONST 0\nRET")
                )),
                Err(error::limit)
            );
        }
        assert_eq!(
            validate(&authored(
                "",
                &format!(
                    "PUSH_CONST 0\nRET\nLOAD_LOCAL $ny.local.00.{}",
                    "x".repeat(117)
                )
            )),
            Err(error::limit)
        );
    });
}

#[test]
fn generated_names_do_not_expand_parameters_or_other_identifier_namespaces() {
    checked(|| {
        for prefix in [
            "# declaration_line 1\nfunction other($ny.local.0.value: i64) -> i64:\nRET\nend function\n",
            "# declaration_line 1\nfunction $ny.local.0.other -> i64:\nPUSH_CONST 0\nRET\nend function\n",
            "# String constants\nconst $ny.local.0.text = \"text\"\n",
            "# Struct definitions\nstruct $ny.local.0.record\nend struct\n",
            "# Struct definitions\nstruct record\n$ny.local.0.field: i64\nend struct\n",
            "# Enum definitions\nenum choice\n$ny.local.0.Unit\nend enum\n",
        ] {
            assert_eq!(
                validate(&authored(prefix, "PUSH_CONST 0\nRET")),
                Err(error::syntax)
            );
        }
        for instruction in [
            "LABEL $ny.local.0.label",
            "JMP $ny.local.0.label",
            "PUSH_FN_PTR $ny.local.0.identity",
            "LOAD_GLOBAL $ny.local.0.slot i64",
            "STORE_GLOBAL $ny.local.0.slot i64",
        ] {
            assert_eq!(
                validate(&authored("", &format!("PUSH_CONST 0\nRET\n{instruction}"))),
                Err(error::syntax),
                "{instruction}"
            );
        }
        for instruction in [
            "REF_LOCAL $ny.local.0.x",
            "BINDING_REF_LOCAL $ny.local.0.x",
            "OWN_BIND $ny.local.0.x",
            "OWN_MOVE $ny.local.0.x",
        ] {
            assert_eq!(
                validate(&authored("", &format!("PUSH_CONST 0\nRET\n{instruction}"))),
                Err(error::unsupported)
            );
        }
    });
}

#[test]
fn lexical_storage_remains_function_local_and_parameter_names_stay_distinct() {
    checked(|| {
        let functions = "# declaration_line 1\nfunction first(value: i64) -> i64:\nSTORE_LOCAL value\nPUSH_CONST 1\nCAST bool\nSTORE_LOCAL $ny.local.0.value\nLOAD_LOCAL $ny.local.0.value\nCAST i64\nLOAD_LOCAL value\nADD i64\nRET\nend function\n# declaration_line 1\nfunction second -> f64:\nPUSH_CONST -0.0\nSTORE_LOCAL $ny.local.0.value\nLOAD_LOCAL $ny.local.0.value\nRET\nend function\n";
        let source = authored(
            functions,
            "PUSH_CONST 41\nCALL first 1\nCALL second 0\nPOP\nSTORE_LOCAL $ny.local.0.value\nLOAD_LOCAL $ny.local.0.value\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let admitted = hygienic_locals::admit(&image, 0, &mut workspace).unwrap();
        let functions = admitted.program().functions();
        assert_eq!(
            functions.iter().map(|f| f.local_count).collect::<Vec<_>>(),
            [2, 1, 1]
        );
        assert_eq!(functions[0].parameter_names[0], "value");
        assert_eq!(functions[0].local_names[1], functions[1].local_names[0]);
        assert_eq!(functions[1].local_names[0], functions[2].local_names[0]);
    });
}

#[test]
fn hygienic_slots_keep_initialization_and_kind_checks_at_cfg_joins() {
    checked(|| {
        for body in [
            "LOAD_LOCAL $ny.local.0.x\nRET",
            "PUSH_CONST 1\nSTORE_LOCAL x\nLOAD_LOCAL $ny.local.0.x\nRET",
            "PUSH_CONST 1\nSTORE_LOCAL $ny.local.0.x\nLOAD_LOCAL $ny.local.1.x\nRET",
            "PUSH_CONST 1\nSTORE_LOCAL $ny.local.0.x\nPUSH_CONST 0\nCAST bool\nSTORE_LOCAL $ny.local.0.x\nPUSH_CONST 0\nRET",
            "PUSH_CONST 1\nJMP_IF_NOT joined\nPUSH_CONST 42\nSTORE_LOCAL $ny.local.0.x\nLABEL joined\nLOAD_LOCAL $ny.local.0.x\nRET",
            "PUSH_CONST 1\nJMP_IF_NOT alternate\nPUSH_CONST 42\nSTORE_LOCAL $ny.local.0.x\nJMP joined\nLABEL alternate\nPUSH_CONST 42\nSTORE_LOCAL $ny.local.1.x\nLABEL joined\nLOAD_LOCAL $ny.local.0.x\nRET",
            "PUSH_CONST 1\nJMP_IF_NOT alternate\nPUSH_CONST 42\nSTORE_LOCAL $ny.local.0.x\nJMP joined\nLABEL alternate\nPUSH_CONST 1\nCAST bool\nSTORE_LOCAL $ny.local.0.x\nLABEL joined\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(validate(&authored("", body)), Err(error::syntax), "{body}");
        }
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 1\nJMP_IF_NOT alternate\nPUSH_CONST 42\nSTORE_LOCAL $ny.local.0.x\nJMP joined\nLABEL alternate\nPUSH_CONST 7\nSTORE_LOCAL $ny.local.0.x\nLABEL joined\nLOAD_LOCAL $ny.local.0.x\nRET"
            )),
            Ok(())
        );
        let unused = "# declaration_line 1\nfunction unused -> i64:\nLOAD_LOCAL $ny.local.0.x\nRET\nend function\n";
        assert_eq!(
            validate(&authored(unused, "PUSH_CONST 0\nRET")),
            Err(error::syntax)
        );
    });
}

#[test]
fn exact_generated_call_names_keep_existing_local_first_resolution_and_signatures() {
    checked(|| {
        let source = authored(
            helper,
            "PUSH_FN_PTR identity\nSTORE_LOCAL $ny.local.0.identity\nPUSH_FN_PTR identity\nSTORE_LOCAL $ny.local.0.println\nPUSH_FN_PTR identity\nSTORE_LOCAL println\nPUSH_CONST 42\nCALL $ny.local.0.identity 1\nCALL $ny.local.0.println 1\nCALL println 1\nCALL identity 1\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let admitted = hygienic_locals::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(admitted.required_services(), 0);
        let calls: Vec<_> = admitted
            .program()
            .instructions()
            .iter()
            .filter_map(|item| match item.operation {
                operation::call_local { local, arguments } => Some((local, arguments)),
                _ => None,
            })
            .collect();
        assert_eq!(calls, [(0, 1), (1, 1), (2, 1)]);
        assert!(
            admitted
                .program()
                .instructions()
                .iter()
                .any(|item| item.operation == operation::call(0))
        );
        for body in [
            "PUSH_CONST 42\nCALL $ny.local.0.identity 1\nRET",
            "PUSH_CONST 0\nRET\nCALL $ny.local.0.println 1",
        ] {
            assert_eq!(validate(&authored(helper, body)), Err(error::unsupported));
        }
        for body in [
            "PUSH_CONST 7\nSTORE_LOCAL $ny.local.0.identity\nCALL $ny.local.0.identity 0\nRET",
            "PUSH_FN_PTR identity\nSTORE_LOCAL $ny.local.0.identity\nCALL $ny.local.0.identity 0\nRET",
            "PUSH_CONST 42\nCALL $ny.local.0.identity 1\nRET\nPUSH_FN_PTR identity\nSTORE_LOCAL $ny.local.0.identity",
        ] {
            assert_eq!(validate(&authored(helper, body)), Err(error::syntax));
        }
        let different =
            "# declaration_line 1\nfunction other(value: bool) -> bool:\nRET\nend function\n";
        assert_eq!(
            validate(&authored(
                &format!("{helper}{different}"),
                "PUSH_FN_PTR identity\nSTORE_LOCAL $ny.local.0.selected\nPUSH_FN_PTR other\nSTORE_LOCAL $ny.local.0.selected\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
    });
}

#[test]
fn all_local_names_share_existing_capacity_without_counter_based_allocation() {
    checked(|| {
        let mut body = String::from(
            "PUSH_CONST 0\nSTORE_LOCAL plain\nPUSH_CONST 0\nSTORE_LOCAL $ny.temp.value.0\n",
        );
        for index in 0..126 {
            body.push_str(&format!(
                "PUSH_CONST 0\nSTORE_LOCAL $ny.local.{index}.value\n"
            ));
        }
        for _ in 0..16 {
            body.push_str("PUSH_CONST 42\nSTORE_LOCAL $ny.local.125.value\n");
        }
        let source = authored("", &format!("{body}LOAD_LOCAL $ny.local.125.value\nRET"));
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        assert_eq!(
            hygienic_locals::admit(&image, 0, &mut workspace)
                .unwrap()
                .program()
                .functions()[0]
                .local_count,
            maximum_locals
        );
        assert_eq!(
            validate(&authored(
                "",
                &format!("{body}PUSH_CONST 0\nSTORE_LOCAL $ny.local.126.value\nPUSH_CONST 0\nRET")
            )),
            Err(error::limit)
        );
        let huge = format!("$ny.local.{}.value", "9".repeat(100));
        let image = package(&authored(
            "",
            &format!("PUSH_CONST 42\nSTORE_LOCAL {huge}\nLOAD_LOCAL {huge}\nRET"),
        ));
        assert_eq!(
            hygienic_locals::admit(&image, 0, &mut workspace)
                .unwrap()
                .program()
                .functions()[0]
                .local_count,
            1
        );
    });
}

#[test]
fn metadata_retains_source_and_storage_without_creating_or_typing_a_slot() {
    checked(|| {
        let metadata =
            "# source_debug 0 \"scope.ny\" 1 \"value\" \"$ny.local.0.value\" \"bool\" 2\n";
        let source = authored(
            "",
            &format!(
                "{metadata}PUSH_CONST 42\nSTORE_LOCAL $ny.local.0.value\nLOAD_LOCAL $ny.local.0.value\nRET"
            ),
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let admitted = hygienic_locals::admit(&image, 0, &mut workspace).unwrap();
        let binding = admitted
            .program()
            .source_debug(0)
            .unwrap()
            .bindings()
            .next()
            .unwrap();
        assert_eq!(binding.name.bytes().collect::<Vec<_>>(), b"value");
        assert_eq!(
            binding.storage.bytes().collect::<Vec<_>>(),
            b"$ny.local.0.value"
        );
        assert_eq!(binding.lexical_type.bytes().collect::<Vec<_>>(), b"bool");
        assert_eq!(binding.scope_depth, 2);
        let metadata_only = authored("", &format!("{metadata}PUSH_CONST 0\nRET"));
        let image = package(&metadata_only);
        assert_eq!(
            hygienic_locals::admit(&image, 0, &mut workspace)
                .unwrap()
                .program()
                .functions()[0]
                .local_count,
            0
        );
        let mut prior = vec![0; maximum_image_bytes];
        assert!(
            nested_records::encode(metadata_only.as_bytes(), &mut prior, &mut workspace).is_ok()
        );
        assert_eq!(
            validate(&authored(
                "",
                &format!("{metadata}LOAD_LOCAL $ny.local.0.value\nRET")
            )),
            Err(error::syntax)
        );
    });
}

#[test]
fn workspace_reuse_clears_hygienic_names_without_widening_previous_profiles() {
    checked(|| {
        let generated = authored(
            "",
            "PUSH_CONST 42\nSTORE_LOCAL $ny.local.0.value\nLOAD_LOCAL $ny.local.0.value\nRET",
        );
        let plain = authored(
            "",
            "PUSH_CONST 42\nSTORE_LOCAL value\nLOAD_LOCAL value\nRET",
        );
        let mut workspace = validation_workspace::new();
        let mut output = vec![0; maximum_image_bytes];
        let size =
            hygienic_locals::encode(generated.as_bytes(), &mut output, &mut workspace).unwrap();
        let image = output[..size].to_vec();
        assert_eq!(
            hygienic_locals::admit(&image, 0, &mut workspace)
                .unwrap()
                .program()
                .functions()[0]
                .local_names[0],
            "$ny.local.0.value"
        );
        let size = nested_records::encode(plain.as_bytes(), &mut output, &mut workspace).unwrap();
        let image = output[..size].to_vec();
        assert_eq!(
            nested_records::admit(&image, 0, &mut workspace)
                .unwrap()
                .program()
                .functions()[0]
                .local_names[0],
            "value"
        );
        assert_eq!(
            nested_records::encode(generated.as_bytes(), &mut output, &mut workspace),
            Err(error::syntax)
        );
        assert!(hygienic_locals::encode(generated.as_bytes(), &mut output, &mut workspace).is_ok());
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_hygienic_validation_preserves_intent_and_whole_file_authority() {
    checked(|| {
        let source = authored("", "PUSH_CONST 42\nSTORE_LOCAL $ny.local.0.value\nLOAD_LOCAL $ny.local.0.value\nRET\nCALL println 1").replace(intent, "");
        assert_eq!(validate(&source), Err(error::intent));
        let mut workspace = validation_workspace::new();
        assert_eq!(
            conformance::validate_hygienic_locals(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        let raw =
            conformance::validate_hygienic_locals(source.as_bytes(), 1, &mut workspace).unwrap();
        assert_eq!(raw.profile(), 13);
        assert_eq!(raw.intent(), None);
        assert_eq!(raw.required_services(), authority::console_write);
        assert_eq!(raw.identity(), sha256(source.as_bytes()));
        assert_eq!(
            conformance::validate_nested_records(source.as_bytes(), 1, &mut workspace).err(),
            Some(error::syntax)
        );
        let malformed = source.replace(
            "STORE_LOCAL $ny.local.0.value",
            "STORE_LOCAL $ny.local.00.value",
        );
        assert_eq!(
            conformance::validate_hygienic_locals(malformed.as_bytes(), 1, &mut workspace).err(),
            Some(error::syntax)
        );
    });
}
