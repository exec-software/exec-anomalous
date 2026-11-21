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
        "# NIL Textual Module\nmodule root\n{prefix}@ai_context(\"Exercise explicit typed scalars\")\n# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn helper(name: &str, value_type: &str) -> String {
    format!(
        "# declaration_line 1\nfunction {name}(value: {value_type}) -> {value_type}:\nRET\nend function\n"
    )
}

fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let length = typed_scalars::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(length);
    output
}

fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0xa5; maximum_image_bytes];
    let result = typed_scalars::encode(source.as_bytes(), &mut output, &mut workspace);
    if result.is_err() {
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
    result.map(|_| ())
}

#[test]
fn typed_scalar_profile_preserves_previous_image_bytes_except_both_tags() {
    checked(|| {
        let source = authored(
            "# Enum definitions\nenum state\nIdle\nend enum\n",
            "# source_debug 0 \"source.ny\" 0\nNEW_VARIANT state::Idle 0 state void\nIS_VARIANT state::Idle state void\nSTORE_GLOBAL app::g i64\nLOAD_GLOBAL app::g i64\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let mut previous = vec![0; maximum_image_bytes];
        let length =
            source_debug::encode(source.as_bytes(), &mut previous, &mut workspace).unwrap();
        previous.truncate(length);
        assert_eq!(typed_scalars::derivation_profile, 11);
        assert_eq!(get32(&image, 12), 11);
        assert_eq!(get32(&image, image_header_bytes + 12), 11);
        put32(&mut previous, 12, 11);
        put32(&mut previous, image_header_bytes + 12, 11);
        assert_eq!(image, previous);
        let witness = typed_scalars::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 11);
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(
            witness
                .program()
                .source_debug(0)
                .unwrap()
                .instruction_index(),
            0
        );
        assert_eq!(witness.required_services(), 0);
    });
}

#[test]
fn typed_scalar_headers_and_all_previous_grammars_remain_isolated() {
    checked(|| {
        let image = package(&authored("", "PUSH_CONST 0.5\nPOP\nPUSH_CONST 0\nRET"));
        for outer in [0, 1, 2, 9, 10, 11, 12, u32::MAX] {
            for inner in [0, 1, 10, 11, 12, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                assert_eq!(
                    typed_scalars::admit(&changed, 0, &mut workspace).err(),
                    if outer == 11 && inner == 11 {
                        None
                    } else {
                        Some(error::profile)
                    }
                );
            }
        }
        for source in [
            authored("", "PUSH_CONST 0.5\nPOP\nPUSH_CONST 0\nRET"),
            authored("", "PUSH_CONST 1\nCAST bool\nCAST i64\nRET"),
            authored("", "PUSH_CONST 255\nCAST u8\nCAST i64\nRET"),
            authored(&helper("identity", "bool"), "PUSH_CONST 0\nRET"),
            authored(&helper("identity", "u8"), "PUSH_CONST 0\nRET"),
            authored(&helper("identity", "f64"), "PUSH_CONST 0\nRET"),
        ] {
            let mut output = vec![0xa5; maximum_image_bytes];
            assert_eq!(
                anomalous_program_manifest::encode(source.as_bytes(), &mut output),
                // Profile 1 checks the complete header as a simple function
                // name before considering its unsupported parameter types.
                Err(if source.contains("function identity(") {
                    error::syntax
                } else {
                    error::unsupported
                }),
                "profile 1: {source}"
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
            ] {
                let mut workspace = validation_workspace::new();
                assert_eq!(
                    encode_profile(source.as_bytes(), &mut output, &mut workspace, profile),
                    Err(error::unsupported),
                    "profile {}: {source}",
                    profile.tag()
                );
                assert!(output.iter().all(|byte| *byte == 0xa5));
            }
        }
    });
}

