#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Host-only profile-2 and explicitly selected profile-3 admission.
//! All syntax and references are checked before
//! reachable CFG validation; even dead instructions contribute authority.
//! The immutable plan is accessible only through a complete validation witness.
//! The opt-in host conformance path does not create production admission.

#[cfg(all(feature = "host_conformance", target_os = "none"))]
compile_error!("host_conformance is unavailable for freestanding targets");

pub use anomalous_program_manifest::{
    authority, error, image_header_bytes, manifest_header_bytes, maximum_edition_bytes,
    maximum_image_bytes, maximum_intent_bytes, sha256,
};

pub const derivation_profile: u32 = 2;
pub const maximum_functions: usize = 64;
pub const maximum_constants: usize = 128;
pub const maximum_instructions: usize = 4096;
pub const maximum_locals: usize = 128;
pub const maximum_stack_values: usize = 256;
pub const maximum_parameters: usize = 8;
pub const maximum_basic_blocks: usize = 256;
pub const maximum_validation_transfers: usize = 1 << 20;
pub const maximum_name_bytes: usize = 128;

#[derive(Clone, Copy, Eq, PartialEq)]
enum parse_profile {
    flow,
    scalar,
}

#[derive(Clone, Copy)]
enum validation_purpose {
    production,
    #[cfg(feature = "host_conformance")]
    conformance,
}

impl validation_purpose {
    const fn requires_intent(self) -> bool {
        match self {
            Self::production => true,
            #[cfg(feature = "host_conformance")]
            Self::conformance => false,
        }
    }
}

impl parse_profile {
    const fn tag(self) -> u32 {
        match self {
            Self::flow => derivation_profile,
            Self::scalar => scalar::derivation_profile,
        }
    }
}

/// Explicit opt-in for bounded compiler temporary locals and integer ADD.
/// Profile 2 retains its original grammar and never selects this parser policy.
pub mod scalar {
    use super::{admitted_program, error, parse_profile, validation_workspace};

    pub const derivation_profile: u32 = 3;

    /// Validates completely before publishing any output bytes.
    pub fn encode<'a>(
        edition: &'a [u8],
        output: &mut [u8],
        workspace: &mut validation_workspace<'a>,
    ) -> Result<usize, error> {
        super::encode_profile(edition, output, workspace, parse_profile::scalar)
    }

    pub fn admit<'a, 'w>(
        image: &'a [u8],
        ceiling: u64,
        workspace: &'w mut validation_workspace<'a>,
    ) -> Result<admitted_program<'a, 'w>, error> {
        super::admit_profile(image, ceiling, workspace, parse_profile::scalar)
    }
}

