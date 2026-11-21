extern crate std;

use super::*;
use std::{format, string::String, vec, vec::Vec};

const intent: &str = "@ai_context(\"Exercise bounded indexed storage\")\n";

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
    let size = indexed_storage::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(size);
    output
}

fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0xa5; maximum_image_bytes];
    let result = indexed_storage::encode(source.as_bytes(), &mut output, &mut workspace);
    if result.is_err() {
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
    result.map(|_| ())
}

#[test]
fn indexed_storage_inherits_hygienic_names_and_changes_only_both_tags() {
    checked(|| {
        let source = authored(
            "# Struct definitions\nstruct child<i64>\nnumber: i64\nend struct\nstruct parent\nchild: child<i64>\nend struct\n",
            "# source_debug 0 \"inherited.ny\" 0\nPUSH_CONST -0.0\nCAST f64\nPOP\nPUSH_CONST 1\nCAST bool\nCAST u8\nCAST i64\nSTORE_GLOBAL number i64\nNEW_STRUCT parent\nNEW_STRUCT child<i64>\nLOAD_GLOBAL number i64\nSET_FIELD number\nSET_FIELD child\nGET_FIELD child\nGET_FIELD number\nSTORE_LOCAL $ny.local.0.value\nNEW_ARRAY_LITERAL 0 text\nLEN\nPOP\nLOAD_LOCAL $ny.local.0.value\nRET",
        );
        let image = package(&source);
        let mut previous = vec![0; maximum_image_bytes];
        let mut workspace = validation_workspace::new();
        let size =
            hygienic_locals::encode(source.as_bytes(), &mut previous, &mut workspace).unwrap();
        previous.truncate(size);
        assert_eq!(indexed_storage::derivation_profile, 14);
        assert_eq!(get32(&image, 12), 14);
        assert_eq!(get32(&image, image_header_bytes + 12), 14);
        put32(&mut previous, 12, 14);
        put32(&mut previous, image_header_bytes + 12, 14);
        assert_eq!(image, previous);
        let admitted = indexed_storage::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(admitted.profile(), 14);
        assert_eq!(admitted.edition(), source.as_bytes());
        assert_eq!(admitted.identity(), sha256(source.as_bytes()));
        assert_eq!(admitted.required_services(), 0);
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
fn indexed_storage_requires_both_tags_and_preserves_every_previous_grammar() {
    checked(|| {
        let source = authored("", "PUSH_CONST 1\nALLOC i64\nFREE\nPUSH_CONST 0\nRET");
        let image = package(&source);
        for outer in [0, 1, 2, 12, 13, 14, 15, u32::MAX] {
            for inner in [0, 1, 12, 13, 14, 15, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                assert_eq!(
                    indexed_storage::admit(&changed, 0, &mut workspace).err(),
                    if outer == 14 && inner == 14 {
                        None
                    } else {
                        Some(error::profile)
                    }
                );
            }
        }
        let mut workspace = validation_workspace::new();
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            anomalous_program_manifest::admit(&image, 0).err(),
            Some(error::profile)
        );
        assert_eq!(
            anomalous_program_manifest::encode(source.as_bytes(), &mut output),
            Err(error::unsupported)
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
        ] {
            assert_eq!(
                admit_profile(&image, 0, &mut workspace, profile).err(),
                Some(error::profile)
            );
            for opcode in ["ALLOC i64", "FREE", "SET_INDEX", "CALL array__push 2"] {
                let source = authored("", &format!("PUSH_CONST 0\nRET\n{opcode}"));
                let mut workspace = validation_workspace::new();
                assert_eq!(
                    encode_profile(source.as_bytes(), &mut output, &mut workspace, profile),
                    Err(error::unsupported)
                );
            }
        }
        assert!(output.iter().all(|byte| *byte == 0xa5));
    });
}

#[test]
fn indexed_operations_have_exact_operand_order_and_void_results() {
    checked(|| {
        let source = authored(
            "",
            "NEW_ARRAY_LITERAL 0 i64\nSTORE_LOCAL values\nLOAD_LOCAL values\nPUSH_CONST 42\nCALL array__push 2\nLOAD_LOCAL values\nPUSH_CONST 0\nPUSH_CONST 7\nSET_INDEX\nPUSH_CONST 2\nALLOC i64\nSTORE_LOCAL $ny.local.0.buffer\nLOAD_LOCAL $ny.local.0.buffer\nPUSH_CONST 1\nPUSH_CONST 35\nSET_INDEX\nLOAD_LOCAL values\nPUSH_CONST 0\nGET_INDEX\nLOAD_LOCAL $ny.local.0.buffer\nPUSH_CONST 1\nGET_INDEX\nADD\nLOAD_LOCAL $ny.local.0.buffer\nFREE\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let admitted = indexed_storage::admit(&image, 0, &mut workspace).unwrap();
        let operations: Vec<_> = admitted
            .program()
            .instructions()
            .iter()
            .map(|item| item.operation)
            .filter(|operation| {
                matches!(
                    operation,
                    operation::array_set
                        | operation::array_push
                        | operation::buffer_alloc
                        | operation::buffer_free
                        | operation::array_get
                )
            })
            .collect();
        assert_eq!(
            operations,
            [
                operation::array_push,
                operation::array_set,
                operation::buffer_alloc,
                operation::array_set,
                operation::array_get,
                operation::array_get,
                operation::buffer_free
            ]
        );
        assert_eq!(admitted.required_services(), 0);
        for body in [
            "NEW_ARRAY_LITERAL 0 i64\nPUSH_CONST 0\nPUSH_CONST 1\nSET_INDEX\nPOP\nPUSH_CONST 0\nRET",
            "NEW_ARRAY_LITERAL 0 i64\nPUSH_CONST 1\nCALL array__push 2\nPOP\nPUSH_CONST 0\nRET",
            "PUSH_CONST 1\nALLOC i64\nFREE\nPOP\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(validate(&authored("", body)), Err(error::syntax));
        }
    });
}

#[test]
fn array_mutation_preserves_existing_direct_and_indirect_array_signatures() {
    checked(|| {
        let helper = "# declaration_line 1\nfunction append(values: array<i64>, value: i64) -> array<i64>:\nSTORE_LOCAL value\nSTORE_LOCAL values\nLOAD_LOCAL values\nLOAD_LOCAL value\nCALL array__push 2\nLOAD_LOCAL values\nPUSH_CONST 0\nLOAD_LOCAL value\nSET_INDEX\nLOAD_LOCAL values\nRET\nend function\n";
        assert_eq!(
            validate(&authored(
                helper,
                "NEW_ARRAY_LITERAL 0 i64\nPUSH_CONST 7\nCALL append 2\nPUSH_FN_PTR append\nSTORE_LOCAL selected\nPUSH_CONST 42\nCALL selected 2\nPUSH_CONST 0\nGET_INDEX\nRET"
            )),
            Ok(())
        );
        assert_eq!(
            validate(&authored(
                helper,
                "PUSH_CONST 1\nALLOC i64\nPUSH_CONST 7\nCALL append 2\nLEN\nRET"
            )),
            Err(error::syntax)
        );
    });
}

#[test]
fn array_push_fallback_preserves_local_and_declared_function_precedence() {
    checked(|| {
        let helpers = "# declaration_line 1\nfunction identity(value: i64) -> i64:\nRET\nend function\n# declaration_line 1\nfunction array__push(value: i64) -> i64:\nRET\nend function\n";
        let source = authored(helpers, "PUSH_CONST 42\nCALL array__push 1\nRET");
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let admitted = indexed_storage::admit(&image, 0, &mut workspace).unwrap();
        assert!(
            admitted
                .program()
                .instructions()
                .iter()
                .any(|item| item.operation == operation::call(1))
        );
        let source = authored(
            helpers,
            "PUSH_FN_PTR identity\nSTORE_LOCAL array__push\nPUSH_CONST 42\nCALL array__push 1\nRET",
        );
        let image = package(&source);
        let admitted = indexed_storage::admit(&image, 0, &mut workspace).unwrap();
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
        let parameter = "# declaration_line 1\nfunction identity(array__push: i64) -> i64:\nSTORE_LOCAL array__push\nLOAD_LOCAL array__push\nRET\nend function\n";
        assert_eq!(
            validate(&authored(parameter, "PUSH_CONST 42\nCALL identity 1\nRET")),
            Ok(())
        );
        for source in [
            authored(
                helpers,
                "NEW_ARRAY_LITERAL 0 i64\nPUSH_CONST 42\nCALL array__push 2\nPUSH_CONST 0\nRET",
            ),
            authored(
                "",
                "NEW_ARRAY_LITERAL 0 i64\nPUSH_CONST 42\nCALL array__push 2\nPUSH_CONST 0\nRET\nPUSH_CONST 0\nSTORE_LOCAL array__push",
            ),
            authored(
                "",
                "PUSH_CONST 0\nSTORE_LOCAL array__push\nNEW_ARRAY_LITERAL 0 i64\nPUSH_CONST 42\nCALL array__push 2\nPUSH_CONST 0\nRET",
            ),
        ] {
            assert_eq!(validate(&source), Err(error::syntax));
        }
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 0\nRET\nCALL $ny.local.0.array__push 2"
            )),
            Err(error::unsupported)
        );
    });
}

