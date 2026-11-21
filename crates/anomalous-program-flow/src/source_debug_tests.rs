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

fn edition(prefix: &str, annotation: &str, body: &str) -> String {
    format!(
        "# NIL Textual Module\nmodule root\n{prefix}{annotation}# declaration_line 1\nfunction main -> i64:\n{body}\nend function\n"
    )
}

fn authored(prefix: &str, body: &str) -> String {
    edition(prefix, "@ai_context(\"Inspect source metadata\")\n", body)
}

fn package(source: &str) -> Vec<u8> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0; maximum_image_bytes];
    let length = source_debug::encode(source.as_bytes(), &mut output, &mut workspace).unwrap();
    output.truncate(length);
    output
}

fn validate(source: &str) -> Result<(), error> {
    let mut workspace = validation_workspace::new();
    let mut output = vec![0xa5; maximum_image_bytes];
    let result = source_debug::encode(source.as_bytes(), &mut output, &mut workspace);
    if result.is_err() {
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
    result.map(|_| ())
}

fn decoded(value: source_debug::quoted_text<'_>) -> String {
    String::from_utf8(value.bytes().collect()).unwrap()
}

#[test]
fn source_debug_profile_changes_only_both_tags_for_previous_syntax() {
    checked(|| {
        let source = authored(
            "# Enum definitions\nenum state\nIdle\nend enum\n",
            "NEW_VARIANT state::Idle 0 state void\nIS_VARIANT state::Idle state void\nSTORE_GLOBAL app::result i64\nLOAD_GLOBAL app::result i64\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let mut prior = vec![0; maximum_image_bytes];
        let length = enums::encode(source.as_bytes(), &mut prior, &mut workspace).unwrap();
        prior.truncate(length);
        assert_eq!(source_debug::derivation_profile, 10);
        assert_eq!(get32(&image, 12), 10);
        assert_eq!(get32(&image, image_header_bytes + 12), 10);
        put32(&mut prior, 12, 10);
        put32(&mut prior, image_header_bytes + 12, 10);
        assert_eq!(image, prior);
        let witness = source_debug::admit(&image, 0, &mut workspace).unwrap();
        assert_eq!(witness.profile(), 10);
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
        assert_eq!(witness.edition(), source.as_bytes());
        assert_eq!(witness.required_services(), 0);
        assert!(witness.program().source_debug(0).is_none());
    });
}

#[test]
fn source_debug_headers_and_all_previous_profiles_remain_isolated() {
    checked(|| {
        let source = authored("", "# source_debug 0 \"sample.ny\" 0\nPUSH_CONST 0\nRET");
        let image = package(&source);
        for outer in [0, 1, 2, 8, 9, 10, 11, u32::MAX] {
            for inner in [0, 1, 9, 10, 11, u32::MAX] {
                let mut changed = image.clone();
                put32(&mut changed, 12, outer);
                put32(&mut changed, image_header_bytes + 12, inner);
                let mut workspace = validation_workspace::new();
                assert_eq!(
                    source_debug::admit(&changed, 0, &mut workspace).err(),
                    if outer == 10 && inner == 10 {
                        None
                    } else {
                        Some(error::profile)
                    },
                );
            }
        }
        let mut output = vec![0xa5; maximum_image_bytes];
        assert_eq!(
            anomalous_program_manifest::encode(source.as_bytes(), &mut output),
            Err(error::unsupported)
        );
        for profile in 2..=9 {
            let mut workspace = validation_workspace::new();
            let result = match profile {
                2 => encode(source.as_bytes(), &mut output, &mut workspace),
                3 => scalar::encode(source.as_bytes(), &mut output, &mut workspace),
                4 => text::encode(source.as_bytes(), &mut output, &mut workspace),
                5 => array::encode(source.as_bytes(), &mut output, &mut workspace),
                6 => callable::encode(source.as_bytes(), &mut output, &mut workspace),
                7 => record::encode(source.as_bytes(), &mut output, &mut workspace),
                8 => globals::encode(source.as_bytes(), &mut output, &mut workspace),
                _ => enums::encode(source.as_bytes(), &mut output, &mut workspace),
            };
            assert_eq!(result, Err(error::unsupported));
            assert!(output.iter().all(|byte| *byte == 0xa5));
        }
        let mut changed = image;
        put32(&mut changed, 12, 9);
        put32(&mut changed, image_header_bytes + 12, 9);
        let mut workspace = validation_workspace::new();
        assert_eq!(
            enums::admit(&changed, 0, &mut workspace).err(),
            Some(error::unsupported)
        );
    });
}

#[test]
fn source_debug_views_preserve_decoded_utf8_escapes_and_edition_identity() {
    checked(|| {
        let source = authored(
            "",
            "# source_debug 000 \"folder; \\\"quoted\\\"\\\\雪.ny\" 01 \"café\" \"$ny.local.0.x\" \"arbitrary<type>\" 01024\n\nPUSH_CONST 0\n# source_debug 1 \"other.ny\" 0\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = source_debug::admit(&image, 0, &mut workspace).unwrap();
        let plan = witness.program();
        assert_eq!(plan.instructions().len(), 2);
        let debug = plan.source_debug(0).unwrap();
        assert_eq!(debug.instruction_index(), 0);
        assert_eq!(debug.binding_count(), 1);
        assert_eq!(decoded(debug.path()), "folder; \"quoted\"\\雪.ny");
        assert_eq!(debug.path().escaped(), "folder; \\\"quoted\\\"\\\\雪.ny");
        let mut locals = debug.bindings();
        assert_eq!(locals.len(), 1);
        let local = locals.next().unwrap();
        assert_eq!(decoded(local.name), "café");
        assert_eq!(decoded(local.storage), "$ny.local.0.x");
        assert_eq!(decoded(local.lexical_type), "arbitrary<type>");
        assert_eq!(local.scope_depth, 1024);
        assert_eq!(locals.len(), 0);
        assert!(locals.next().is_none());
        assert!(locals.next().is_none());
        let mut short = [0xa5; 2];
        assert_eq!(local.name.copy_to(&mut short), Err(error::output));
        assert_eq!(short, [0xa5; 2]);
        let mut destination = [0xa5; 16];
        let length = local.name.copy_to(&mut destination).unwrap();
        assert_eq!(length, local.name.decoded_len());
        assert_eq!(&destination[..length], "café".as_bytes());
        assert!(destination[length..].iter().all(|byte| *byte == 0xa5));
        assert_eq!(plan.source_debug(1).unwrap().binding_count(), 0);
        assert!(plan.source_debug(2).is_none());
        assert!(plan.source_debug(usize::MAX).is_none());
        assert_eq!(witness.identity(), sha256(source.as_bytes()));
    });
}

#[test]
fn source_debug_indices_reset_per_function_and_count_labels() {
    checked(|| {
        let prefix = "# declaration_line 1\nfunction helper -> i64:\n# source_debug 0 \"helper.ny\" 0\nPUSH_CONST 7\n# source_debug 1 \"helper.ny\" 0\nRET\nend function\n";
        let source = authored(
            prefix,
            "#source_debug 0 \"main.ny\" 0\nLABEL entry\n#\tsource_debug\t1\t\"main.ny\"\t0\nCALL helper 0\n# source_debug 2 \"main.ny\" 0\nRET",
        );
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        let witness = source_debug::admit(&image, 0, &mut workspace).unwrap();
        let plan = witness.program();
        assert_eq!(plan.functions()[1].start, 2);
        assert_eq!(plan.instructions().len(), 5);
        assert_eq!(plan.instructions()[2].operation, operation::label);
        for (global, local) in [(0, 0), (1, 1), (2, 0), (3, 1), (4, 2)] {
            assert_eq!(
                plan.source_debug(global).unwrap().instruction_index(),
                local
            );
        }
        assert_eq!(decoded(plan.source_debug(2).unwrap().path()), "main.ny");
        assert!(plan.instructions()[2].line > plan.instructions()[1].line);
        assert_eq!(
            validate(&source.replace("source_debug 2 \"main", "source_debug 1 \"main")),
            Err(error::syntax)
        );
    });
}

#[test]
fn source_debug_rejects_malformed_fields_and_delimiters() {
    checked(|| {
        for record in [
            "# source_debug",
            "# source_debug 0",
            "# source_debug 0 \"path\"",
            "# source_debug 0 path 0",
            "# source_debug \"0\" \"path\" 0",
            "# source_debug 0 \"path\" \"0\"",
            "# source_debug +0 \"path\" 0",
            "# source_debug -0 \"path\" 0",
            "# source_debug 0x0 \"path\" 0",
            "# source_debug ０ \"path\" 0",
            "# source_debug 184467440737095516160 \"path\" 0",
            "# source_debug 1 \"path\" 0",
            "# source_debug 0 \"\" 0",
            "# source_debug 0 \"path\" 0 extra",
            "# source_debug 0 \"path\" 0 ;",
            "# source_debug 0 \"path\" 0 ; line 1",
            "# source_debug 0, \"path\" 0",
            "# source_debug 0 \"path\", 0",
            "# source_debug 0 \"path\"0",
            "# source_debug 0 \"path 0",
            "# source_debug 0 \"path\" 1",
            "# source_debug 0 \"path\" 1 name \"slot\" \"i64\" 0",
            "# source_debug 0 \"path\" 1 \"name\" slot \"i64\" 0",
            "# source_debug 0 \"path\" 1 \"name\" \"slot\" i64 0",
            "# source_debug 0 \"path\" 1 \"name\" \"slot\" \"i64\" \"0\"",
            "# source_debug 0 \"path\" 1 \"name\" \"slot\" \"i64\" -1",
            "# source_debug 0 \"path\" 1 \"\" \"slot\" \"i64\" 0",
            "# source_debug 0 \"path\" 1 \"name\" \"\" \"i64\" 0",
            "# source_debug 0 \"path\" 1 \"name\" \"slot\" \"\" 0",
            "# source_debug 0 \"path\" 0 \"unused\"",
        ] {
            assert_eq!(
                validate(&authored("", &format!("{record}\nPUSH_CONST 0\nRET"))),
                Err(error::syntax),
                "{record}"
            );
        }
    });
}

#[test]
fn source_debug_control_and_escape_policy_is_explicit() {
    checked(|| {
        for escaped in [
            "a\\n", "a\\r", "a\\t", "a\\v", "a\\f", "a\\0", "a\tb", "a\u{7f}b", "a\u{85}b",
        ] {
            let source = authored(
                "",
                &format!("# source_debug 0 \"{escaped}\" 0\nPUSH_CONST 0\nRET"),
            );
            assert_eq!(validate(&source), Err(error::syntax), "{escaped:?}");
        }
        for escaped in ["a\\x20", "a\\u0020", "a\\q", "a\\/", "a\\雪"] {
            let source = authored(
                "",
                &format!("# source_debug 0 \"{escaped}\" 0\nPUSH_CONST 0\nRET"),
            );
            assert_eq!(validate(&source), Err(error::unsupported), "{escaped:?}");
        }
        let valid = authored("", "# source_debug 0 \"x\" 0\nPUSH_CONST 0\nRET");
        let mut invalid = valid.into_bytes();
        let index = invalid.iter().position(|byte| *byte == b'x').unwrap();
        invalid[index] = 0xff;
        let mut output = vec![0xa5; maximum_image_bytes];
        let mut workspace = validation_workspace::new();
        assert_eq!(
            source_debug::encode(&invalid, &mut output, &mut workspace),
            Err(error::edition)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
    });
}

#[test]
fn source_debug_rejects_duplicate_lexical_names_or_storage() {
    checked(|| {
        for bindings in [
            "\"same\" \"a\" \"i64\" 0 \"same\" \"b\" \"text\" 1",
            "\"first\" \"same\" \"i64\" 0 \"second\" \"same\" \"text\" 1",
            "\"a\\\\b\" \"one\" \"i64\" 0 \"a\\\\b\" \"two\" \"i64\" 0",
        ] {
            let source = authored(
                "",
                &format!("# source_debug 0 \"path\" 2 {bindings}\nPUSH_CONST 0\nRET"),
            );
            assert_eq!(validate(&source), Err(error::syntax));
        }
        // Name and storage namespaces are independently unique, not disjoint.
        let source = authored(
            "",
            "# source_debug 0 \"path\" 2 \"a\" \"b\" \"i64\" 0 \"b\" \"a\" \"text\" 1\nPUSH_CONST 0\nRET",
        );
        assert_eq!(validate(&source), Ok(()));
    });
}

#[test]
fn source_debug_bounds_count_decoded_utf8_bytes_and_all_bindings() {
    checked(|| {
        let max_path = "\\\\".repeat(source_debug::maximum_path_bytes);
        assert_eq!(
            validate(&authored(
                "",
                &format!("# source_debug 0 \"{max_path}\" 0\nPUSH_CONST 0\nRET")
            )),
            Ok(())
        );
        assert_eq!(
            validate(&authored(
                "",
                &format!("# source_debug 0 \"{max_path}x\" 0\nPUSH_CONST 0\nRET")
            )),
            Err(error::limit)
        );
        let maximum = "é".repeat(64);
        for strings in [
            format!("\"{maximum}\" \"s\" \"t\""),
            format!("\"n\" \"{maximum}\" \"t\""),
            format!("\"n\" \"s\" \"{maximum}\""),
        ] {
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("# source_debug 0 \"path\" 1 {strings} 1024\nPUSH_CONST 0\nRET")
                )),
                Ok(())
            );
            let too_long = strings.replace(&maximum, &format!("{maximum}x"));
            assert_eq!(
                validate(&authored(
                    "",
                    &format!("# source_debug 0 \"path\" 1 {too_long} 1024\nPUSH_CONST 0\nRET")
                )),
                Err(error::limit)
            );
        }
        let mut bindings = String::new();
        for index in 0..source_debug::maximum_bindings {
            bindings.push_str(&format!(
                " \"name_{index}\" \"slot_{index}\" \"type\" {index}"
            ));
        }
        assert_eq!(
            validate(&authored(
                "",
                &format!("# source_debug 0 \"path\" 128{bindings}\nPUSH_CONST 0\nRET")
            )),
            Ok(())
        );
        assert_eq!(
            validate(&authored(
                "",
                &format!(
                    "# source_debug 0 \"path\" 129{bindings} \"last\" \"last\" \"i64\" 0\nPUSH_CONST 0\nRET"
                )
            )),
            Err(error::limit)
        );
        assert_eq!(
            validate(&authored(
                "",
                "# source_debug 0 \"path\" 1 \"n\" \"s\" \"t\" 1025\nPUSH_CONST 0\nRET"
            )),
            Err(error::limit)
        );
    });
}

#[test]
fn source_debug_rejects_orphans_misplacement_and_arbitrary_comments() {
    checked(|| {
        for body in [
            "# source_debug 0 \"path\" 0\n# source_debug 0 \"other\" 0\nPUSH_CONST 0\nRET",
            "PUSH_CONST 0\nRET\n# source_debug 2 \"path\" 0",
        ] {
            assert_eq!(validate(&authored("", body)), Err(error::syntax));
        }
        for prefix in [
            "# source_debug 0 \"path\" 0\n",
            "# Struct definitions\nstruct value\n# source_debug 0 \"path\" 0\nitem: i64\nend struct\n",
            "# Enum definitions\nenum state\n# source_debug 0 \"path\" 0\nIdle\nend enum\n",
        ] {
            assert_eq!(
                validate(&authored(prefix, "PUSH_CONST 0\nRET")),
                Err(error::syntax)
            );
        }
        for record in [
            "# unknown comment",
            "# source_debug_extra 0 \"path\" 0",
            "# SOURCE_DEBUG 0 \"path\" 0",
        ] {
            assert_eq!(
                validate(&authored("", &format!("{record}\nPUSH_CONST 0\nRET"))),
                Err(error::unsupported)
            );
        }
        let pending_at_eof = authored("", "PUSH_CONST 0\nRET\n# source_debug 2 \"path\" 0");
        assert_eq!(
            validate(pending_at_eof.strip_suffix("end function\n").unwrap()),
            Err(error::syntax)
        );
    });
}

#[test]
fn source_debug_validates_dead_and_unused_records_without_type_or_authority_effects() {
    checked(|| {
        let helper = "# declaration_line 1\nfunction unused -> i64:\n# source_debug 0 \"/host/private\" 1 \"missing\" \"not_a_local\" \"arbitrary<unsafe>\" 9\nPUSH_CONST 1\nRET\nend function\n";
        let body = "PUSH_CONST 0\nRET\n# source_debug 2 \"unopened\" 1 \"println\" \"print\" \"text\" 0\nPUSH_CONST 1\nCALL println 1";
        let source = authored(helper, body);
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        assert_eq!(
            source_debug::admit(&image, 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        let witness = source_debug::admit(&image, authority::known, &mut workspace).unwrap();
        assert_eq!(witness.required_services(), authority::console_write);
        assert!(witness.program().source_debug(4).is_some());
        assert_eq!(
            validate(&source.replace("\"arbitrary<unsafe>\" 9", "\"\" 9")),
            Err(error::syntax)
        );
        assert_eq!(
            validate(&source.replace("source_debug 2 \"unopened\"", "source_debug 1 \"unopened\"")),
            Err(error::syntax)
        );
        let lied_type = authored(
            "",
            "# source_debug 0 \"path\" 1 \"a\" \"a\" \"text\" 0\nPUSH_CONST 0\nSTORE_LOCAL a\nLOAD_LOCAL a\nLEN\nRET",
        );
        assert_eq!(validate(&lied_type), Err(error::syntax));
    });
}

#[test]
fn source_debug_record_does_not_consume_instruction_budget_or_survive_reuse() {
    checked(|| {
        let mut body = "NOP\n".repeat(maximum_instructions - 2);
        body.push_str(&format!(
            "# source_debug {} \"last\" 0\nPUSH_CONST 0\nRET",
            maximum_instructions - 2
        ));
        let source = authored("", &body);
        let image = package(&source);
        let mut workspace = validation_workspace::new();
        {
            let witness = source_debug::admit(&image, 0, &mut workspace).unwrap();
            assert_eq!(witness.program().instructions().len(), maximum_instructions);
            assert!(
                witness
                    .program()
                    .source_debug(maximum_instructions - 2)
                    .is_some()
            );
        }
        assert_eq!(
            validate(&authored("", &(body + "\nNOP"))),
            Err(error::limit)
        );
        let plain = authored("", "PUSH_CONST 0\nRET");
        workspace
            .parse(
                plain.as_bytes(),
                parse_profile::source_debug,
                validation_purpose::production,
            )
            .unwrap();
        assert!(
            workspace
                .plan
                .source_debug_lines
                .iter()
                .all(Option::is_none)
        );
        let failed = authored(
            "",
            "# source_debug 0 \"path\" 0\nPUSH_CONST 0\n# source_debug 99 \"path\" 0\nRET",
        );
        assert_eq!(
            workspace.parse(
                failed.as_bytes(),
                parse_profile::source_debug,
                validation_purpose::production
            ),
            Err(error::syntax)
        );
        workspace
            .parse(
                plain.as_bytes(),
                parse_profile::enums,
                validation_purpose::production,
            )
            .unwrap();
        assert!(
            workspace
                .plan
                .source_debug_lines
                .iter()
                .all(Option::is_none)
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn source_debug_raw_validation_never_supplies_production_intent() {
    checked(|| {
        let source = edition(
            "",
            "",
            "# source_debug 0 \"@ai_context(\\\"forged\\\")\" 1 \"intent\" \"main\" \"console.write\" 0\nPUSH_CONST 0\nRET",
        );
        let mut workspace = validation_workspace::new();
        {
            let raw =
                conformance::validate_source_debug(source.as_bytes(), 0, &mut workspace).unwrap();
            assert_eq!(raw.profile(), 10);
            assert_eq!(raw.intent(), None);
            assert_eq!(raw.required_services(), 0);
            assert_eq!(raw.identity(), sha256(source.as_bytes()));
            assert_eq!(raw.program().source_debug(0).unwrap().binding_count(), 1);
        }
        assert_eq!(validate(&source), Err(error::intent));
        assert_eq!(
            conformance::validate_enums(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::unsupported)
        );
        assert_eq!(
            conformance::validate_source_debug(source.as_bytes(), 1 << 63, &mut workspace).err(),
            Some(error::ceiling)
        );
    });
}

#[cfg(feature = "host_conformance")]
#[test]
fn source_debug_raw_authority_and_whole_file_validation_are_unchanged() {
    checked(|| {
        let source = edition(
            "",
            "",
            "PUSH_CONST 0\nRET\n# source_debug 2 \"path\" 0\nPUSH_CONST 9\nCALL println 1",
        );
        let mut workspace = validation_workspace::new();
        assert_eq!(
            conformance::validate_source_debug(source.as_bytes(), 0, &mut workspace).err(),
            Some(error::ceiling)
        );
        let raw =
            conformance::validate_source_debug(source.as_bytes(), authority::known, &mut workspace)
                .unwrap();
        assert_eq!(raw.required_services(), authority::console_write);
        let malformed = source.replace("source_debug 2", "source_debug 99");
        assert_eq!(
            conformance::validate_source_debug(
                malformed.as_bytes(),
                authority::known,
                &mut workspace
            )
            .err(),
            Some(error::syntax)
        );
    });
}