/// Raw host language checks, deliberately separate from manifest admission.
#[cfg(feature = "host_conformance")]
pub mod conformance;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum kind {
    integer,
    text,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum operation {
    integer(i64),
    text(usize),
    load(usize),
    store(usize),
    binary {
        opcode: &'static str,
        width: &'static str,
    },
    compare(&'static str),
    cast(&'static str),
    negate(&'static str),
    not,
    pop,
    nop,
    label,
    jump(usize),
    branch {
        target: usize,
        nonzero: bool,
    },
    call(usize),
    ret,
    generated_fallthrough,
    console {
        newline: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct instruction {
    pub operation: operation,
    pub line: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct constant<'a> {
    pub name: &'a str,
    pub escaped: &'a str,
    pub line: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct function<'a> {
    pub name: &'a str,
    pub start: usize,
    /// Exclusive instruction index.
    pub end: usize,
    pub return_kind: Option<kind>,
    pub local_count: usize,
    parameters: [kind; maximum_parameters],
    parameter_names: [&'a str; maximum_parameters],
    parameter_count: usize,
    local_names: [&'a str; maximum_locals],
    local_stored: [bool; maximum_locals],
}

impl function<'_> {
    const fn empty() -> Self {
        Self {
            name: "",
            start: 0,
            end: 0,
            return_kind: None,
            local_count: 0,
            parameters: [kind::integer; maximum_parameters],
            parameter_names: [""; maximum_parameters],
            parameter_count: 0,
            local_names: [""; maximum_locals],
            local_stored: [false; maximum_locals],
        }
    }
    pub fn parameter_kinds(&self) -> &[kind] {
        &self.parameters[..self.parameter_count]
    }
}

pub struct program<'a> {
    functions: [function<'a>; maximum_functions],
    function_count: usize,
    instructions: [instruction; maximum_instructions],
    instruction_count: usize,
    constants: [constant<'a>; maximum_constants],
    constant_count: usize,
    main: usize,
    intent: &'a str,
    authority: u64,
}

impl<'a> program<'a> {
    const fn new() -> Self {
        Self {
            functions: [function::empty(); maximum_functions],
            function_count: 0,
            instructions: [instruction {
                operation: operation::nop,
                line: 0,
            }; maximum_instructions],
            instruction_count: 0,
            constants: [constant {
                name: "",
                escaped: "",
                line: 0,
            }; maximum_constants],
            constant_count: 0,
            main: usize::MAX,
            intent: "",
            authority: 0,
        }
    }
    pub fn functions(&self) -> &[function<'a>] {
        &self.functions[..self.function_count]
    }
    pub fn instructions(&self) -> &[instruction] {
        &self.instructions[..self.instruction_count]
    }
    pub fn constants(&self) -> &[constant<'a>] {
        &self.constants[..self.constant_count]
    }
    pub const fn main(&self) -> usize {
        self.main
    }
}

#[derive(Clone, Copy)]
struct label<'a> {
    name: &'a str,
    target: usize,
    owner: usize,
}

#[derive(Clone, Copy)]
struct flow_state {
    stack: [kind; maximum_stack_values],
    depth: usize,
    locals: [Option<kind>; maximum_locals],
}

impl flow_state {
    const fn empty() -> Self {
        Self {
            stack: [kind::integer; maximum_stack_values],
            depth: 0,
            locals: [None; maximum_locals],
        }
    }
    fn push(&mut self, value: kind) -> Result<(), error> {
        if self.depth == maximum_stack_values {
            return Err(error::limit);
        }
        self.stack[self.depth] = value;
        self.depth += 1;
        Ok(())
    }
    fn pop(&mut self) -> Result<kind, error> {
        self.depth = self.depth.checked_sub(1).ok_or(error::syntax)?;
        Ok(self.stack[self.depth])
    }
    fn integer(&mut self) -> Result<(), error> {
        if self.pop()? != kind::integer {
            return Err(error::syntax);
        }
        Ok(())
    }
}

/// Caller-owned fixed storage. Failed validation cannot expose a plan; a live
/// validation witness immutably borrows the workspace and prevents its reuse.
pub struct validation_workspace<'a> {
    plan: program<'a>,
    references: [(&'a str, usize); maximum_instructions],
    labels: [label<'a>; maximum_basic_blocks],
    label_count: usize,
    block_start: [bool; maximum_instructions],
    blocks: [usize; maximum_basic_blocks],
    block_count: usize,
    states: [flow_state; maximum_basic_blocks],
    seen: [bool; maximum_basic_blocks],
    queued: [bool; maximum_basic_blocks],
    queue: [usize; maximum_basic_blocks],
    queue_head: usize,
    queue_length: usize,
}

impl<'a> Default for validation_workspace<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> validation_workspace<'a> {
    pub const fn new() -> Self {
        Self {
            plan: program::new(),
            references: [("", 0); maximum_instructions],
            labels: [label {
                name: "",
                target: 0,
                owner: 0,
            }; maximum_basic_blocks],
            label_count: 0,
            block_start: [false; maximum_instructions],
            blocks: [0; maximum_basic_blocks],
            block_count: 0,
            states: [flow_state::empty(); maximum_basic_blocks],
            seen: [false; maximum_basic_blocks],
            queued: [false; maximum_basic_blocks],
            queue: [0; maximum_basic_blocks],
            queue_head: 0,
            queue_length: 0,
        }
    }

    fn reset(&mut self) {
        self.plan.function_count = 0;
        self.plan.instruction_count = 0;
        self.plan.constant_count = 0;
        self.plan.main = usize::MAX;
        self.plan.intent = "";
        self.plan.authority = 0;
        self.label_count = 0;
        self.block_count = 0;
        self.block_start.fill(false);
        self.seen.fill(false);
        self.queued.fill(false);
        self.queue_head = 0;
        self.queue_length = 0;
    }

    fn parse(
        &mut self,
        edition: &'a [u8],
        profile: parse_profile,
        purpose: validation_purpose,
    ) -> Result<(), error> {
        self.reset();
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
        let mut lines = text.split('\n').enumerate();
        if lines.next().map(|(_, line)| line) != Some("# NIL Textual Module") {
            return Err(error::syntax);
        }
        let (mut module, mut constants, mut declaration, mut intent_pending) =
            (false, false, false, false);
        let mut active: Option<usize> = None;
        for (line_number, raw) in lines {
            let line = trim(raw);
            if line.is_empty() {
                continue;
            }
            if let Some(owner) = active {
                if line == "end function" {
                    let function = &mut self.plan.functions[owner];
                    function.end = self.plan.instruction_count;
                    if function.start == function.end {
                        return Err(error::syntax);
                    }
                    if function.local_stored[..function.local_count].contains(&false) {
                        return Err(error::syntax);
                    }
                    active = None;
                } else {
                    self.parse_instruction(owner, line, line_number + 1, profile)?;
                }
                continue;
            }
            if !module {
                let name = line.strip_prefix("module ").ok_or(error::syntax)?;
                for segment in name.split('.') {
                    identifier(segment)?;
                }
                module = true;
                continue;
            }
            if let Some(header) = line.strip_prefix("function ") {
                if !declaration {
                    return Err(error::syntax);
                }
                let index = self.plan.function_count;
                if index == maximum_functions {
                    return Err(error::limit);
                }
                let mut function = parse_function(header)?;
                if self
                    .plan
                    .functions()
                    .iter()
                    .any(|item| item.name == function.name)
                {
                    return Err(error::syntax);
                }
                if function.name == "main" {
                    if (purpose.requires_intent() && !intent_pending)
                        || self.plan.main != usize::MAX
                        || function.parameter_count != 0
                        || function.return_kind != Some(kind::integer)
                    {
                        return Err(error::intent);
                    }
                    self.plan.main = index;
                } else if intent_pending {
                    return Err(error::intent);
                }
                function.start = self.plan.instruction_count;
                self.plan.functions[index] = function;
                self.plan.function_count += 1;
                active = Some(index);
                declaration = false;
                intent_pending = false;
                continue;
            }
            if declaration {
                return Err(error::syntax);
            }
            if let Some(number) = line.strip_prefix("# declaration_line ") {
                debug_line(number, true)?;
                declaration = true;
                continue;
            }
            if line.starts_with('@') {
                if intent_pending || !self.plan.intent.is_empty() {
                    return Err(error::intent);
                }
                let intent = line
                    .strip_prefix("@ai_context(\"")
                    .and_then(|s| s.strip_suffix("\")"))
                    .ok_or(error::unsupported)?;
                if intent.is_empty()
                    || intent
                        .chars()
                        .any(|ch| ch.is_control() || ch == '"' || ch == '\\')
                {
                    return Err(error::intent);
                }
                if intent.len() > maximum_intent_bytes {
                    return Err(error::limit);
                }
                self.plan.intent = intent;
                intent_pending = true;
                continue;
            }
            if intent_pending {
                return Err(error::intent);
            }
            if line == "# String constants" {
                if constants || self.plan.function_count != 0 {
                    return Err(error::syntax);
                }
                constants = true;
                continue;
            }
            if let Some(value) = line.strip_prefix("const ") {
                if !constants || self.plan.function_count != 0 {
                    return Err(error::syntax);
                }
                let (name, value) = value.split_once(" = ").ok_or(error::syntax)?;
                identifier(name)?;
                if self.plan.constants().iter().any(|item| item.name == name) {
                    return Err(error::syntax);
                }
                let escaped = string_constant(value)?;
                if self.plan.constant_count == maximum_constants {
                    return Err(error::limit);
                }
                self.plan.constants[self.plan.constant_count] = constant {
                    name,
                    escaped,
                    line: line_number + 1,
                };
                self.plan.constant_count += 1;
                continue;
            }
            return Err(error::unsupported);
        }
        if !module || active.is_some() || declaration || intent_pending {
            return Err(error::syntax);
        }
        if self.plan.main == usize::MAX
            || (purpose.requires_intent() && self.plan.intent.is_empty())
        {
            return Err(error::intent);
        }
        self.resolve()?;
        self.validate_flow()
    }

    fn local(
        &mut self,
        owner: usize,
        name: &'a str,
        stored: bool,
        profile: parse_profile,
    ) -> Result<usize, error> {
        local_identifier(name, profile)?;
        if builtin(name).is_some() {
            return Err(error::unsupported);
        }
        let function = &mut self.plan.functions[owner];
        let index = if let Some(index) = function.local_names[..function.local_count]
            .iter()
            .position(|item| *item == name)
        {
            index
        } else {
            if function.local_count == maximum_locals {
                return Err(error::limit);
            }
            let index = function.local_count;
            function.local_names[index] = name;
            function.local_count += 1;
            index
        };
        function.local_stored[index] |= stored;
        Ok(index)
    }

    fn parse_instruction(
        &mut self,
        owner: usize,
        line: &'a str,
        physical_line: usize,
        profile: parse_profile,
    ) -> Result<(), error> {
        let pc = self.plan.instruction_count;
        if pc == maximum_instructions {
            return Err(error::limit);
        }
        let (fields, count) = instruction_fields(line)?;
        let fields = &fields[..count];
        self.references[pc] = ("", 0);
        let operation = match fields {
            ["PUSH_CONST", value] => operation::integer(decimal(value)?),
            ["PUSH_CONST_STR", name] => {
                identifier(name)?;
                operation::text(
                    self.plan
                        .constants()
                        .iter()
                        .position(|item| item.name == *name)
                        .ok_or(error::syntax)?,
                )
            }
            ["LOAD_LOCAL", name] => operation::load(self.local(owner, name, false, profile)?),
            ["STORE_LOCAL", name] => operation::store(self.local(owner, name, true, profile)?),
            ["CAST", value] => operation::cast(width(value)?),
            ["NEG", value] => operation::negate(width(value)?),
            [opcode, value] if binary(opcode).is_some() => operation::binary {
                opcode: binary(opcode).unwrap(),
                width: width(value)?,
            },
            // The reference's untyped ADD is also overloaded for arrays and
            // floats. Reachable CFG validation proves both operands integer;
            // this profile has no array or floating values. Dead code keeps
            // its syntax/reference checks without invented operand states.
            ["ADD"] if profile == parse_profile::scalar => operation::binary {
                opcode: "ADD",
                width: "i64",
            },
            [opcode] if comparison(opcode).is_some() => {
                operation::compare(comparison(opcode).unwrap())
            }
            ["NOT"] => operation::not,
            ["POP"] => operation::pop,
            ["NOP"] => operation::nop,
            ["RET"] => operation::ret,
            ["RET", "$ny.generated.nonvoid-fallthrough"] => {
                if self.plan.functions[owner].return_kind.is_none() {
                    return Err(error::syntax);
                }
                operation::generated_fallthrough
            }
            ["LABEL", name] => {
                identifier(name)?;
                if self.labels[..self.label_count]
                    .iter()
                    .any(|item| item.name == *name)
                {
                    return Err(error::syntax);
                }
                if self.label_count == maximum_basic_blocks {
                    return Err(error::limit);
                }
                self.labels[self.label_count] = label {
                    name,
                    target: pc,
                    owner,
                };
                self.label_count += 1;
                operation::label
            }
            ["JMP", name] | ["JMP_IF", name] | ["JMP_IF_NOT", name] => {
                identifier(name)?;
                self.references[pc] = (name, 0);
                if fields[0] == "JMP" {
                    operation::jump(0)
                } else {
                    operation::branch {
                        target: 0,
                        nonzero: fields[0] == "JMP_IF",
                    }
                }
            }
            ["CALL", name, count] => {
                identifier(name)?;
                let count = unsigned(count)?;
                if let Some(newline) = builtin(name) {
                    if count != 1 {
                        return Err(error::unsupported);
                    }
                    operation::console { newline }
                } else {
                    if count > maximum_parameters {
                        return Err(error::limit);
                    }
                    self.references[pc] = (name, count);
                    operation::call(0)
                }
            }
            ["CONSOLE_WRITE"] => operation::console { newline: false },
            ["CONSOLE_WRITELN"] => operation::console { newline: true },
            ["INTEROP_CALL", "intrinsic", "console.write", "1", "void"] => {
                operation::console { newline: false }
            }
            ["INTEROP_CALL", "intrinsic", "console.writeln", "1", "void"] => {
                operation::console { newline: true }
            }
            _ => return Err(error::unsupported),
        };
        if matches!(operation, operation::console { .. }) {
            self.plan.authority |= authority::console_write;
        }
        self.plan.instructions[pc] = instruction {
            operation,
            line: physical_line,
        };
        self.plan.instruction_count += 1;
        Ok(())
    }

    fn resolve(&mut self) -> Result<(), error> {
        for function in self.plan.functions() {
            for name in function.local_names[..function.local_count]
                .iter()
                .chain(function.parameter_names[..function.parameter_count].iter())
            {
                if builtin(name).is_some()
                    || self
                        .plan
                        .functions()
                        .iter()
                        .any(|function| function.name == *name)
                {
                    return Err(error::unsupported);
                }
            }
        }
        for label in &self.labels[..self.label_count] {
            if self
                .plan
                .functions()
                .iter()
                .any(|function| function.name == label.name)
            {
                return Err(error::syntax);
            }
        }
        for owner in 0..self.plan.function_count {
            let start = self.plan.functions[owner].start;
            let end = self.plan.functions[owner].end;
            self.block_start[start] = true;
            for pc in start..end {
                let (name, count) = self.references[pc];
                let old = self.plan.instructions[pc].operation;
                self.plan.instructions[pc].operation = match old {
                    operation::call(_) => {
                        let target = self
                            .plan
                            .functions()
                            .iter()
                            .position(|function| function.name == name)
                            .ok_or(error::unsupported)?;
                        if self.plan.functions[target].parameter_count != count {
                            return Err(error::syntax);
                        }
                        operation::call(target)
                    }
                    operation::jump(_) | operation::branch { .. } => {
                        let label = self.labels[..self.label_count]
                            .iter()
                            .find(|label| label.name == name)
                            .ok_or(error::syntax)?;
                        if label.owner != owner {
                            return Err(error::syntax);
                        }
                        if let operation::branch { nonzero, .. } = old {
                            operation::branch {
                                target: label.target,
                                nonzero,
                            }
                        } else {
                            operation::jump(label.target)
                        }
                    }
                    _ => old,
                };
                if matches!(old, operation::label) {
                    self.block_start[pc] = true;
                }
                if matches!(
                    old,
                    operation::jump(_)
                        | operation::branch { .. }
                        | operation::ret
                        | operation::generated_fallthrough
                ) && pc + 1 < end
                {
                    self.block_start[pc + 1] = true;
                }
            }
        }
        for pc in 0..self.plan.instruction_count {
            if self.block_start[pc] {
                if self.block_count == maximum_basic_blocks {
                    return Err(error::limit);
                }
                self.blocks[self.block_count] = pc;
                self.block_count += 1;
            }
        }
        Ok(())
    }

    fn enqueue(&mut self, destination: usize, state: &flow_state) -> Result<(), error> {
        let index = self.blocks[..self.block_count]
            .binary_search(&destination)
            .map_err(|_| error::syntax)?;
        let mut changed = !self.seen[index];
        if !self.seen[index] {
            self.states[index] = *state;
            self.seen[index] = true;
        } else {
            let old = &mut self.states[index];
            if old.depth != state.depth || old.stack[..old.depth] != state.stack[..state.depth] {
                return Err(error::syntax);
            }
            for (prior, incoming) in old.locals.iter_mut().zip(state.locals) {
                match (*prior, incoming) {
                    (Some(a), Some(b)) if a != b => return Err(error::syntax),
                    (Some(_), None) => {
                        *prior = None;
                        changed = true;
                    }
                    _ => {}
                }
            }
        }
        if changed && !self.queued[index] {
            if self.queue_length == maximum_basic_blocks {
                return Err(error::limit);
            }
            let tail = (self.queue_head + self.queue_length) % maximum_basic_blocks;
            self.queue[tail] = index;
            self.queue_length += 1;
            self.queued[index] = true;
        }
        Ok(())
    }

    fn validate_flow(&mut self) -> Result<(), error> {
        for index in 0..self.plan.function_count {
            let function = &self.plan.functions[index];
            let start = function.start;
            let mut state = flow_state::empty();
            for kind in function.parameter_kinds() {
                state.push(*kind)?;
            }
            self.enqueue(start, &state)?;
        }
        let mut transfers = 0;
        while self.queue_length != 0 {
            let block = self.queue[self.queue_head];
            self.queue_head = (self.queue_head + 1) % maximum_basic_blocks;
            self.queue_length -= 1;
            self.queued[block] = false;
            let start = self.blocks[block];
            let owner = self
                .plan
                .functions()
                .iter()
                .position(|function| function.start <= start && start < function.end)
                .ok_or(error::syntax)?;
            let function_end = self.plan.functions[owner].end;
            let end = if block + 1 == self.block_count {
                function_end
            } else {
                self.blocks[block + 1].min(function_end)
            };
            let mut state = self.states[block];
            let mut terminal = false;
            for pc in start..end {
                transfers += 1;
                if transfers > maximum_validation_transfers {
                    return Err(error::limit);
                }
                match self.plan.instructions[pc].operation {
                    operation::integer(_) => state.push(kind::integer)?,
                    operation::text(_) => state.push(kind::text)?,
                    operation::load(index) => {
                        state.push(state.locals[index].ok_or(error::syntax)?)?
                    }
                    operation::store(index) => {
                        let kind = state.pop()?;
                        if state.locals[index].is_some_and(|old| old != kind) {
                            return Err(error::syntax);
                        }
                        state.locals[index] = Some(kind);
                    }
                    operation::cast(_) | operation::negate(_) | operation::not => {
                        state.integer()?;
                        state.push(kind::integer)?;
                    }
                    operation::binary { .. } | operation::compare(_) => {
                        state.integer()?;
                        state.integer()?;
                        state.push(kind::integer)?;
                    }
                    operation::pop | operation::console { .. } => {
                        state.pop()?;
                    }
                    operation::nop | operation::label => {}
                    operation::call(target) => {
                        let function = &self.plan.functions[target];
                        let count = function.parameter_count;
                        let base = state.depth.checked_sub(count).ok_or(error::syntax)?;
                        if state.stack[base..state.depth] != *function.parameter_kinds() {
                            return Err(error::syntax);
                        }
                        state.depth = base;
                        if let Some(kind) = function.return_kind {
                            state.push(kind)?;
                        }
                    }
                    operation::jump(target) => {
                        self.enqueue(target, &state)?;
                        terminal = true;
                    }
                    operation::branch { target, .. } => {
                        state.integer()?;
                        self.enqueue(target, &state)?;
                    }
                    operation::ret => {
                        match self.plan.functions[owner].return_kind {
                            None if state.depth == 0 => {}
                            Some(kind) if state.depth == 1 && state.stack[0] == kind => {}
                            _ => return Err(error::syntax),
                        }
                        terminal = true;
                    }
                    operation::generated_fallthrough => terminal = true,
                }
            }
            if !terminal {
                if end == function_end {
                    return Err(error::syntax);
                }
                self.enqueue(end, &state)?;
            }
        }
        Ok(())
    }
}

pub struct admitted_program<'a, 'w> {
    profile: u32,
    edition: &'a [u8],
    identity: [u8; 32],
    required: u64,
    granted: u64,
    plan: &'w program<'a>,
}

impl<'a> admitted_program<'a, '_> {
    pub const fn profile(&self) -> u32 {
        self.profile
    }
    pub const fn edition(&self) -> &'a [u8] {
        self.edition
    }
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub const fn intent(&self) -> &'a str {
        self.plan.intent
    }
    pub const fn required_services(&self) -> u64 {
        self.required
    }
    pub const fn granted_services(&self) -> u64 {
        self.granted
    }
    pub const fn program(&self) -> &program<'a> {
        self.plan
    }
}

/// Validates completely before publishing any output bytes.
pub fn encode<'a>(
    edition: &'a [u8],
    output: &mut [u8],
    workspace: &mut validation_workspace<'a>,
) -> Result<usize, error> {
    encode_profile(edition, output, workspace, parse_profile::flow)
}

fn encode_profile<'a>(
    edition: &'a [u8],
    output: &mut [u8],
    workspace: &mut validation_workspace<'a>,
    profile: parse_profile,
) -> Result<usize, error> {
    workspace.parse(edition, profile, validation_purpose::production)?;
    let intent = workspace.plan.intent.as_bytes();
    let manifest_length = manifest_header_bytes + intent.len();
    let edition_offset = image_header_bytes + manifest_length;
    let total = edition_offset + edition.len();
    if output.len() < total {
        return Err(error::output);
    }
    let identity = sha256(edition);
    let image = &mut output[..total];
    image.fill(0);
    image[..8].copy_from_slice(b"ANPROG\r\n");
    put16(image, 8, 1);
    put16(image, 10, image_header_bytes as u16);
    put32(image, 12, profile.tag());
    put64(image, 16, image_header_bytes as u64);
    put64(image, 24, manifest_length as u64);
    put64(image, 32, edition_offset as u64);
    put64(image, 40, edition.len() as u64);
    put64(image, 48, total as u64);
    let manifest = &mut image[image_header_bytes..edition_offset];
    manifest[..8].copy_from_slice(b"ANSMAN\r\n");
    put16(manifest, 8, 1);
    put16(manifest, 10, manifest_header_bytes as u16);
    put32(manifest, 12, profile.tag());
    put64(manifest, 16, workspace.plan.authority);
    put64(manifest, 24, edition.len() as u64);
    manifest[32..64].copy_from_slice(&identity);
    put32(manifest, 64, intent.len() as u32);
    manifest[manifest_header_bytes..].copy_from_slice(intent);
    image[edition_offset..].copy_from_slice(edition);
    Ok(total)
}

pub fn admit<'a, 'w>(
    image: &'a [u8],
    ceiling: u64,
    workspace: &'w mut validation_workspace<'a>,
) -> Result<admitted_program<'a, 'w>, error> {
    admit_profile(image, ceiling, workspace, parse_profile::flow)
}

fn admit_profile<'a, 'w>(
    image: &'a [u8],
    ceiling: u64,
    workspace: &'w mut validation_workspace<'a>,
    profile: parse_profile,
) -> Result<admitted_program<'a, 'w>, error> {
    workspace.reset();
    if ceiling & !authority::known != 0 {
        return Err(error::ceiling);
    }
    if image.len() < image_header_bytes + manifest_header_bytes || image.len() > maximum_image_bytes
    {
        return Err(error::image);
    }
    if &image[..8] != b"ANPROG\r\n"
        || get16(image, 10) != image_header_bytes as u16
        || image[56..64].iter().any(|b| *b != 0)
    {
        return Err(error::encoding);
    }
    if get16(image, 8) != 1 {
        return Err(error::version);
    }
    if get32(image, 12) != profile.tag() {
        return Err(error::profile);
    }
    let manifest_length = get64(image, 24);
    let edition_offset = get64(image, 32);
    let edition_length = get64(image, 40);
    if get64(image, 16) != image_header_bytes as u64
        || !(manifest_header_bytes as u64 + 1
            ..=manifest_header_bytes as u64 + maximum_intent_bytes as u64)
            .contains(&manifest_length)
        || edition_offset != image_header_bytes as u64 + manifest_length
        || !(1..=maximum_edition_bytes as u64).contains(&edition_length)
        || edition_offset.checked_add(edition_length) != Some(image.len() as u64)
        || get64(image, 48) != image.len() as u64
    {
        return Err(error::layout);
    }
    let manifest = &image[image_header_bytes..edition_offset as usize];
    let edition = &image[edition_offset as usize..];
    if &manifest[..8] != b"ANSMAN\r\n"
        || get16(manifest, 10) != manifest_header_bytes as u16
        || manifest[68..96].iter().any(|b| *b != 0)
    {
        return Err(error::encoding);
    }
    if get16(manifest, 8) != 1 {
        return Err(error::version);
    }
    if get32(manifest, 12) != profile.tag() {
        return Err(error::profile);
    }
    if get64(manifest, 24) != edition_length
        || get32(manifest, 64) as u64 != manifest_length - manifest_header_bytes as u64
    {
        return Err(error::layout);
    }
    let required = get64(manifest, 16);
    if required & !authority::known != 0 {
        return Err(error::unsupported);
    }
    let intent =
        core::str::from_utf8(&manifest[manifest_header_bytes..]).map_err(|_| error::intent)?;
    let identity = sha256(edition);
    if identity.as_slice() != &manifest[32..64] {
        return Err(error::identity);
    }
    workspace.parse(edition, profile, validation_purpose::production)?;
    if required != workspace.plan.authority || intent != workspace.plan.intent {
        return Err(error::mismatch);
    }
    if required & !ceiling != 0 {
        return Err(error::ceiling);
    }
    Ok(admitted_program {
        profile: profile.tag(),
        edition,
        identity,
        required,
        granted: required & ceiling,
        plan: &workspace.plan,
    })
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
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        || !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(error::syntax);
    }
    Ok(())
}

fn local_identifier(value: &str, profile: parse_profile) -> Result<(), error> {
    if profile != parse_profile::scalar || !value.starts_with("$ny.temp.") {
        return identifier(value);
    }
    if value.len() > maximum_name_bytes {
        return Err(error::limit);
    }
    let (prefix, counter) = value[9..].split_once('.').ok_or(error::syntax)?;
    identifier(prefix)?;
    // Retain canonical spelling as identity. The counter is never an index or
    // a quantity, so its bounded decimal bytes need no integer conversion.
    if counter.is_empty()
        || !counter.bytes().all(|byte| byte.is_ascii_digit())
        || (counter.len() > 1 && counter.starts_with('0'))
    {
        return Err(error::syntax);
    }
    Ok(())
}
fn builtin(name: &str) -> Option<bool> {
    match name {
        "println" | "core__print" | "console__writeln" => Some(true),
        "console__write" | "console__sys_write" => Some(false),
        _ => None,
    }
}
fn width(value: &str) -> Result<&'static str, error> {
    match value {
        "i32" => Ok("i32"),
        "i64" => Ok("i64"),
        _ => Err(error::unsupported),
    }
}
fn value_kind(value: &str) -> Result<kind, error> {
    match value {
        "i32" | "i64" => Ok(kind::integer),
        "text" => Ok(kind::text),
        _ => Err(error::unsupported),
    }
}
fn binary(value: &str) -> Option<&'static str> {
    Some(match value {
        "ADD" => "ADD",
        "SUB" => "SUB",
        "MUL" => "MUL",
        "DIV" => "DIV",
        "MOD" => "MOD",
        "BIT_AND" => "BIT_AND",
        "BIT_OR" => "BIT_OR",
        "BIT_XOR" => "BIT_XOR",
        "SHL" => "SHL",
        "SHR" => "SHR",
        _ => return None,
    })
}
fn comparison(value: &str) -> Option<&'static str> {
    Some(match value {
        "CMP_EQ" => "CMP_EQ",
        "CMP_NE" => "CMP_NE",
        "CMP_LT" => "CMP_LT",
        "CMP_LE" => "CMP_LE",
        "CMP_GT" => "CMP_GT",
        "CMP_GE" => "CMP_GE",
        _ => return None,
    })
}
fn unsigned(value: &str) -> Result<usize, error> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(error::syntax);
    }
    value.parse().map_err(|_| error::syntax)
}
fn decimal(value: &str) -> Result<i64, error> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(error::unsupported);
    }
    value.parse().map_err(|_| error::syntax)
}
fn debug_line(value: &str, allow_zero: bool) -> Result<(), error> {
    let number = unsigned(value)?;
    if number > i32::MAX as usize || (!allow_zero && number == 0) {
        return Err(error::syntax);
    }
    Ok(())
}
fn instruction_fields(line: &str) -> Result<([&str; 6], usize), error> {
    let body = if let Some((body, comment)) = line.split_once(';') {
        debug_line(
            trim(comment)
                .strip_prefix("line ")
                .ok_or(error::unsupported)?,
            false,
        )?;
        trim(body)
    } else {
        line
    };
    if body.bytes().any(|b| matches!(b, b'"' | b'\\' | b',')) {
        return Err(error::unsupported);
    }
    let mut fields = [""; 6];
    let mut count = 0;
    for field in body.split_ascii_whitespace() {
        if count == fields.len() {
            return Err(error::unsupported);
        }
        fields[count] = field;
        count += 1;
    }
    Ok((fields, count))
}
fn parse_function(header: &str) -> Result<function<'_>, error> {
    let (left, result) = header.split_once(" -> ").ok_or(error::unsupported)?;
    let result = result.strip_suffix(':').ok_or(error::syntax)?;
    let mut function = function::empty();
    function.return_kind = if result == "void" {
        None
    } else {
        Some(value_kind(result)?)
    };
    let (name, arguments) = if let Some((name, rest)) = left.split_once('(') {
        (name, Some(rest.strip_suffix(')').ok_or(error::syntax)?))
    } else {
        (left, None)
    };
    identifier(name)?;
    if builtin(name).is_some() {
        return Err(error::unsupported);
    }
    function.name = name;
    if let Some(arguments) = arguments.filter(|s| !s.is_empty()) {
        for argument in arguments.split(',') {
            if function.parameter_count == maximum_parameters {
                return Err(error::limit);
            }
            let (name, kind) = trim(argument).split_once(':').ok_or(error::syntax)?;
            let name = trim(name);
            identifier(name)?;
            if builtin(name).is_some()
                || function.parameter_names[..function.parameter_count].contains(&name)
            {
                return Err(error::unsupported);
            }
            function.parameter_names[function.parameter_count] = name;
            function.parameters[function.parameter_count] = value_kind(trim(kind))?;
            function.parameter_count += 1;
        }
    }
    Ok(function)
}
fn string_constant(value: &str) -> Result<&str, error> {
    let escaped = value
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or(error::syntax)?;
    let mut chars = escaped.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Err(error::syntax),
            '\\' => match chars.next() {
                Some('"' | '\\' | 'n' | 'r' | 't' | 'v' | 'f' | 'b') => {}
                _ => return Err(error::unsupported),
            },
            ch if ch.is_control() => return Err(error::syntax),
            _ => {}
        }
    }
    Ok(escaped)
}
fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn get16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}
fn get32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn get64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod scalar_tests;
