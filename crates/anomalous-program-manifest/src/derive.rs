//! Fully consumed, deliberately small textual NIL admission profile.
//!
//! This is not a general NIL loader or an interpreter. Every accepted body is
//! inspected, including unused functions. Unsupported syntax cannot become an
//! authority-free instruction merely because it is unreachable.

use crate::{authority, error, maximum_edition_bytes, maximum_intent_bytes};

pub const maximum_functions: usize = 64;
pub const maximum_constants: usize = 128;
pub const maximum_instructions: usize = 4096;
pub const maximum_name_bytes: usize = 128;
pub const maximum_locals: usize = 128;
pub const maximum_stack_values: usize = 256;

/// Only the complete parser can construct this artifact-derived witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct derivation {
    authority: u64,
    intent: [u8; maximum_intent_bytes],
    intent_length: usize,
}

impl derivation {
    pub const fn authority(&self) -> u64 {
        self.authority
    }

    pub fn intent(&self) -> &str {
        // The slice was copied from a checked UTF-8 string at character bounds.
        core::str::from_utf8(&self.intent[..self.intent_length]).unwrap()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum value_kind {
    integer,
    text,
}

struct function_state<'a> {
    locals: [Option<&'a str>; maximum_locals],
    local_kinds: [value_kind; maximum_locals],
    local_count: usize,
    stack: [value_kind; maximum_stack_values],
    depth: usize,
    returned: bool,
}

impl<'a> function_state<'a> {
    fn new() -> Self {
        Self {
            locals: [None; maximum_locals],
            local_kinds: [value_kind::integer; maximum_locals],
            local_count: 0,
            stack: [value_kind::integer; maximum_stack_values],
            depth: 0,
            returned: false,
        }
    }

    fn push(&mut self, kind: value_kind) -> Result<(), error> {
        if self.depth == maximum_stack_values {
            return Err(error::limit);
        }
        self.stack[self.depth] = kind;
        self.depth += 1;
        Ok(())
    }

    fn pop(&mut self) -> Result<value_kind, error> {
        self.depth = self.depth.checked_sub(1).ok_or(error::syntax)?;
        Ok(self.stack[self.depth])
    }

    fn integer(&mut self) -> Result<(), error> {
        if self.pop()? != value_kind::integer {
            return Err(error::syntax);
        }
        Ok(())
    }

    fn instruction(
        &mut self,
        line: &'a str,
        constants: &[Option<&str>],
        derived_authority: &mut u64,
    ) -> Result<(), error> {
        if self.returned {
            return Err(error::syntax);
        }
        let (operands, count) = instruction_fields(line)?;
        let fields = &operands[..count];
        match fields {
            ["PUSH_CONST", value] => {
                signed_decimal(value)?;
                self.push(value_kind::integer)?;
            }
            ["PUSH_CONST_STR", name] => {
                identifier(name)?;
                if !constants.contains(&Some(*name)) {
                    return Err(error::syntax);
                }
                self.push(value_kind::text)?;
            }
            ["STORE_LOCAL", name] => {
                identifier(name)?;
                if builtin(name) {
                    return Err(error::unsupported);
                }
                let kind = self.pop()?;
                if let Some(index) = self.locals[..self.local_count]
                    .iter()
                    .position(|entry| *entry == Some(*name))
                {
                    if self.local_kinds[index] != kind {
                        return Err(error::syntax);
                    }
                } else {
                    if self.local_count == maximum_locals {
                        return Err(error::limit);
                    }
                    self.locals[self.local_count] = Some(name);
                    self.local_kinds[self.local_count] = kind;
                    self.local_count += 1;
                }
            }
            ["LOAD_LOCAL", name] => {
                identifier(name)?;
                let index = self.locals[..self.local_count]
                    .iter()
                    .position(|entry| *entry == Some(*name))
                    .ok_or(error::syntax)?;
                self.push(self.local_kinds[index])?;
            }
            ["CAST", "i32" | "i64"] | ["NEG", "i32" | "i64"] => {
                self.integer()?;
                self.push(value_kind::integer)?;
            }
            [
                "ADD" | "SUB" | "MUL" | "DIV" | "MOD" | "BIT_AND" | "BIT_OR" | "BIT_XOR" | "SHL"
                | "SHR",
                "i32" | "i64",
            ] => {
                self.integer()?;
                self.integer()?;
                self.push(value_kind::integer)?;
            }
            ["CMP_EQ" | "CMP_NE" | "CMP_LT" | "CMP_LE" | "CMP_GT" | "CMP_GE"] => {
                self.integer()?;
                self.integer()?;
                self.push(value_kind::integer)?;
            }
            ["NOT"] => {
                self.integer()?;
                self.push(value_kind::integer)?;
            }
            ["POP"] => {
                self.pop()?;
            }
            ["NOP"] => {}
            ["RET"] => {
                self.integer()?;
                if self.depth != 0 {
                    return Err(error::syntax);
                }
                self.returned = true;
            }
            ["CALL", name, "1"] if builtin(name) => {
                self.pop()?;
                *derived_authority |= authority::console_write;
            }
            ["CONSOLE_WRITE" | "CONSOLE_WRITELN"]
            | [
                "INTEROP_CALL",
                "intrinsic",
                "console.write" | "console.writeln",
                "1",
                "void",
            ] => {
                self.pop()?;
                *derived_authority |= authority::console_write;
            }
            _ => return Err(error::unsupported),
        }
        Ok(())
    }
}

/// Derive authority and the one supported authored intent from the whole file.
/// No allocation, imports, external symbol resolution, or execution occurs.
pub fn derive(edition: &[u8]) -> Result<derivation, error> {
    if edition.is_empty() || edition.len() > maximum_edition_bytes {
        return Err(error::edition);
    }
    let text = core::str::from_utf8(edition).map_err(|_| error::edition)?;
    if text
        .chars()
        .any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
    {
        return Err(error::syntax);
    }
    let mut lines = text.split('\n');
    if lines.next() != Some("# NIL Textual Module") {
        return Err(error::syntax);
    }
    let mut module_seen = false;
    let mut constants_section = false;
    let mut constants = [None; maximum_constants];
    let mut constant_count = 0;
    let mut functions = [None; maximum_functions];
    let mut function_count = 0;
    let mut instructions = 0;
    let mut active: Option<function_state<'_>> = None;
    let mut declaration_pending = false;
    let mut intent_pending = false;
    let mut main_seen = false;
    let mut result = derivation {
        authority: 0,
        intent: [0; maximum_intent_bytes],
        intent_length: 0,
    };

    for raw_line in lines {
        let line = trim(raw_line);
        if line.is_empty() {
            continue;
        }
        if let Some(function) = active.as_mut() {
            if line == "end function" {
                if !function.returned {
                    return Err(error::syntax);
                }
                active = None;
                continue;
            }
            if instructions == maximum_instructions {
                return Err(error::limit);
            }
            instructions += 1;
            function.instruction(line, &constants[..constant_count], &mut result.authority)?;
            continue;
        }
        if !module_seen {
            let name = line.strip_prefix("module ").ok_or(error::syntax)?;
            if name.is_empty() {
                return Err(error::syntax);
            }
            for segment in name.split('.') {
                identifier(segment)?;
            }
            module_seen = true;
            continue;
        }
        if let Some(header) = line.strip_prefix("function ") {
            if !declaration_pending {
                return Err(error::syntax);
            }
            let (name, return_type) = header.split_once(" -> ").ok_or(error::unsupported)?;
            identifier(name)?;
            if builtin(name) || !matches!(return_type, "i32:" | "i64:") {
                return Err(error::unsupported);
            }
            if functions[..function_count].contains(&Some(name)) {
                return Err(error::syntax);
            }
            if function_count == maximum_functions {
                return Err(error::limit);
            }
            functions[function_count] = Some(name);
            function_count += 1;
            if name == "main" {
                if main_seen || !intent_pending {
                    return Err(error::intent);
                }
                main_seen = true;
            } else if intent_pending {
                return Err(error::intent);
            }
            declaration_pending = false;
            intent_pending = false;
            active = Some(function_state::new());
            continue;
        }
        if declaration_pending {
            return Err(error::syntax);
        }
        if let Some(number) = line.strip_prefix("# declaration_line ") {
            debug_line(number, true)?;
            declaration_pending = true;
            continue;
        }
        if line.starts_with('@') {
            if intent_pending || result.intent_length != 0 {
                return Err(error::intent);
            }
            let authored = line
                .strip_prefix("@ai_context(\"")
                .and_then(|value| value.strip_suffix("\")"))
                .ok_or(error::unsupported)?;
            // The current compiler retains legacy annotation arguments after
            // source escape decoding. A second escape decoder would change
            // some authored backslashes. Admit the unambiguous plain subset.
            if authored.is_empty()
                || authored
                    .chars()
                    .any(|ch| ch.is_control() || ch == '"' || ch == '\\')
            {
                return Err(error::intent);
            }
            if authored.len() > maximum_intent_bytes {
                return Err(error::limit);
            }
            result.intent[..authored.len()].copy_from_slice(authored.as_bytes());
            result.intent_length = authored.len();
            intent_pending = true;
            continue;
        }
        if intent_pending {
            return Err(error::intent);
        }
        if line == "# String constants" {
            if constants_section || function_count != 0 {
                return Err(error::syntax);
            }
            constants_section = true;
            continue;
        }
        if let Some(constant) = line.strip_prefix("const ") {
            if !constants_section || function_count != 0 {
                return Err(error::syntax);
            }
            let (name, value) = constant.split_once(" = ").ok_or(error::syntax)?;
            identifier(name)?;
            if constants[..constant_count].contains(&Some(name)) {
                return Err(error::syntax);
            }
            if constant_count == maximum_constants {
                return Err(error::limit);
            }
            string_constant(value)?;
            constants[constant_count] = Some(name);
            constant_count += 1;
            continue;
        }
        // In particular, reserved # function_annotation, declaration_annotation
        // and source-debug records cannot disappear as generic comments.
        return Err(error::unsupported);
    }
    if !module_seen || active.is_some() || declaration_pending || intent_pending {
        return Err(error::syntax);
    }
    if !main_seen || result.intent_length == 0 {
        return Err(error::intent);
    }
    Ok(result)
}

fn trim(value: &str) -> &str {
    value.trim_matches([' ', '\t'])
}

fn identifier(value: &str) -> Result<(), error> {
    if value.len() > maximum_name_bytes {
        return Err(error::limit);
    }
    let mut bytes = value.bytes();
    if !bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(error::syntax);
    }
    Ok(())
}

fn builtin(name: &str) -> bool {
    matches!(
        name,
        "println" | "core__print" | "console__writeln" | "console__write" | "console__sys_write"
    )
}

fn debug_line(value: &str, allow_zero: bool) -> Result<(), error> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(error::syntax);
    }
    let number = value.parse::<u32>().map_err(|_| error::syntax)?;
    if number > i32::MAX as u32 || (!allow_zero && number == 0) {
        return Err(error::syntax);
    }
    Ok(())
}

