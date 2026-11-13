//! Iterative host-only execution of the separately admitted control-flow profile.
//! Storage is caller-owned, fixed-capacity and scrubbed at both ends of a run.

use super::{
    binary, console_sink, error, execution, maximum_output_bytes, narrow, publish_console,
    publish_decoded_console, statistics,
};
use anomalous_program_flow::{admitted_program, kind, operation, program};

mod array;
use array::{array_arena, array_handle};
mod text;
use text::{text_arena, text_span};

pub const maximum_steps: usize = 65536;
pub const maximum_frames: usize = 16;
pub const maximum_values: usize = 256;
pub const maximum_locals: usize = 128;
/// Cumulative decoded concatenation bytes per execution, with no reclamation
/// until the run ends. Literals remain borrowed and do not consume this arena.
pub const maximum_text_bytes: usize = 65536;
/// Immutable arrays retain descriptors and elements until the execution ends.
pub const maximum_arrays: usize = 128;
pub const maximum_array_elements: usize = 1024;

/// A function-table identity and its validated canonical signature. Never an
/// address or a name derived from text; values remain private to one run.
#[derive(Clone, Copy)]
struct function_handle {
    target: usize,
    signature: u8,
}

#[derive(Clone, Copy)]
enum value<'a> {
    integer(i64),
    text(&'a str),
    computed_text(text_span),
    array(array_handle),
    function(function_handle),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct limits {
    pub instructions: usize,
    /// Includes the main frame. Zero refuses before any instruction executes.
    pub frames: usize,
    pub output_bytes: usize,
}

impl Default for limits {
    fn default() -> Self {
        Self {
            instructions: maximum_steps,
            frames: maximum_frames,
            output_bytes: maximum_output_bytes,
        }
    }
}

#[derive(Clone, Copy)]
struct frame<'a> {
    function: usize,
    floor: usize,
    resume: usize,
    locals: [Option<value<'a>>; maximum_locals],
}

impl frame<'_> {
    const fn empty() -> Self {
        Self {
            function: 0,
            floor: 0,
            resume: 0,
            locals: [None; maximum_locals],
        }
    }
}