#[test]
fn binary64_literals_retain_exact_rounding_subnormal_and_signed_zero_bits() {
    checked(|| {
        for (literal, expected) in [
            ("0.5", 0x3fe0_0000_0000_0000),
            (".5", 0x3fe0_0000_0000_0000),
            ("+1.", 0x3ff0_0000_0000_0000),
            ("-1E+0", 0xbff0_0000_0000_0000),
            ("1e3", 0x408f_4000_0000_0000),
            ("0.0", 0),
            ("+0e-999999999999999999999999", 0),
            ("-0.000E+999999999999999999999999", 1u64 << 63),
            ("-0e-999999999999999999999999", 1u64 << 63),
            ("4.9406564584124654e-324", 1),
            ("2.4703282292062328e-324", 1),
            ("-4.9406564584124654e-324", 0x8000_0000_0000_0001),
            ("2.2250738585072014e-308", 0x0010_0000_0000_0000),
            ("1.7976931348623157e308", 0x7fef_ffff_ffff_ffff),
            (
                "1.00000000000000011102230246251565404236316680908203125",
                0x3ff0_0000_0000_0000,
            ),
            (
                "1.00000000000000011102230246251565404236316680908203126",
                0x3ff0_0000_0000_0001,
            ),
            ("9007199254740993.0", 0x4340_0000_0000_0000),
            ("9007199254740995.0", 0x4340_0000_0000_0002),
        ] {
            let source = authored(
                "",
                &format!("PUSH_CONST {literal}\nCAST f64\nPOP\nPUSH_CONST 0\nRET"),
            );
            let image = package(&source);
            let mut workspace = validation_workspace::new();
            let witness = typed_scalars::admit(&image, 0, &mut workspace).unwrap();
            assert_eq!(
                witness.program().instructions()[0].operation,
                operation::float(expected),
                "{literal}"
            );
            assert_eq!(
                witness.program().instructions()[1].operation,
                operation::scalar_cast(scalar_cast::float_identity)
            );
        }
    });
}

#[test]
fn floating_literal_spelling_range_and_integer_radix_errors_are_distinct() {
    checked(|| {
        for literal in [
            ".", "+.", "-.e1", "1e", "1e+", "1e-", "1.2.3", "1e1e1", "1.0f64", "NaN", "inf",
            "-inf", "0x1.0p0", "0x1e.0", "true", "false", "+1", "1_000.0", "١.٠", "0e+bad",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST {literal}\nPOP\nPUSH_CONST 0\nRET")
                )),
                Err(error::unsupported),
                "{literal}"
            );
        }
        for literal in [
            "1e309",
            "-1e309",
            "1.7976931348623159e308",
            "1e-9999",
            "-1e-9999",
            "2.4703282292062327e-324",
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST {literal}\nPOP\nPUSH_CONST 0\nRET")
                )),
                Err(error::syntax),
                "{literal}"
            );
        }
        for (literal, expected) in [("0x1e", 30), ("+0XFE", 254), ("-0xE", -14), ("-0", 0)] {
            let image = package(&authored("", &format!("PUSH_CONST {literal}\nRET")));
            let mut workspace = validation_workspace::new();
            let witness = typed_scalars::admit(&image, 0, &mut workspace).unwrap();
            assert_eq!(
                witness.program().instructions()[0].operation,
                operation::integer(expected)
            );
        }
    });
}

#[test]
fn all_integral_cast_edges_are_explicit_and_boolean_range_is_a_runtime_check() {
    checked(|| {
        for prefix in [
            "PUSH_CONST 2",
            "PUSH_CONST 1\nCAST bool",
            "PUSH_CONST 255\nCAST u8",
        ] {
            for (target, expected) in [
                ("bool", scalar_cast::boolean),
                ("u8", scalar_cast::byte),
                ("i32", scalar_cast::integer32),
                ("i64", scalar_cast::integer64),
            ] {
                let image = package(&authored(
                    "",
                    &format!("{prefix}\nCAST {target}\nPOP\nPUSH_CONST 0\nRET"),
                ));
                let mut workspace = validation_workspace::new();
                let witness = typed_scalars::admit(&image, 0, &mut workspace).unwrap();
                assert_eq!(
                    witness.program().instructions()[prefix.lines().count()].operation,
                    operation::scalar_cast(expected)
                );
            }
        }
        for prefix in [
            "PUSH_CONST 0.5",
            "PUSH_CONST 0",
            "PUSH_CONST 1\nCAST bool",
            "PUSH_CONST 1\nCAST u8",
        ] {
            let targets: &[&str] = if prefix == "PUSH_CONST 0.5" {
                &["bool", "u8", "i32", "i64", "text"]
            } else {
                &["f64"]
            };
            for target in targets {
                assert_eq!(
                    validate(&authored(
                        "",
                        &format!("{prefix}\nCAST {target}\nPOP\nPUSH_CONST 0\nRET")
                    )),
                    Err(error::syntax)
                );
            }
        }
    });
}