#[test]
fn new_opcode_grammar_is_exact_even_when_dead_or_in_an_unused_function() {
    checked(|| {
        for opcode in [
            "SET_INDEX i64",
            "SET_INDEX 0",
            "ALLOC",
            "ALLOC i32",
            "ALLOC text",
            "ALLOC bool",
            "ALLOC u8",
            "ALLOC f64",
            "ALLOC ptr<i64>",
            "ALLOC array<i64>",
            "ALLOC i64 1",
            "FREE i64",
            "FREE 0",
            "OWN_ALLOC i64",
            "READ_PTR",
            "WRITE_PTR",
            "CALL array__push 0",
            "CALL array__push 1",
            "CALL array__push 3",
            "CALL array__set 3",
            "CALL array__push 2 i64",
        ] {
            assert_eq!(
                validate(&authored("", &format!("PUSH_CONST 0\nRET\n{opcode}"))),
                Err(error::unsupported),
                "{opcode}"
            );
            let helper = format!(
                "# declaration_line 1\nfunction unused -> void:\n{opcode}\nRET\nend function\n"
            );
            assert_eq!(
                validate(&authored(&helper, "PUSH_CONST 0\nRET")),
                Err(error::unsupported),
                "{opcode}"
            );
        }
        assert_eq!(
            validate(&authored("", "PUSH_CONST 0\nRET\nCALL array__push 9")),
            Err(error::limit)
        );
        assert_eq!(
            validate(&authored("", "PUSH_CONST 0\nRET\nCALL array__push -2")),
            Err(error::syntax)
        );
    });
}

