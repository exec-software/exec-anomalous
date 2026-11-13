#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Experimental host qualification executors for admitted NIL (ADRs 0104–0106).
//! This is not a native-integrated or full-corpus-qualified NIL interpreter.
//! Production entries require whole-edition admission. The opt-in host conformance
//! entry accepts a distinct raw validation witness, never a production package.
//! Storage, dispatch and output are bounded; the only effect is an explicit,
//! witness-bound console sink.

use anomalous_native_runtime::{maximum_write_bytes, validate_console_text};
use anomalous_program_manifest::{
    admitted_edition, authority,
    execution_view::{
        maximum_constants, maximum_instructions, maximum_locals, maximum_stack_values, record,
    },
};
use core::fmt::Write;

pub mod flow;

#[cfg(feature = "host_conformance")]
pub mod conformance;

pub const maximum_output_bytes: usize = 65536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum error {
    invalid_limits,
    instruction_limit,
    output_limit,
    output_policy,
    division_by_zero,
    division_overflow,
    shift_count,
    service,
    authority,
    invalid_state,
    frame_limit,
    stack_limit,
    generated_fallthrough,
}

impl error {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::invalid_limits => "invalid_limits",
            Self::instruction_limit => "instruction_limit",
            Self::output_limit => "output_limit",
            Self::output_policy => "output_policy",
            Self::division_by_zero => "division_by_zero",
            Self::division_overflow => "division_overflow",
            Self::shift_count => "shift_count",
            Self::service => "service",
            Self::authority => "authority",
            Self::invalid_state => "invalid_state",
            Self::frame_limit => "frame_limit",
            Self::stack_limit => "stack_limit",
            Self::generated_fallthrough => "generated_fallthrough",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct limits {
    pub instructions: usize,
    pub output_bytes: usize,
}

impl Default for limits {
    fn default() -> Self {
        Self {
            instructions: maximum_instructions,
            output_bytes: maximum_output_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct statistics {
    /// Dispatched instructions, including a failing instruction. Exhaustion
    /// happens before dispatch and does not increment this counter.
    pub instructions: usize,
    /// Nonempty, validated requests attempted (including a refused request).
    pub console_calls: usize,
    /// Bytes acknowledged by successful whole requests, never presumed writes.
    pub output_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct execution {
    pub outcome: Result<i64, error>,
    pub statistics: statistics,
    pub edition_identity: [u8; 32],
    /// Physical edition line at return/refusal, or zero for invalid limits.
    pub line: usize,
}

/// Created only after checking the admitted workload authority and the whole
/// payload. This is a userspace proof object, not a kernel grant. The receiver
/// must still enforce its own task-bound grant before native integration.
pub struct console_request<'a> {
    edition_identity: [u8; 32],
    sequence: usize,
    bytes: &'a [u8],
}

impl console_request<'_> {
    pub const fn edition_identity(&self) -> [u8; 32] {
        self.edition_identity
    }
    pub const fn sequence(&self) -> usize {
        self.sequence
    }
    pub const fn bytes(&self) -> &[u8] {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum service_error {
    refused,
}

pub trait console_sink {
    /// Acknowledge the complete request or return refusal. The executor never
    /// retries. Refusal does not promise rollback of effects inside the sink.
    fn write(&mut self, request: console_request<'_>) -> Result<(), service_error>;
}

#[derive(Clone, Copy)]
enum value<'a> {
    integer(i64),
    text(&'a str),
}

struct machine<'a> {
    stack: [value<'a>; maximum_stack_values],
    depth: usize,
    locals: [Option<(&'a str, value<'a>)>; maximum_locals],
    local_count: usize,
    constants: [Option<(&'a str, &'a str)>; maximum_constants],
    constant_count: usize,
    stats: statistics,
    line: usize,
}

impl<'a> machine<'a> {
    fn new() -> Self {
        Self {
            stack: [value::integer(0); maximum_stack_values],
            depth: 0,
            locals: [None; maximum_locals],
            local_count: 0,
            constants: [None; maximum_constants],
            constant_count: 0,
            stats: statistics::default(),
            line: 0,
        }
    }

    fn push(&mut self, value: value<'a>) -> Result<(), error> {
        let slot = self.stack.get_mut(self.depth).ok_or(error::invalid_state)?;
        *slot = value;
        self.depth += 1;
        Ok(())
    }

    fn pop(&mut self) -> Result<value<'a>, error> {
        self.depth = self.depth.checked_sub(1).ok_or(error::invalid_state)?;
        Ok(self.stack[self.depth])
    }

    fn integer(&mut self) -> Result<i64, error> {
        match self.pop()? {
            value::integer(number) => Ok(number),
            value::text(_) => Err(error::invalid_state),
        }
    }

    fn console(
        &mut self,
        newline: bool,
        admitted: &admitted_edition<'_>,
        budget: limits,
        sink: &mut impl console_sink,
    ) -> Result<(), error> {
        if admitted.granted_services() & authority::console_write == 0 {
            return Err(error::authority);
        }
        let item = self.pop()?;
        publish_console(
            item,
            newline,
            admitted.identity(),
            admitted.granted_services(),
            budget.output_bytes,
            &mut self.stats,
            sink,
        )
    }

    fn run(
        &mut self,
        admitted: &admitted_edition<'a>,
        budget: limits,
        sink: &mut impl console_sink,
    ) -> Result<i64, error> {
        if budget.instructions > maximum_instructions || budget.output_bytes > maximum_output_bytes
        {
            return Err(error::invalid_limits);
        }
        let mut in_main = false;
        for record in admitted.execution_records() {
            match record.map_err(|_| error::invalid_state)? {
                record::constant { name, escaped } => {
                    *self
                        .constants
                        .get_mut(self.constant_count)
                        .ok_or(error::invalid_state)? = Some((name, escaped));
                    self.constant_count += 1;
                }
                record::function { name } => in_main = name == "main",
                record::end_function => in_main = false,
                record::instruction(instruction) if in_main => {
                    self.line = instruction.line;
                    if self.stats.instructions == budget.instructions {
                        return Err(error::instruction_limit);
                    }
                    self.stats.instructions += 1;
                    match instruction.fields() {
                        ["PUSH_CONST", text] => self.push(value::integer(
                            text.parse().map_err(|_| error::invalid_state)?,
                        ))?,
                        ["PUSH_CONST_STR", name] => {
                            let (_, escaped) = self.constants[..self.constant_count]
                                .iter()
                                .flatten()
                                .find(|(key, _)| key == name)
                                .ok_or(error::invalid_state)?;
                            self.push(value::text(escaped))?;
                        }
                        ["STORE_LOCAL", name] => {
                            let item = self.pop()?;
                            if let Some(slot) = self.locals[..self.local_count]
                                .iter_mut()
                                .flatten()
                                .find(|(key, _)| key == name)
                            {
                                slot.1 = item;
                            } else {
                                *self
                                    .locals
                                    .get_mut(self.local_count)
                                    .ok_or(error::invalid_state)? = Some((name, item));
                                self.local_count += 1;
                            }
                        }
                        ["LOAD_LOCAL", name] => {
                            let (_, item) = self.locals[..self.local_count]
                                .iter()
                                .flatten()
                                .find(|(key, _)| key == name)
                                .ok_or(error::invalid_state)?;
                            self.push(*item)?;
                        }
                        ["CAST", width] => {
                            let number = narrow(self.integer()?, width)?;
                            self.push(value::integer(number))?;
                        }
                        ["NEG", width] => {
                            let number = narrow(self.integer()?, width)?;
                            self.push(value::integer(narrow(number.wrapping_neg(), width)?))?;
                        }
                        [
                            operation @ ("ADD" | "SUB" | "MUL" | "DIV" | "MOD" | "BIT_AND"
                            | "BIT_OR" | "BIT_XOR" | "SHL" | "SHR"),
                            width,
                        ] => {
                            let right = self.integer()?;
                            let left = self.integer()?;
                            self.push(value::integer(binary(operation, width, left, right)?))?;
                        }
                        [
                            operation @ ("CMP_EQ" | "CMP_NE" | "CMP_LT" | "CMP_LE" | "CMP_GT"
                            | "CMP_GE"),
                        ] => {
                            let right = self.integer()?;
                            let left = self.integer()?;
                            let truth = match *operation {
                                "CMP_EQ" => left == right,
                                "CMP_NE" => left != right,
                                "CMP_LT" => left < right,
                                "CMP_LE" => left <= right,
                                "CMP_GT" => left > right,
                                "CMP_GE" => left >= right,
                                _ => return Err(error::invalid_state),
                            };
                            self.push(value::integer(i64::from(truth)))?;
                        }
                        ["NOT"] => {
                            let number = self.integer()?;
                            self.push(value::integer(i64::from(number == 0)))?;
                        }
                        ["POP"] => {
                            self.pop()?;
                        }
                        ["NOP"] => {}
                        ["RET"] => {
                            let result = self.integer()?;
                            return if self.depth == 0 {
                                Ok(result)
                            } else {
                                Err(error::invalid_state)
                            };
                        }
                        ["CALL", name, "1"] => {
                            let newline = match *name {
                                "println" | "core__print" | "console__writeln" => true,
                                "console__write" | "console__sys_write" => false,
                                _ => return Err(error::invalid_state),
                            };
                            self.console(newline, admitted, budget, sink)?;
                        }
                        [operation @ ("CONSOLE_WRITE" | "CONSOLE_WRITELN")] => {
                            self.console(*operation == "CONSOLE_WRITELN", admitted, budget, sink)?
                        }
                        [
                            "INTEROP_CALL",
                            "intrinsic",
                            name @ ("console.write" | "console.writeln"),
                            "1",
                            "void",
                        ] => self.console(*name == "console.writeln", admitted, budget, sink)?,
                        _ => return Err(error::invalid_state),
                    }
                }
                record::instruction(_) => {}
            }
        }
        Err(error::invalid_state)
    }
}

/// Run only `main`. Other bodies still contribute authority during admission.
/// Earlier acknowledged output remains observable if a later instruction fails.
/// No source path, ambient host service, allocator, clock, FFI or native grant is read.
pub fn execute(
    admitted: &admitted_edition<'_>,
    budget: limits,
    sink: &mut impl console_sink,
) -> execution {
    let mut machine = machine::new();
    let outcome = machine.run(admitted, budget, sink);
    execution {
        outcome,
        statistics: machine.stats,
        edition_identity: admitted.identity(),
        line: machine.line,
    }
}

// Shared effect path for both admitted profiles. The only way to construct a
// request is through complete payload, authority and budget checks here.
fn publish_console(
    item: value<'_>,
    newline: bool,
    identity: [u8; 32],
    granted: u64,
    output_limit: usize,
    stats: &mut statistics,
    sink: &mut impl console_sink,
) -> Result<(), error> {
    if granted & authority::console_write == 0 {
        return Err(error::authority);
    }
    let mut output = output_buffer::new();
    match item {
        value::integer(number) => {
            write!(&mut output, "{number}").map_err(|_| error::output_limit)?
        }
        value::text(escaped) => output.decode(escaped)?,
    }
    if newline {
        output.append(b"\n")?;
    }
    let bytes = output.bytes();
    // The service rejects empty writes; an empty non-newline instruction
    // has no observable output and therefore makes no service request.
    if bytes.is_empty() {
        return Ok(());
    }
    validate_console_text(bytes).map_err(|_| error::output_policy)?;
    let total = stats
        .output_bytes
        .checked_add(bytes.len())
        .ok_or(error::output_limit)?;
    if total > output_limit {
        return Err(error::output_limit);
    }
    stats.console_calls += 1;
    sink.write(console_request {
        edition_identity: identity,
        sequence: stats.console_calls,
        bytes,
    })
    .map_err(|_| error::service)?;
    stats.output_bytes = total;
    Ok(())
}

fn narrow(number: i64, width: &str) -> Result<i64, error> {
    match width {
        "i32" => Ok(number as i32 as i64),
        "i64" => Ok(number),
        _ => Err(error::invalid_state),
    }
}

fn binary(operation: &str, width: &str, left: i64, original_right: i64) -> Result<i64, error> {
    let left = narrow(left, width)?;
    let right = narrow(original_right, width)?;
    let result = match operation {
        "ADD" => left.wrapping_add(right),
        "SUB" => left.wrapping_sub(right),
        "MUL" => left.wrapping_mul(right),
        "BIT_AND" => left & right,
        "BIT_OR" => left | right,
        "BIT_XOR" => left ^ right,
        "DIV" | "MOD" => {
            if right == 0 {
                return Err(error::division_by_zero);
            }
            let minimum = if width == "i32" {
                i32::MIN as i64
            } else {
                i64::MIN
            };
            if left == minimum && right == -1 {
                return Err(error::division_overflow);
            }
            if operation == "DIV" {
                left / right
            } else {
                left % right
            }
        }
        "SHL" | "SHR" => {
            let bits = if width == "i32" { 32 } else { 64 };
            if !(0..bits).contains(&original_right) {
                return Err(error::shift_count);
            }
            if operation == "SHL" {
                left.wrapping_shl(original_right as u32)
            } else {
                left >> original_right as u32
            }
        }
        _ => return Err(error::invalid_state),
    };
    narrow(result, width)
}

struct output_buffer {
    bytes: [u8; maximum_write_bytes],
    length: usize,
}

impl output_buffer {
    fn new() -> Self {
        Self {
            bytes: [0; maximum_write_bytes],
            length: 0,
        }
    }
    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    fn append(&mut self, bytes: &[u8]) -> Result<(), error> {
        let end = self
            .length
            .checked_add(bytes.len())
            .ok_or(error::output_limit)?;
        self.bytes
            .get_mut(self.length..end)
            .ok_or(error::output_limit)?
            .copy_from_slice(bytes);
        self.length = end;
        Ok(())
    }
    fn decode(&mut self, escaped: &str) -> Result<(), error> {
        let mut chars = escaped.chars();
        while let Some(mut ch) = chars.next() {
            if ch == '\\' {
                ch = match chars.next().ok_or(error::invalid_state)? {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'v' => '\u{b}',
                    'f' => '\u{c}',
                    '\\' => '\\',
                    '"' => '"',
                    // The pinned C++ loader preserves this unknown escape.
                    'b' => {
                        self.append(b"\\b")?;
                        continue;
                    }
                    _ => return Err(error::invalid_state),
                };
            }
            self.append(ch.encode_utf8(&mut [0; 4]).as_bytes())?;
        }
        Ok(())
    }
}

impl Write for output_buffer {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        self.append(text.as_bytes()).map_err(|_| core::fmt::Error)
    }
}

#[cfg(test)]
mod tests;