#[test]
fn typed_helper_calls_and_function_values_keep_distinct_exact_signatures() {
    checked(|| {
        let mut functions = String::new();
        for (name, value_type) in [
            ("word32", "i32"),
            ("word64", "i64"),
            ("boolean", "bool"),
            ("byte", "u8"),
            ("floating", "f64"),
            ("same", "bool"),
        ] {
            functions.push_str(&helper(name, value_type));
        }
        let image = package(&authored(
            &functions,
            "PUSH_FN_PTR boolean\nSTORE_LOCAL target\nPUSH_CONST 1\nCAST bool\nCALL target 1\nCALL same 1\nCAST i64\nPOP\nPUSH_CONST -0.0\nCALL floating 1\nPOP\nPUSH_CONST 255\nCAST u8\nCALL byte 1\nCAST i64\nRET",
        ));
        let mut workspace = validation_workspace::new();
        let witness = typed_scalars::admit(&image, 0, &mut workspace).unwrap();
        for (index, expected) in [0, 1, 2, 3, 4, 2].iter().enumerate() {
            assert_eq!(witness.program().functions()[index].signature(), *expected);
        }
        assert_eq!(
            witness.program().functions()[2].parameter_kinds(),
            &[kind::boolean]
        );
        assert_eq!(
            witness.program().functions()[3].parameter_kinds(),
            &[kind::byte]
        );
        assert_eq!(
            witness.program().functions()[4].parameter_kinds(),
            &[kind::float]
        );
        for (target, input) in [
            ("boolean", "PUSH_CONST 1"),
            ("byte", "PUSH_CONST 255"),
            ("floating", "PUSH_CONST 1"),
            ("word64", "PUSH_CONST 1\nCAST bool"),
            ("boolean", "PUSH_CONST 1\nCAST u8"),
        ] {
            for call in [
                format!("CALL {target} 1"),
                format!("PUSH_FN_PTR {target}\nSTORE_LOCAL target\nCALL target 1"),
            ] {
                assert_eq!(
                    validate(&authored(
                        &functions,
                        &format!("{input}\n{call}\nPOP\nPUSH_CONST 0\nRET")
                    )),
                    Err(error::syntax)
                );
            }
        }
    });
}

#[test]
fn new_scalars_cannot_implicitly_enter_integer_text_or_aggregate_operations() {
    checked(|| {
        let declarations = "# Struct definitions\nstruct point\nx: i64\nend struct\n# Enum definitions\nenum state\nValue: i64\nend enum\n";
        for prefix in [
            "PUSH_CONST 1\nCAST bool",
            "PUSH_CONST 255\nCAST u8",
            "PUSH_CONST 0.5",
        ] {
            for tail in [
                "PUSH_CONST 1\nADD i64\nPOP",
                "PUSH_CONST 1\nCMP_EQ\nPOP",
                "NOT\nPOP",
                "NEG i64\nPOP",
                "JMP_IF_NOT done\nLABEL done",
                "CAST text\nPOP",
                "CALL println 1",
                "STORE_GLOBAL app::g i64",
                "NEW_ARRAY_LITERAL 1 i64\nPOP",
                "NEW_ARRAY_LITERAL 1 text\nPOP",
                "NEW_VARIANT state::Value 1 state i64\nPOP",
                "STORE_LOCAL item\nNEW_STRUCT point\nLOAD_LOCAL item\nSET_FIELD x\nPOP",
                "STORE_LOCAL index\nPUSH_CONST 7\nNEW_ARRAY_LITERAL 1 i64\nLOAD_LOCAL index\nGET_INDEX\nPOP",
            ] {
                assert_eq!(
                    validate(&authored(
                        declarations,
                        &format!("{prefix}\n{tail}\nPUSH_CONST 0\nRET")
                    )),
                    Err(error::syntax),
                    "{prefix}; {tail}"
                );
            }
        }
        for prefix in ["PUSH_CONST 1\nCAST bool", "PUSH_CONST 255\nCAST u8"] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("{prefix}\nCAST i64\nCAST text\nCALL println 1\nPUSH_CONST 0\nRET")
                )),
                Ok(())
            );
        }
    });
}