/// No allocation or Rust recursion is used during execution. This storage is
/// not a qualified native stack layout; a future embedding must place and
/// measure it explicitly. The witness/edition lifetime bounds all borrowed text.
pub struct execution_workspace<'a> {
    frames: [frame<'a>; maximum_frames],
    frame_count: usize,
    values: [value<'a>; maximum_values],
    depth: usize,
    pc: usize,
    stats: statistics,
    line: usize,
    text: text_arena,
    arrays: array_arena<'a>,
}

impl Default for execution_workspace<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> execution_workspace<'a> {
    pub const fn new() -> Self {
        Self {
            frames: [frame::empty(); maximum_frames],
            frame_count: 0,
            values: [value::integer(0); maximum_values],
            depth: 0,
            pc: 0,
            stats: statistics {
                instructions: 0,
                console_calls: 0,
                output_bytes: 0,
            },
            line: 0,
            text: text_arena::new(),
            arrays: array_arena::new(),
        }
    }

    fn clear(&mut self) {
        self.frames.fill(frame::empty());
        self.values.fill(value::integer(0));
        self.frame_count = 0;
        self.depth = 0;
        self.pc = 0;
        self.stats = statistics::default();
        self.line = 0;
        self.text.clear();
        self.arrays.clear();
    }

    fn current(&self) -> Result<&frame<'a>, error> {
        self.frames
            .get(
                self.frame_count
                    .checked_sub(1)
                    .ok_or(error::invalid_state)?,
            )
            .ok_or(error::invalid_state)
    }

    fn current_mut(&mut self) -> Result<&mut frame<'a>, error> {
        self.frames
            .get_mut(
                self.frame_count
                    .checked_sub(1)
                    .ok_or(error::invalid_state)?,
            )
            .ok_or(error::invalid_state)
    }

    fn push(&mut self, item: value<'a>) -> Result<(), error> {
        *self.values.get_mut(self.depth).ok_or(error::stack_limit)? = item;
        self.depth += 1;
        Ok(())
    }

    fn pop(&mut self) -> Result<value<'a>, error> {
        if self.depth <= self.current()?.floor {
            return Err(error::invalid_state);
        }
        self.depth -= 1;
        let item = self.values[self.depth];
        self.values[self.depth] = value::integer(0);
        Ok(item)
    }

    fn integer(&mut self) -> Result<i64, error> {
        match self.pop()? {
            value::integer(number) => Ok(number),
            value::text(_) | value::computed_text(_) | value::array(_) | value::function(_) => {
                Err(error::invalid_state)
            }
        }
    }

    fn call(
        &mut self,
        program: &program<'a>,
        function: usize,
        resume: usize,
        budget: limits,
    ) -> Result<(), error> {
        let callee = program
            .functions()
            .get(function)
            .ok_or(error::invalid_state)?;
        let count = callee.parameter_kinds().len();
        let floor = self.depth.checked_sub(count).ok_or(error::invalid_state)?;
        if self.frame_count > 0 && floor < self.current()?.floor {
            return Err(error::invalid_state);
        }
        if self.frame_count >= budget.frames {
            return Err(error::frame_limit);
        }
        for (item, expected) in self.values[floor..self.depth]
            .iter()
            .zip(callee.parameter_kinds())
        {
            if value_kind(*item) != *expected {
                return Err(error::invalid_state);
            }
        }
        // Argument order and values are untouched. The program's own reversed
        // STORE_LOCAL prologue consumes them; any scalar narrowing is a CAST.
        self.frames[self.frame_count] = frame {
            function,
            floor,
            resume,
            locals: [None; maximum_locals],
        };
        self.frame_count += 1;
        self.pc = callee.start;
        Ok(())
    }

    fn run(
        &mut self,
        program: &program<'a>,
        identity: [u8; 32],
        granted: u64,
        budget: limits,
        sink: &mut impl console_sink,
    ) -> Result<i64, error> {
        if budget.instructions > maximum_steps
            || budget.frames > maximum_frames
            || budget.output_bytes > maximum_output_bytes
        {
            return Err(error::invalid_limits);
        }
        self.call(program, program.main(), 0, budget)?;
        loop {
            let current = self.current()?;
            let function = program
                .functions()
                .get(current.function)
                .ok_or(error::invalid_state)?;
            if self.pc < function.start || self.pc >= function.end {
                return Err(error::invalid_state);
            }
            let instruction = program
                .instructions()
                .get(self.pc)
                .ok_or(error::invalid_state)?;
            self.line = instruction.line;
            if self.stats.instructions == budget.instructions {
                return Err(error::instruction_limit);
            }
            self.stats.instructions += 1;
            let next = self.pc + 1;
            match instruction.operation {
                operation::integer(number) => self.push(value::integer(number))?,
                operation::text(index) => {
                    let text = program
                        .constants()
                        .get(index)
                        .ok_or(error::invalid_state)?
                        .escaped;
                    self.push(value::text(text))?;
                }
                operation::function(target) => {
                    let callee = program
                        .functions()
                        .get(target)
                        .ok_or(error::invalid_state)?;
                    self.push(value::function(function_handle {
                        target,
                        signature: callee.signature(),
                    }))?;
                }
                operation::concat => {
                    let right = self.pop()?;
                    let left = self.pop()?;
                    let span = self.text.concatenate(left, right)?;
                    self.push(value::computed_text(span))?;
                }
                operation::array_literal { count, element } => {
                    let start = self.depth.checked_sub(count).ok_or(error::invalid_state)?;
                    if start < self.current()?.floor {
                        return Err(error::invalid_state);
                    }
                    if start >= maximum_values {
                        return Err(error::stack_limit);
                    }
                    let handle = self
                        .arrays
                        .allocate(&self.values[start..self.depth], element)?;
                    self.values[start..self.depth].fill(value::integer(0));
                    self.depth = start;
                    self.push(value::array(handle))?;
                }
                operation::array_len => {
                    let value::array(handle) = self.pop()? else {
                        return Err(error::invalid_state);
                    };
                    self.push(value::integer(self.arrays.length(handle)?))?;
                }
                operation::array_get => {
                    let index = self.integer()?;
                    let value::array(handle) = self.pop()? else {
                        return Err(error::invalid_state);
                    };
                    self.push(self.arrays.get(handle, index)?)?;
                }
                operation::load(index) => {
                    let item = self
                        .current()?
                        .locals
                        .get(index)
                        .and_then(|item| *item)
                        .ok_or(error::invalid_state)?;
                    self.push(item)?;
                }
                operation::store(index) => {
                    let item = self.pop()?;
                    *self
                        .current_mut()?
                        .locals
                        .get_mut(index)
                        .ok_or(error::invalid_state)? = Some(item);
                }
                operation::cast(width) => {
                    let value = narrow(self.integer()?, width)?;
                    self.push(value::integer(value))?;
                }
                operation::negate(width) => {
                    let value = narrow(self.integer()?, width)?;
                    self.push(value::integer(narrow(value.wrapping_neg(), width)?))?;
                }
                operation::binary { opcode, width } => {
                    let right = self.integer()?;
                    let left = self.integer()?;
                    self.push(value::integer(binary(opcode, width, left, right)?))?;
                }
                operation::compare(opcode) => {
                    let right = self.integer()?;
                    let left = self.integer()?;
                    let result = match opcode {
                        "CMP_EQ" => left == right,
                        "CMP_NE" => left != right,
                        "CMP_LT" => left < right,
                        "CMP_LE" => left <= right,
                        "CMP_GT" => left > right,
                        "CMP_GE" => left >= right,
                        _ => return Err(error::invalid_state),
                    };
                    self.push(value::integer(i64::from(result)))?;
                }
                operation::not => {
                    let value = self.integer()?;
                    self.push(value::integer(i64::from(value == 0)))?;
                }
                operation::pop => {
                    self.pop()?;
                }
                operation::nop | operation::label => {}
                operation::jump(target) => {
                    self.pc = target;
                    continue;
                }
                operation::branch { target, nonzero } => {
                    let condition = self.integer()?;
                    if (condition != 0) == nonzero {
                        self.pc = target;
                        continue;
                    }
                }
                operation::call_local { local, arguments } => {
                    let Some(value::function(handle)) =
                        self.current()?.locals.get(local).and_then(|item| *item)
                    else {
                        return Err(error::invalid_state);
                    };
                    let callee = program
                        .functions()
                        .get(handle.target)
                        .ok_or(error::invalid_state)?;
                    if callee.signature() != handle.signature
                        || callee.parameter_kinds().len() != arguments
                    {
                        return Err(error::invalid_state);
                    }
                    // Read the caller-local target before changing frames. The
                    // common call path checks operand kinds, floor and limits.
                    self.call(program, handle.target, next, budget)?;
                    continue;
                }
                operation::call(target) => {
                    self.call(program, target, next, budget)?;
                    continue;
                }
                operation::generated_fallthrough => return Err(error::generated_fallthrough),
                operation::console { newline } => {
                    let item = self.pop()?;
                    match item {
                        value::array(_) | value::function(_) => return Err(error::invalid_state),
                        value::computed_text(span) => publish_decoded_console(
                            self.text.bytes(span)?,
                            newline,
                            identity,
                            granted,
                            budget.output_bytes,
                            &mut self.stats,
                            sink,
                        )?,
                        value::integer(number) => publish_console(
                            super::value::integer(number),
                            newline,
                            identity,
                            granted,
                            budget.output_bytes,
                            &mut self.stats,
                            sink,
                        )?,
                        value::text(escaped) => publish_console(
                            super::value::text(escaped),
                            newline,
                            identity,
                            granted,
                            budget.output_bytes,
                            &mut self.stats,
                            sink,
                        )?,
                    }
                }
                operation::ret => {
                    let frame = self.current()?;
                    let function = program
                        .functions()
                        .get(frame.function)
                        .ok_or(error::invalid_state)?;
                    let count = usize::from(function.return_kind.is_some());
                    if self.depth.checked_sub(frame.floor) != Some(count) {
                        return Err(error::invalid_state);
                    }
                    if let Some(expected) = function.return_kind
                        && value_kind(self.values[frame.floor]) != expected
                    {
                        return Err(error::invalid_state);
                    }
                    if self.frame_count == 1 {
                        return match (count, self.values[frame.floor]) {
                            (1, value::integer(result)) => Ok(result),
                            _ => Err(error::invalid_state),
                        };
                    }
                    let resume = frame.resume;
                    self.frame_count -= 1;
                    self.frames[self.frame_count] = frame::empty();
                    self.pc = resume;
                    continue;
                }
            }
            self.pc = next;
        }
    }
}

