extern crate std;

use super::*;
use crate::{maximum_edition_bytes, maximum_image_bytes, maximum_intent_bytes, scalar};
use std::{format, string::String, vec, vec::Vec};

fn checked(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn edition(annotation: &str, helpers: &str, main: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n# String constants\nconst S = \"text\"\n{helpers}{annotation}# declaration_line 1\nfunction {main}:\n{body}\nend function\n"
    )
}

fn raw(body: &str) -> String {
    edition("", "", "main -> i64", body)
}

fn validate(source: &str, ceiling: u64) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    validate_scalar(source.as_bytes(), ceiling, &mut workspace).map(|_| ())
}

#[test]
fn raw_witness_binds_exact_bytes_without_inventing_intent_or_authority() {
    checked(|| {
        let source = raw("PUSH_CONST 42\nRET");
        let mut workspace = validation_workspace::new();
        let witness =
            validate_scalar(source.as_bytes(), authority::console_write, &mut workspace).unwrap();
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.edition().as_ptr(), source.as_bytes().as_ptr());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.intent(), None);
        assert_eq!(witness.required_services(), 0);
        assert_eq!(witness.granted_services(), 0);
        assert_eq!(
            witness.program().functions()[witness.program().main()].name,
            "main"
        );
    });
}

#[test]
fn production_packaging_still_requires_authored_main_intent() {
    checked(|| {
        let source = raw("PUSH_CONST 42\nRET");
        let mut workspace = validation_workspace::new();
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(validate(&source, 0), Ok(()));
        assert_eq!(
            scalar::encode(source.as_bytes(), &mut output, &mut workspace),
            Err(error::intent)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        assert_eq!(
            crate::encode(source.as_bytes(), &mut output, &mut workspace),
            Err(error::intent)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
        assert_eq!(
            anomalous_program_manifest::encode(source.as_bytes(), &mut output),
            Err(error::intent)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
    });
}

#[test]
fn authored_annotation_remains_exact_and_optional_only_in_conformance() {
    checked(|| {
        let intent = "Describe this test without granting effects";
        let source = edition(
            &format!("@ai_context(\"{intent}\")\n"),
            "",
            "main -> i32",
            "PUSH_CONST 0\nRET",
        );
        let mut workspace = validation_workspace::new();
        let witness = validate_scalar(source.as_bytes(), 0, &mut workspace).unwrap();
        assert_eq!(witness.intent(), Some(intent));
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        let mut output = vec![0; maximum_image_bytes];
        assert!(scalar::encode(source.as_bytes(), &mut output, &mut workspace).is_ok());
    });
}

#[test]
fn present_annotations_keep_their_strict_grammar_and_size() {
    checked(|| {
        for (annotation, expected) in [
            ("@ai_context(\"\")\n", error::intent),
            ("@ai_context(\"a\\nb\")\n", error::intent),
            ("@ai_context(\"a\"b\")\n", error::intent),
            ("@ai_context(\"a\tb\")\n", error::intent),
            ("@ai_context(text=\"a\")\n", error::unsupported),
            ("@other(\"a\")\n", error::unsupported),
            (
                "@ai_context(\"one\")\n@ai_context(\"two\")\n",
                error::intent,
            ),
        ] {
            assert_eq!(
                validate(
                    &edition(annotation, "", "main -> i64", "PUSH_CONST 0\nRET"),
                    0
                ),
                Err(expected)
            );
        }
        for length in [maximum_intent_bytes, maximum_intent_bytes + 1] {
            let source = edition(
                &format!("@ai_context(\"{}\")\n", "x".repeat(length)),
                "",
                "main -> i64",
                "PUSH_CONST 0\nRET",
            );
            assert_eq!(
                validate(&source, 0),
                if length == maximum_intent_bytes {
                    Ok(())
                } else {
                    Err(error::limit)
                }
            );
        }
    });
}

#[test]
fn optional_intent_does_not_permit_annotations_on_helpers_or_elsewhere() {
    checked(|| {
        let helper = "@ai_context(\"helper metadata\")\n# declaration_line 1\nfunction helper -> i64:\nPUSH_CONST 1\nRET\nend function\n";
        assert_eq!(
            validate(&edition("", helper, "main -> i64", "PUSH_CONST 0\nRET"), 0),
            Err(error::intent)
        );
        assert_eq!(
            validate(&raw("@ai_context(\"inside body\")\nPUSH_CONST 0\nRET"), 0),
            Err(error::unsupported)
        );
        let dangling = format!("{}@ai_context(\"dangling\")\n", raw("PUSH_CONST 0\nRET"));
        assert_eq!(validate(&dangling, 0), Err(error::syntax));
    });
}

#[test]
fn entry_requirements_are_unchanged_when_intent_is_absent() {
    checked(|| {
        for main in [
            "helper -> i64",
            "main(value: i64) -> i64",
            "main -> text",
            "main -> void",
        ] {
            assert_eq!(
                validate(&edition("", "", main, "PUSH_CONST 0\nRET"), 0),
                Err(error::intent)
            );
        }
        let duplicate =
            "# declaration_line 1\nfunction main -> i64:\nPUSH_CONST 0\nRET\nend function\n";
        assert_eq!(
            validate(
                &edition("", duplicate, "main -> i64", "PUSH_CONST 0\nRET"),
                0
            ),
            Err(error::syntax)
        );
    });
}

#[test]
fn scalar_normalization_and_flow_checks_are_shared() {
    checked(|| {
        let source = raw(
            "PUSH_CONST 40\nSTORE_LOCAL $ny.temp.idx.0\nLOAD_LOCAL $ny.temp.idx.0\nPUSH_CONST 2\nADD\nRET",
        );
        let mut workspace = validation_workspace::new();
        let witness = validate_scalar(source.as_bytes(), 0, &mut workspace).unwrap();
        assert_eq!(
            witness.program().instructions()[4].operation,
            crate::operation::binary {
                opcode: "ADD",
                width: "i64"
            }
        );
        for (body, expected) in [
            ("PUSH_CONST 0\nPUSH_CONST_STR S\nADD\nRET", error::syntax),
            ("LOAD_LOCAL value\nRET", error::syntax),
            (
                "PUSH_CONST 0\nJMP_IF_NOT skip\nPUSH_CONST 1\nSTORE_LOCAL value\nLABEL skip\nLOAD_LOCAL value\nRET",
                error::syntax,
            ),
            ("PUSH_CONST 1\nPUSH_CONST 2\nSUB\nRET", error::unsupported),
            (
                "PUSH_CONST 1\nSTORE_LOCAL $ny.temp.query.source.0\nPUSH_CONST 0\nRET",
                error::syntax,
            ),
        ] {
            assert_eq!(validate(&raw(body), 0), Err(expected), "{body}");
        }
    });
}

#[test]
fn dead_and_unused_effects_require_the_same_explicit_ceiling() {
    checked(|| {
        let unused = "# declaration_line 1\nfunction unused -> i64:\nPUSH_CONST_STR S\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET\nend function\n";
        for source in [
            raw("PUSH_CONST 0\nRET\nPUSH_CONST_STR S\nCONSOLE_WRITELN"),
            edition("", unused, "main -> i64", "PUSH_CONST 0\nRET"),
        ] {
            assert_eq!(validate(&source, 0), Err(error::ceiling));
            let mut workspace = validation_workspace::new();
            let witness =
                validate_scalar(source.as_bytes(), authority::console_write, &mut workspace)
                    .unwrap();
            assert_eq!(witness.required_services(), authority::console_write);
            assert_eq!(witness.granted_services(), authority::console_write);
            assert_eq!(witness.intent(), None);
        }
        assert_eq!(validate(&raw("PUSH_CONST 0\nRET"), 2), Err(error::ceiling));
    });
}

#[test]
fn unreachable_unsupported_and_unresolved_instructions_are_not_ignored() {
    checked(|| {
        for (body, expected) in [
            (
                "PUSH_CONST 0\nRET\nINTEROP_CALL libc open 0 i64",
                error::unsupported,
            ),
            ("PUSH_CONST 0\nRET\nCALL missing 0", error::unsupported),
            ("PUSH_CONST 0\nRET\nJMP missing", error::syntax),
        ] {
            assert_eq!(
                validate(&raw(body), authority::console_write),
                Err(expected)
            );
        }
    });
}

#[test]
fn raw_input_limits_and_envelope_separation_remain_explicit() {
    checked(|| {
        let source = raw("PUSH_CONST 0\nRET");
        let mut workspace = validation_workspace::new();
        assert_eq!(
            validate_scalar(&[], 0, &mut workspace).err(),
            Some(error::edition)
        );
        let oversized = format!("{source}{}", "\n".repeat(maximum_edition_bytes));
        assert_eq!(validate(&oversized, 0), Err(error::edition));
        assert_eq!(
            validate_scalar(&[0xff], 0, &mut workspace).err(),
            Some(error::edition)
        );
        let authored = edition(
            "@ai_context(\"Production package\")\n",
            "",
            "main -> i64",
            "PUSH_CONST 0\nRET",
        );
        let mut output = vec![0; maximum_image_bytes];
        let length = scalar::encode(authored.as_bytes(), &mut output, &mut workspace).unwrap();
        assert!(validate_scalar(&output[..length], 0, &mut workspace).is_err());
        assert!(scalar::admit(source.as_bytes(), 0, &mut workspace).is_err());
    });
}

#[test]
fn workspace_reuse_cannot_carry_conformance_policy_or_effects_into_production() {
    checked(|| {
        let unannotated = raw("PUSH_CONST 0\nRET");
        let console = raw("PUSH_CONST_STR S\nCONSOLE_WRITELN\nPUSH_CONST 0\nRET");
        let authored = edition(
            "@ai_context(\"Production package\")\n",
            "",
            "main -> i64",
            "PUSH_CONST 0\nRET",
        );
        let mut workspace = validation_workspace::new();
        let mut output = vec![0; maximum_image_bytes];
        assert_eq!(
            validate_scalar(console.as_bytes(), 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        assert_eq!(
            validate_scalar(unannotated.as_bytes(), 0, &mut workspace)
                .unwrap()
                .required_services(),
            0
        );
        assert_eq!(
            scalar::encode(unannotated.as_bytes(), &mut output, &mut workspace),
            Err(error::intent)
        );
        let length = scalar::encode(authored.as_bytes(), &mut output, &mut workspace).unwrap();
        assert_eq!(
            scalar::admit(&output[..length], 0, &mut workspace)
                .unwrap()
                .required_services(),
            0
        );
        assert_eq!(
            validate_scalar(unannotated.as_bytes(), 0, &mut workspace)
                .unwrap()
                .intent(),
            None
        );
        let mut final_output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            crate::encode(unannotated.as_bytes(), &mut final_output, &mut workspace),
            Err(error::intent)
        );
        assert!(final_output.iter().all(|byte| *byte == 0xa5));
    });
}

#[test]
fn complete_compiler_artifacts_have_identical_plans_under_both_paths() {
    checked(|| {
        let fixtures = [
            include_bytes!(
                "../../../anomalous-program-executor/fixtures/scalar-flow/bounds_once.nil"
            )
            .as_slice(),
            include_bytes!(
                "../../../anomalous-program-executor/fixtures/scalar-flow/break_continue.nil"
            )
            .as_slice(),
            include_bytes!("../../../anomalous-program-executor/fixtures/scalar-flow/fuel.nil")
                .as_slice(),
            include_bytes!(
                "../../../anomalous-program-executor/fixtures/scalar-flow/nested_ranges.nil"
            )
            .as_slice(),
            include_bytes!(
                "../../../anomalous-program-executor/fixtures/scalar-flow/range_widths.nil"
            )
            .as_slice(),
            include_bytes!(
                "../../../anomalous-program-executor/fixtures/scalar-flow/scratch_joins.nil"
            )
            .as_slice(),
            include_bytes!(
                "../../../anomalous-program-executor/fixtures/scalar-flow/short_circuit.nil"
            )
            .as_slice(),
            include_bytes!(
                "../../../anomalous-program-executor/fixtures/scalar-flow/zero_ranges.nil"
            )
            .as_slice(),
        ];
        for source in fixtures {
            let mut workspace = validation_workspace::new();
            let witness =
                validate_scalar(source, authority::console_write, &mut workspace).unwrap();
            let raw_operations: Vec<_> = witness
                .program()
                .instructions()
                .iter()
                .map(|item| (item.operation, item.line))
                .collect();
            let raw_functions: Vec<_> = witness
                .program()
                .functions()
                .iter()
                .map(|item| {
                    (
                        item.name,
                        item.start,
                        item.end,
                        item.return_kind,
                        item.local_count,
                        item.parameter_kinds().to_vec(),
                    )
                })
                .collect();
            let intent = witness.intent();
            let required = witness.required_services();
            let identity = witness.identity();
            let mut output = vec![0; maximum_image_bytes];
            let length = scalar::encode(source, &mut output, &mut workspace).unwrap();
            let admitted =
                scalar::admit(&output[..length], authority::console_write, &mut workspace).unwrap();
            assert_eq!(admitted.edition(), source);
            assert_eq!(Some(admitted.intent()), intent);
            assert_eq!(admitted.identity(), identity);
            assert_eq!(admitted.required_services(), required);
            assert_eq!(
                admitted
                    .program()
                    .instructions()
                    .iter()
                    .map(|item| (item.operation, item.line))
                    .collect::<Vec<_>>(),
                raw_operations
            );
            assert_eq!(
                admitted
                    .program()
                    .functions()
                    .iter()
                    .map(|item| (
                        item.name,
                        item.start,
                        item.end,
                        item.return_kind,
                        item.local_count,
                        item.parameter_kinds().to_vec()
                    ))
                    .collect::<Vec<_>>(),
                raw_functions
            );
        }
    });
}