#[test]
fn typed_scalar_cfg_joins_and_local_stores_require_identical_kinds() {
    checked(|| {
        for (left, right) in [
            ("PUSH_CONST 1\nCAST bool", "PUSH_CONST 1\nCAST u8"),
            ("PUSH_CONST 1\nCAST bool", "PUSH_CONST 1"),
            ("PUSH_CONST 1.0", "PUSH_CONST 1"),
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!(
                        "PUSH_CONST 0\nJMP_IF_NOT other\n{left}\nJMP merged\nLABEL other\n{right}\nLABEL merged\nPOP\nPUSH_CONST 0\nRET"
                    )
                )),
                Err(error::syntax)
            );
            assert_eq!(
                validate(&authored(
                    "",
                    &format!(
                        "{left}\nSTORE_LOCAL value\n{right}\nSTORE_LOCAL value\nPUSH_CONST 0\nRET"
                    )
                )),
                Err(error::syntax)
            );
        }
        assert_eq!(
            validate(&authored(
                "",
                "PUSH_CONST 0\nJMP_IF_NOT other\nPUSH_CONST -0.0\nJMP merged\nLABEL other\nPUSH_CONST 0.5\nLABEL merged\nCAST f64\nPOP\nPUSH_CONST 0\nRET"
            )),
            Ok(())
        );
        let functions = format!("{}{}", helper("yes", "bool"), helper("byte", "u8"));
        assert_eq!(
            validate(&authored(
                &functions,
                "PUSH_CONST 0\nJMP_IF_NOT other\nPUSH_FN_PTR yes\nJMP merged\nLABEL other\nPUSH_FN_PTR byte\nLABEL merged\nPOP\nPUSH_CONST 0\nRET"
            )),
            Err(error::syntax)
        );
    });
}

#[test]
fn typed_scalar_helpers_obey_arity_bounds_and_are_checked_when_unused() {
    checked(|| {
        let eight = "# declaration_line 1\nfunction eight(a: bool,b: u8,c: f64,d: bool,e: u8,f: f64,g: i32,h: i64) -> void:\nPOP\nPOP\nPOP\nPOP\nPOP\nPOP\nPOP\nPOP\nRET\nend function\n";
        assert_eq!(validate(&authored(eight, "PUSH_CONST 0\nRET")), Ok(()));
        assert_eq!(
            validate(&authored(
                &eight.replace("h: i64)", "h: i64,i: bool)"),
                "PUSH_CONST 0\nRET"
            )),
            Err(error::limit)
        );
        for function in [
            "# declaration_line 1\nfunction bad(value: bool) -> i64:\nRET\nend function\n",
            "# declaration_line 1\nfunction bad -> f64:\nPUSH_CONST 1\nRET\nend function\n",
            "# declaration_line 1\nfunction bad -> i64:\nPUSH_CONST 0\nRET\nPUSH_CONST 1e9999\nend function\n",
        ] {
            assert_eq!(
                validate(&authored(function, "PUSH_CONST 0\nRET")),
                Err(error::syntax)
            );
        }
    });
}

#[test]
fn new_scalar_names_do_not_widen_entrypoint_or_aggregate_declarations() {
    checked(|| {
        for value_type in ["bool", "u8", "f64"] {
            assert_eq!(
                validate(&authored("", "PUSH_CONST 0\nRET").replace(
                    "function main -> i64:",
                    &format!("function main -> {value_type}:")
                )),
                Err(error::intent)
            );
            for prefix in [
                format!("# Struct definitions\nstruct point\nx: {value_type}\nend struct\n"),
                format!("# Enum definitions\nenum state\nValue: {value_type}\nend enum\n"),
                helper("array", &format!("array<{value_type}>")),
            ] {
                assert_eq!(
                    validate(&authored(&prefix, "PUSH_CONST 0\nRET")),
                    Err(error::unsupported)
                );
            }
        }
        for alias in ["boolean", "byte", "float", "double", "f32", "u16"] {
            assert_eq!(
                validate(&authored(&helper("identity", alias), "PUSH_CONST 0\nRET")),
                Err(error::unsupported)
            );
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("PUSH_CONST 1\nCAST {alias}\nPOP\nPUSH_CONST 0\nRET")
                )),
                Err(error::unsupported)
            );
        }
    });
}