fn signed_decimal(value: &str) -> Result<(), error> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(error::unsupported);
    }
    value.parse::<i64>().map_err(|_| error::syntax)?;
    Ok(())
}

// Shared by admission and the witness-only execution view. This does not
// validate the stack or grant authority by itself.
pub(crate) fn instruction_fields(line: &str) -> Result<([&str; 6], usize), error> {
    let body = instruction_body(line)?;
    if body.bytes().any(|byte| matches!(byte, b'"' | b'\\' | b',')) {
        return Err(error::unsupported);
    }
    let mut operands = [""; 6];
    let mut count = 0;
    for operand in body.split_ascii_whitespace() {
        if count == operands.len() {
            return Err(error::unsupported);
        }
        operands[count] = operand;
        count += 1;
    }
    Ok((operands, count))
}

fn instruction_body(line: &str) -> Result<&str, error> {
    if let Some((body, comment)) = line.split_once(';') {
        let number = trim(comment)
            .strip_prefix("line ")
            .ok_or(error::unsupported)?;
        debug_line(number, false)?;
        Ok(trim(body))
    } else {
        Ok(line)
    }
}

fn string_constant(value: &str) -> Result<(), error> {
    let mut chars = value.chars();
    if chars.next() != Some('"') {
        return Err(error::syntax);
    }
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                return if chars.next().is_none() {
                    Ok(())
                } else {
                    Err(error::syntax)
                };
            }
            '\\' => match chars.next() {
                Some('"' | '\\' | 'n' | 'r' | 't' | 'v' | 'f' | 'b') => {}
                _ => return Err(error::unsupported),
            },
            ch if ch.is_control() => return Err(error::syntax),
            _ => {}
        }
    }
    Err(error::syntax)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::{format, string::String};

    fn edition(body: &str) -> String {
        format!(
            "# NIL Textual Module\nmodule root\n@ai_context(\"Calculate a result\")\n# declaration_line 1\nfunction main -> i32:\n{body}\nend function\n"
        )
    }

    #[test]
    fn plain_intent_is_preserved_without_authority() {
        let source =
            edition("PUSH_CONST 42\nCAST i32\nRET").replace("Calculate a result", "Calculate — ✓");
        let result = derive(source.as_bytes()).unwrap();
        assert_eq!(result.authority(), 0);
        assert_eq!(result.intent(), "Calculate — ✓");
    }

    #[test]
    fn authored_fixtures_admit_but_complete_compiler_outputs_refuse() {
        let arithmetic = include_bytes!("../fixtures/arithmetic.nil");
        let console = include_bytes!("../fixtures/console.nil");
        assert_eq!(derive(arithmetic).unwrap().authority(), 0);
        assert_eq!(
            derive(console).unwrap().authority(),
            authority::console_write
        );
        // The compiler-added registration helper is an external call. Retain
        // the whole artifact and refuse it even though no main call reaches it.
        for compiled in [
            include_bytes!("../fixtures/compiler-arithmetic.nil").as_slice(),
            include_bytes!("../fixtures/compiler-console.nil").as_slice(),
        ] {
            assert_eq!(derive(compiled), Err(error::unsupported));
        }
    }

    #[test]
    fn every_supported_console_spelling_derives_the_same_authority() {
        for instruction in [
            "CALL println 1",
            "CALL core__print 1",
            "CALL console__writeln 1",
            "CALL console__write 1",
            "CALL console__sys_write 1",
            "CONSOLE_WRITE",
            "CONSOLE_WRITELN",
            "INTEROP_CALL intrinsic console.write 1 void",
            "INTEROP_CALL intrinsic console.writeln 1 void",
        ] {
            let source = edition(&format!("PUSH_CONST 42\n{instruction}\nPUSH_CONST 0\nRET"));
            assert_eq!(
                derive(source.as_bytes()).unwrap().authority(),
                authority::console_write,
                "{instruction}"
            );
        }
    }

    #[test]
    fn unused_function_authority_is_included_and_unreachable_records_refuse() {
        let source = edition("PUSH_CONST 0\nRET")
            + "# declaration_line 9\nfunction unused -> i64:\nPUSH_CONST 1\nCONSOLE_WRITE\nPUSH_CONST 0\nRET\nend function\n";
        assert_eq!(
            derive(source.as_bytes()).unwrap().authority(),
            authority::console_write
        );
        assert!(
            derive(
                source
                    .replace(
                        "function unused -> i64:",
                        "function unused -> i64:\nTIME_NOW"
                    )
                    .as_bytes()
            )
            .is_err()
        );
        assert!(derive(edition("PUSH_CONST 0\nRET\nCONSOLE_WRITE").as_bytes()).is_err());
    }

    #[test]
    fn unsupported_effects_and_call_variants_never_become_empty_authority() {
        for operation in [
            "CALL print 1",
            "CALL sys_write 1",
            "CALL helper 0",
            "CALL println 0",
            "CALL println 2",
            "CALL \"println\" 1",
            "CALL println, 1",
            "TIME_NOW",
            "TIME_SLEEP",
            "INLINE_ASM x86_64 S1 0 0",
            "READ_PTR",
            "WRITE_PTR",
            "INTEROP_CALL intrinsic file.read_text 1 text",
            "INTEROP_CALL std_time now 0 i64",
            "INTEROP_CALL intrinsic console.read_line 0 text",
            "INTEROP_CALL host callback 0 void",
            "INTEROP_CALL default sys_write 1 void",
            "INTEROP_CALL intrinsic console.write 1",
            "INTEROP_CALL intrinsic console.write 1 i64",
            "INTEROP_CALL intrinsic console.write 1 void text",
            "LOAD_GLOBAL secret i64",
            "JMP label",
            "PUSH_FUNCTION helper",
            "FUTURE_WRITE",
        ] {
            assert!(
                derive(
                    edition(&format!("PUSH_CONST 1\n{operation}\nPUSH_CONST 0\nRET")).as_bytes()
                )
                .is_err(),
                "{operation}"
            );
        }
    }

    #[test]
    fn builtin_shadowing_cannot_change_call_resolution() {
        let source = edition(
            "PUSH_CONST 1\nSTORE_LOCAL println\nPUSH_CONST 1\nCALL println 1\nPUSH_CONST 0\nRET",
        );
        assert!(derive(source.as_bytes()).is_err());
        let source = edition("PUSH_CONST 0\nRET")
            + "# declaration_line 9\nfunction println -> i64:\nPUSH_CONST 0\nRET\nend function\n";
        assert!(derive(source.as_bytes()).is_err());
    }

    #[test]
    fn quoted_constants_are_data_and_cannot_hide_records() {
        let original = edition("PUSH_CONST_STR S1\nCALL println 1\nPUSH_CONST 0\nRET");
        let source = original.replace("module root\n", "module root\n# String constants\nconst S1 = \"quoted \\\"; TIME_NOW\\\" and \\n text\"\n");
        assert_eq!(
            derive(source.as_bytes()).unwrap().authority(),
            authority::console_write
        );
        for value in [
            "\"ok\" TIME_NOW",
            "\"unfinished",
            "\"escape\\q\"",
            "\"nul\\0\"",
        ] {
            let source = original.replace(
                "module root\n",
                &format!("module root\n# String constants\nconst S1 = {value}\n"),
            );
            assert!(derive(source.as_bytes()).is_err(), "{value}");
        }
    }

    #[test]
    fn only_recorded_comments_are_accepted_in_their_structural_position() {
        let source = edition("PUSH_CONST 0 ; line 2\nRET ; line 2");
        assert!(derive(source.as_bytes()).is_ok());
        for record in [
            "# function_annotation \"@ai_context(\\\"hidden\\\")\"",
            "# declaration_annotation \"@ai_context(\\\"hidden\\\")\"",
            "# source_debug forged",
            "# unknown metadata",
            "# declaration_line 4 extra",
            "# declaration_line 2147483648",
            "module second",
            "@interop(\"host\")",
        ] {
            assert!(
                derive(
                    source
                        .replace("@ai_context", &format!("{record}\n@ai_context"))
                        .as_bytes()
                )
                .is_err(),
                "{record}"
            );
        }
        for body in [
            "PUSH_CONST 0 ; ignored",
            "PUSH_CONST 0 ; line 2 ; TIME_NOW",
            "PUSH_CONST 0 ; line 0",
            "# declaration_line 2\nPUSH_CONST 0",
        ] {
            assert!(derive(edition(&format!("{body}\nRET")).as_bytes()).is_err());
        }
    }

    #[test]
    fn intent_is_exactly_one_plain_nonempty_main_annotation() {
        let source = edition("PUSH_CONST 0\nRET");
        for annotation in [
            "",
            "@ai_context",
            "@ai_context(\"\")",
            "@ai_contextual(\"wrong\")",
            "@ai_context(\"one\", \"two\")",
            "@ai_context(intent=\"named\")",
            "@ai_context(\"slash\\n\")",
            "@ai_context(\"a \\\"quote\\\"\")",
            "@ai_context(\"one\")\n@ai_context(\"two\")",
            "@ai_context(\"tab\there\")",
        ] {
            assert!(
                derive(
                    source
                        .replace("@ai_context(\"Calculate a result\")", annotation)
                        .as_bytes()
                )
                .is_err(),
                "{annotation}"
            );
        }
        assert!(derive(source.replace("function main", "function other").as_bytes()).is_err());
        let maximal = source.replace("Calculate a result", &"é".repeat(maximum_intent_bytes / 2));
        assert_eq!(
            derive(maximal.as_bytes()).unwrap().intent().len(),
            maximum_intent_bytes
        );
        assert!(
            derive(
                source
                    .replace("Calculate a result", &"a".repeat(maximum_intent_bytes + 1))
                    .as_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn malformed_structure_references_and_stack_shapes_refuse() {
        let source = edition("PUSH_CONST 0\nRET");
        for changed in [
            source.replace("# declaration_line 1\n", ""),
            source.replace("end function", ""),
            source.clone() + "TIME_NOW\n",
            source.replace("function main -> i32:", "function main() -> i32:"),
            source.replace("function main -> i32:", "function main(value: i32) -> i32:"),
            source.replace("PUSH_CONST 0", "PUSH_CONST_STR missing"),
            source.replace("PUSH_CONST 0", "LOAD_LOCAL missing"),
            source.replace("PUSH_CONST 0", "PUSH_CONST 9223372036854775808"),
            source.replace("PUSH_CONST 0", "PUSH_CONST 0x2a"),
            source.replace("PUSH_CONST 0", "PUSH_CONST 0\nPUSH_CONST 1"),
            source.replace("PUSH_CONST 0", "ADD i32"),
        ] {
            assert!(derive(changed.as_bytes()).is_err(), "{changed}");
        }
        assert!(derive(&[0xff]).is_err());
        assert!(derive(source.replace("module root", "module ro\0ot").as_bytes()).is_err());
        assert!(derive(&[b' '; maximum_edition_bytes + 1]).is_err());
    }

    #[test]
    fn instruction_function_constant_local_and_stack_limits_are_checked() {
        let exact = edition(&("NOP\n".repeat(maximum_instructions - 2) + "PUSH_CONST 0\nRET"));
        assert!(derive(exact.as_bytes()).is_ok());
        assert!(
            derive(
                exact
                    .replace("PUSH_CONST 0", "NOP\nPUSH_CONST 0")
                    .as_bytes()
            )
            .is_err()
        );

        let mut functions = edition("PUSH_CONST 0\nRET");
        for index in 1..maximum_functions {
            functions += &format!(
                "# declaration_line 1\nfunction helper_{index} -> i32:\nPUSH_CONST 0\nRET\nend function\n"
            );
        }
        assert!(derive(functions.as_bytes()).is_ok());
        functions +=
            "# declaration_line 1\nfunction extra -> i32:\nPUSH_CONST 0\nRET\nend function\n";
        assert!(derive(functions.as_bytes()).is_err());

        let mut constants = String::from("# String constants\n");
        for index in 0..maximum_constants {
            constants += &format!("const S{index} = \"value\"\n");
        }
        let source = edition("PUSH_CONST 0\nRET");
        assert!(
            derive(
                source
                    .replace("module root\n", &format!("module root\n{constants}"))
                    .as_bytes()
            )
            .is_ok()
        );
        constants += "const extra = \"value\"\n";
        assert!(
            derive(
                source
                    .replace("module root\n", &format!("module root\n{constants}"))
                    .as_bytes()
            )
            .is_err()
        );

        let mut body = String::new();
        for index in 0..maximum_locals {
            body += &format!("PUSH_CONST 0\nSTORE_LOCAL local_{index}\n");
        }
        assert!(derive(edition(&(body.clone() + "PUSH_CONST 0\nRET")).as_bytes()).is_ok());
        assert!(
            derive(
                edition(&(body + "PUSH_CONST 0\nSTORE_LOCAL extra\nPUSH_CONST 0\nRET")).as_bytes()
            )
            .is_err()
        );

        let body = "PUSH_CONST 0\n".repeat(maximum_stack_values)
            + &"POP\n".repeat(maximum_stack_values - 1)
            + "RET";
        assert!(derive(edition(&body).as_bytes()).is_ok());
        assert!(derive(edition(&(String::from("PUSH_CONST 0\n") + &body)).as_bytes()).is_err());
    }
}