fn value_kind(value: value<'_>) -> kind {
    match value {
        value::integer(_) => kind::integer,
        value::text(_) | value::computed_text(_) => kind::text,
        value::array(handle) => handle.kind(),
        value::function(handle) => kind::function(handle.signature),
    }
}

/// Execute only a validated control-flow witness (profile 2 through 6).
/// Every return, arithmetic error, policy
/// refusal or service refusal clears the internal frame/value storage. Earlier
/// acknowledged sink effects remain committed; there is no automatic retry.
pub fn execute<'a>(
    admitted: &admitted_program<'a, '_>,
    budget: limits,
    workspace: &mut execution_workspace<'a>,
    sink: &mut impl console_sink,
) -> execution {
    execute_checked(
        admitted.program(),
        admitted.identity(),
        admitted.granted_services(),
        budget,
        workspace,
        sink,
    )
}

// Both public entry points require their own unforgeable witness. Keep this
// plan-taking implementation private to the crate: a plan is not admission.
pub(crate) fn execute_checked<'a>(
    program: &program<'a>,
    identity: [u8; 32],
    granted: u64,
    budget: limits,
    workspace: &mut execution_workspace<'a>,
    sink: &mut impl console_sink,
) -> execution {
    workspace.clear();
    let outcome = workspace.run(program, identity, granted, budget, sink);
    let result = execution {
        outcome,
        statistics: workspace.stats,
        edition_identity: identity,
        line: workspace.line,
    };
    workspace.clear();
    result
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod scalar_tests;

#[cfg(all(test, feature = "host_conformance"))]
mod conformance_tests;

#[cfg(test)]
mod text_tests;

#[cfg(test)]
mod array_tests;

#[cfg(test)]
mod callable_tests;