#[test]
fn allocation_requires_an_explicit_integer_and_free_requires_an_opaque_buffer() {
    checked(|| {
        for value in [
            "",
            "PUSH_CONST_STR text_value",
            "PUSH_CONST 1\nCAST bool",
            "PUSH_CONST 1\nCAST u8",
            "PUSH_CONST 1.0",
            "NEW_ARRAY_LITERAL 0 i64",
            "NEW_ARRAY_LITERAL 0 text",
            "PUSH_CONST 1\nALLOC i64",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("{value}\nALLOC i64\nFREE\nPUSH_CONST 0\nRET")
                )),
                Err(error::syntax),
                "{value}"
            );
        }
        for value in [
            "",
            "PUSH_CONST 1",
            "PUSH_CONST_STR text_value",
            "NEW_ARRAY_LITERAL 0 i64",
            "NEW_ARRAY_LITERAL 0 text",
        ] {
            assert_eq!(
                validate(&authored("", &format!("{value}\nFREE\nPUSH_CONST 0\nRET"))),
                Err(error::syntax),
                "{value}"
            );
        }
        for count in [i64::MIN, -1, 0, 1, 1024, 1025, i64::MAX] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST {count}\nALLOC i64\nFREE\nPUSH_CONST 0\nRET")
                )),
                Ok(())
            );
        }
    });
}