#[test]
fn typed_scalar_metadata_is_descriptive_and_retains_exact_instruction_indices() {
    checked(|| {
        let source = authored(
            "",
            "# source_debug 0 \"scalar;雪.ny\" 1 \"value\" \"slot\" \"i64\" 1\nPUSH_CONST -0.0\n# source_debug 1 \"scalar;雪.ny\" 0\nCAST f64\nPOP\nPUSH_CONST 0\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = typed_scalars::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(
            witness.program().instructions()[0].operation,
            operation::float(1u64 << 63)
        );
        assert_eq!(
            witness
                .program()
                .source_debug(0)
                .unwrap()
                .bindings()
                .next()
                .unwrap()
                .lexical_type
                .bytes()
                .collect::<Vec<_>>(),
            b"i64"
        );
        assert_eq!(
            witness
                .program()
                .source_debug(1)
                .unwrap()
                .instruction_index(),
            1
        );
        assert_eq!(
            validate(&source.replace("CAST f64", "CAST i64")),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&source.replace("# source_debug 1", "# source_debug 2")),
            Err(error::syntax)
        );
    });
}

#[test]
fn long_zero_exponents_respect_the_existing_complete_edition_capacity() {
    checked(|| {
        let base = authored("", "PUSH_CONST -0e1\nPOP\nPUSH_CONST 0\nRET");
        let exponent = "9".repeat(maximum_edition_bytes - base.len() + 1);
        let source = base.replace("-0e1", &format!("-0e{exponent}"));
        assert_eq!(source.len(), maximum_edition_bytes);
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = typed_scalars::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(
            witness.program().instructions()[0].operation,
            operation::float(1u64 << 63)
        );
        assert_eq!(validate(&(source + "\n")), Err(error::edition));
    });
}

#[test]
fn typed_scalar_validation_retains_dead_code_authority_and_atomic_encoding() {
    checked(|| {
        let source = authored(
            "",
            "PUSH_CONST 0.5\nPOP\nPUSH_CONST 0\nRET\nPUSH_CONST 7\nCALL println 1",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        assert_eq!(
            typed_scalars::admit(&image, 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        let witness =
            typed_scalars::admit(&image, authority::console_write, &mut workspace).unwrap();
        assert_eq!(witness.required_services(), authority::console_write);
        assert_eq!(witness.granted_services(), authority::console_write);
        let mut short = [0xa5; 8];
        assert_eq!(
            typed_scalars::encode(source.as_bytes(), &mut short, &mut workspace),
            Err(error::output)
        );
        assert_eq!(short, [0xa5; 8]);
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn typed_scalar_raw_conformance_relaxes_only_absent_main_intent() {
    checked(|| {
        let source = authored("", "PUSH_CONST 0.5\nPOP\nPUSH_CONST 0\nRET")
            .replace("@ai_context(\"Exercise explicit typed scalars\")\n", "");
        assert_eq!(validate(&source), Err(error::intent));
        let mut workspace = validation_workspace::new();
        let raw =
            conformance::validate_typed_scalars(source.as_bytes(), 0, &mut workspace).unwrap();
        assert_eq!(raw.profile(), 11);
        assert_eq!(raw.intent(), None);
        assert_eq!(raw.edition(), source.as_bytes());
        assert_eq!(raw.identity(), sha256(source.as_bytes()));
        assert_eq!(
            raw.program().instructions()[0].operation,
            operation::float(0x3fe0_0000_0000_0000)
        );
        for (changed, expected) in [
            (
                source.replace(
                    "PUSH_CONST 0.5\nPOP",
                    "PUSH_CONST 1\nCAST bool\nCALL println 1",
                ),
                error::syntax,
            ),
            (
                source.replace("PUSH_CONST 0.5\nPOP", "PUSH_CONST 1\nCALL println 1"),
                error::ceiling,
            ),
            (
                source.replace(
                    "PUSH_CONST 0.5",
                    "# source_debug 1 \"raw.ny\" 0\nPUSH_CONST 0.5",
                ),
                error::syntax,
            ),
        ] {
            let mut workspace = validation_workspace::new();
            assert_eq!(
                conformance::validate_typed_scalars(changed.as_bytes(), 0, &mut workspace).err(),
                Some(expected)
            );
        }
    });
}