#[test]
fn indexed_mutation_checks_receiver_index_and_value_kinds_without_widening_text() {
    checked(|| {
        for receiver in [
            "PUSH_CONST 1",
            "PUSH_CONST_STR text_value",
            "NEW_ARRAY_LITERAL 0 text",
            "PUSH_CONST 1\nCAST bool",
            "PUSH_CONST 1.0",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!(
                        "{receiver}\nPUSH_CONST 0\nPUSH_CONST 42\nSET_INDEX\nPUSH_CONST 0\nRET"
                    )
                )),
                Err(error::syntax)
            );
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("{receiver}\nPUSH_CONST 42\nCALL array__push 2\nPUSH_CONST 0\nRET")
                )),
                Err(error::syntax)
            );
        }
        for value in [
            "PUSH_CONST_STR text_value",
            "PUSH_CONST 1\nCAST bool",
            "PUSH_CONST 1\nCAST u8",
            "PUSH_CONST 1.0",
            "PUSH_CONST 1\nALLOC i64",
        ] {
            for receiver in ["NEW_ARRAY_LITERAL 0 i64", "PUSH_CONST 1\nALLOC i64"] {
                assert_eq!(
                    validate(&authored(
                        "",
                        &format!(
                            "{receiver}\n{value}\nPUSH_CONST 42\nSET_INDEX\nPUSH_CONST 0\nRET"
                        )
                    )),
                    Err(error::syntax)
                );
                assert_eq!(
                    validate(&authored(
                        "",
                        &format!("{receiver}\nPUSH_CONST 0\n{value}\nSET_INDEX\nPUSH_CONST 0\nRET")
                    )),
                    Err(error::syntax)
                );
            }
            assert_eq!(
                validate(&authored(
                    "",
                    &format!(
                        "NEW_ARRAY_LITERAL 0 i64\n{value}\nCALL array__push 2\nPUSH_CONST 0\nRET"
                    )
                )),
                Err(error::syntax)
            );
        }
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 1\nALLOC i64\nPUSH_CONST 42\nCALL array__push 2\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST_STR text_value\nNEW_ARRAY_LITERAL 1 text\nPUSH_CONST 0\nGET_INDEX\nPOP\nPUSH_CONST 0\nRET"
            )),
            Ok(())
        );
        for body in [
            "SET_INDEX",
            "PUSH_CONST 0\nSET_INDEX",
            "PUSH_CONST 0\nPUSH_CONST 1\nSET_INDEX",
            "CALL array__push 2",
            "NEW_ARRAY_LITERAL 0 i64\nCALL array__push 2",
        ] {
            assert_eq!(
                validate(&authored("", &format!("{body}\nPUSH_CONST 0\nRET"))),
                Err(error::syntax)
            );
        }
    });
}

#[test]
fn buffers_cannot_escape_into_integer_array_record_or_authority_operations() {
    checked(|| {
        let declarations = "# Struct definitions\nstruct item\nnumber: i64\nend struct\n# declaration_line 1\nfunction identity(value: i64) -> i64:\nRET\nend function\n";
        for suffix in [
            "LEN\nRET",
            "CAST i64\nRET",
            "CAST bool\nPOP\nPUSH_CONST 0\nRET",
            "CAST text\nPOP\nPUSH_CONST 0\nRET",
            "STORE_GLOBAL item i64\nPUSH_CONST 0\nRET",
            "PUSH_CONST 1\nADD\nRET",
            "PUSH_CONST 1\nCMP_EQ\nRET",
            "NEG i64\nRET",
            "NEW_ARRAY_LITERAL 1 i64\nLEN\nRET",
            "CALL identity 1\nRET",
            "CONSOLE_WRITE\nPUSH_CONST 0\nRET",
            "RET",
            "JMP_IF end\nLABEL end\nPUSH_CONST 0\nRET",
            "STORE_LOCAL buffer\nNEW_STRUCT item\nLOAD_LOCAL buffer\nSET_FIELD number\nGET_FIELD number\nRET",
            "STORE_LOCAL buffer\nPUSH_CONST 1\nSTORE_LOCAL buffer\nPUSH_CONST 0\nRET",
        ] {
            assert_eq!(
                validate(&authored(
                    declarations,
                    &format!("PUSH_CONST 1\nALLOC i64\n{suffix}")
                )),
                Err(error::syntax),
                "{suffix}"
            );
        }
        for spelling in ["buffer_integer", "buffer<i64>", "ptr<i64>", "owned<i64>"] {
            let helper = format!(
                "# declaration_line 1\nfunction identity(value: {spelling}) -> {spelling}:\nRET\nend function\n"
            );
            assert_eq!(
                validate(&authored(&helper, "PUSH_CONST 0\nRET")),
                Err(error::unsupported)
            );
        }
    });
}

#[test]
fn buffer_alias_liveness_is_runtime_policy_but_cfg_kinds_and_initialization_are_static() {
    checked(|| {
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 1\nALLOC i64\nSTORE_LOCAL original\nLOAD_LOCAL original\nSTORE_LOCAL alias\nLOAD_LOCAL original\nFREE\nLOAD_LOCAL alias\nPUSH_CONST 0\nGET_INDEX\nPOP\nLOAD_LOCAL alias\nPUSH_CONST 0\nPUSH_CONST 42\nSET_INDEX\nLOAD_LOCAL alias\nFREE\nPUSH_CONST 0\nRET"
            )),
            Ok(())
        );
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 0\nJMP_IF other\nPUSH_CONST 1\nALLOC i64\nSTORE_LOCAL buffer\nJMP joined\nLABEL other\nPUSH_CONST 2\nALLOC i64\nSTORE_LOCAL buffer\nLABEL joined\nLOAD_LOCAL buffer\nFREE\nPUSH_CONST 0\nRET"
            )),
            Ok(())
        );
        for alternate in [
            "",
            "PUSH_CONST 0\nSTORE_LOCAL buffer",
            "NEW_ARRAY_LITERAL 0 i64\nSTORE_LOCAL buffer",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!(
                        "PUSH_CONST 0\nJMP_IF other\nPUSH_CONST 1\nALLOC i64\nSTORE_LOCAL buffer\nJMP joined\nLABEL other\n{alternate}\nLABEL joined\nLOAD_LOCAL buffer\nFREE\nPUSH_CONST 0\nRET"
                    )
                )),
                Err(error::syntax)
            );
        }
        let helper =
            "# declaration_line 1\nfunction unused -> void:\nALLOC i64\nFREE\nRET\nend function\n";
        assert_eq!(
            validate(&authored(helper, "PUSH_CONST 1\nCALL unused 0\nRET")),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 0\nRET\nSET_INDEX\nALLOC i64\nFREE\nCALL array__push 2"
            )),
            Ok(())
        );
    });
}

#[test]
fn opaque_buffers_share_existing_local_and_stack_validation_limits() {
    checked(|| {
        let mut body = String::new();
        for index in 0..maximum_locals {
            body.push_str(&format!(
                "PUSH_CONST 1\nALLOC i64\nSTORE_LOCAL $ny.local.{index}.buffer\n"
            ));
        }
        let source = authored("", &format!("{body}PUSH_CONST 0\nRET"));
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        assert_eq!(
            indexed_storage::admit(&image, 0, &mut workspace)
                .unwrap()
                .program()
                .functions()[0]
                .local_count,
            maximum_locals
        );
        assert_eq!(
            validate(&authored(
                "",
                &format!("{body}PUSH_CONST 1\nALLOC i64\nSTORE_LOCAL extra\nPUSH_CONST 0\nRET")
            )),
            Err(error::limit)
        );
        let mut body = "PUSH_CONST 1\nALLOC i64\n".repeat(maximum_stack_values);
        body.push_str(&"POP\n".repeat(maximum_stack_values));
        body.push_str("PUSH_CONST 0\nRET");
        assert_eq!(validate(&authored("", &body)), Ok(()));
        assert_eq!(
            validate(&authored("", &format!("PUSH_CONST 1\nALLOC i64\n{body}"))),
            Err(error::limit)
        );
    });
}

#[test]
fn source_debug_stays_descriptive_and_tracks_new_operations_without_extra_instructions() {
    checked(|| {
        let source = authored(
            "",
            "PUSH_CONST 1\n# source_debug 1 \"buffer.ny\" 1 \"buffer\" \"$ny.local.0.buffer\" \"ptr<i64>\" 0\nALLOC i64\nSTORE_LOCAL $ny.local.0.buffer\nLOAD_LOCAL $ny.local.0.buffer\n# source_debug 4 \"buffer.ny\" 0\nFREE\nPUSH_CONST 0\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let admitted = indexed_storage::admit(&image, 0, &mut workspace).unwrap();
        let plan = admitted.program();
        assert_eq!(plan.instructions().len(), 7);
        assert_eq!(plan.instructions()[1].operation, operation::buffer_alloc);
        assert_eq!(plan.instructions()[4].operation, operation::buffer_free);
        let binding = plan.source_debug(1).unwrap().bindings().next().unwrap();
        assert_eq!(
            binding.lexical_type.bytes().collect::<Vec<_>>(),
            b"ptr<i64>"
        );
        assert_eq!(admitted.required_services(), 0);
        let no_allocation = source.replace("ALLOC i64", "NOP");
        assert_eq!(validate(&no_allocation), Err(error::syntax));
    });
}

#[test]
fn workspace_reuse_cannot_leak_new_storage_kinds_or_call_resolution_into_prior_profiles() {
    checked(|| {
        let new = authored(
            "",
            "PUSH_CONST 1\nALLOC i64\nSTORE_LOCAL shared\nLOAD_LOCAL shared\nFREE\nNEW_ARRAY_LITERAL 0 i64\nPUSH_CONST 1\nCALL array__push 2\nPUSH_CONST 0\nRET",
        );
        let old = authored(
            "",
            "PUSH_CONST 42\nSTORE_LOCAL shared\nLOAD_LOCAL shared\nRET",
        );
        let mut output = vec![0; maximum_image_bytes];
        let mut workspace = validation_workspace::new();
        assert!(indexed_storage::encode(new.as_bytes(), &mut output, &mut workspace).is_ok());
        assert!(hygienic_locals::encode(old.as_bytes(), &mut output, &mut workspace).is_ok());
        assert_eq!(
            hygienic_locals::encode(new.as_bytes(), &mut output, &mut workspace),
            Err(error::unsupported)
        );
        assert!(indexed_storage::encode(new.as_bytes(), &mut output, &mut workspace).is_ok());
        let old_name = authored(
            "# declaration_line 1\nfunction array__push(value: i64) -> i64:\nRET\nend function\n",
            "PUSH_CONST 42\nCALL array__push 1\nRET",
        );
        assert!(hygienic_locals::encode(old_name.as_bytes(), &mut output, &mut workspace).is_ok());
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn raw_indexed_storage_validation_preserves_production_intent_and_whole_file_authority() {
    checked(|| {
        let source = authored("", "PUSH_CONST 1\nALLOC i64\nFREE\nNEW_ARRAY_LITERAL 0 i64\nPUSH_CONST 42\nCALL array__push 2\nPUSH_CONST 0\nRET\nCALL println 1").replace(intent, "");
        assert_eq!(validate(&source), Err(error::intent));
        let mut workspace = validation_workspace::new();
        assert_eq!(
            conformance::validate_indexed_storage(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        let raw =
            conformance::validate_indexed_storage(source.as_bytes(), 1, &mut workspace).unwrap();
        assert_eq!(raw.profile(), 14);
        assert_eq!(raw.intent(), None);
        assert_eq!(raw.required_services(), authority::console_write);
        assert_eq!(raw.granted_services(), authority::console_write);
        assert_eq!(raw.identity(), sha256(source.as_bytes()));
        assert_eq!(
            conformance::validate_hygienic_locals(source.as_bytes(), 1, &mut workspace).err(),
            Some(error::unsupported)
        );
        let malformed = source.replace("ALLOC i64", "ALLOC u8");
        assert_eq!(
            conformance::validate_indexed_storage(malformed.as_bytes(), 1, &mut workspace).err(),
            Some(error::unsupported)
        );
    });
}
