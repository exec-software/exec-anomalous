//! A decoded view of already admitted bytes, never an alternate admission API.
//!
//! Construction requires the private admission witness. Instruction tokenization
//! is shared with whole-edition derivation; nothing here admits arbitrary text.

use crate::{admitted_edition, derive::instruction_fields, error};

pub use crate::derive::{
    maximum_constants, maximum_instructions, maximum_locals, maximum_stack_values,
};

#[derive(Debug, Eq, PartialEq)]
pub enum record<'a> {
    constant { name: &'a str, escaped: &'a str },
    function { name: &'a str },
    end_function,
    instruction(instruction<'a>),
}

#[derive(Debug, Eq, PartialEq)]
pub struct instruction<'a> {
    operands: [&'a str; 6],
    count: usize,
    pub line: usize,
}

impl<'a> instruction<'a> {
    pub fn fields(&self) -> &[&'a str] {
        &self.operands[..self.count]
    }
}

pub struct records<'a> {
    lines: core::iter::Enumerate<core::str::Split<'a, char>>,
    in_function: bool,
}

impl<'a> admitted_edition<'a> {
    pub fn execution_records(&self) -> records<'a> {
        // Admission has checked every byte of this immutable borrowed edition.
        records {
            lines: core::str::from_utf8(self.edition())
                .unwrap()
                .split('\n')
                .enumerate(),
            in_function: false,
        }
    }
}

impl<'a> Iterator for records<'a> {
    type Item = Result<record<'a>, error>;

    fn next(&mut self) -> Option<Self::Item> {
        for (line_number, raw) in self.lines.by_ref() {
            let line = raw.trim_matches([' ', '\t']);
            if line.is_empty() {
                continue;
            }
            if line == "end function" {
                self.in_function = false;
                return Some(Ok(record::end_function));
            }
            if self.in_function {
                return Some(instruction_fields(line).map(|(operands, count)| {
                    record::instruction(instruction {
                        operands,
                        count,
                        line: line_number + 1,
                    })
                }));
            }
            if let Some(header) = line.strip_prefix("function ") {
                self.in_function = true;
                return Some(
                    header
                        .split_once(" -> ")
                        .map(|(name, _)| record::function { name })
                        .ok_or(error::syntax),
                );
            }
            if let Some(constant) = line.strip_prefix("const ") {
                return Some(
                    constant
                        .split_once(" = ")
                        .and_then(|(name, value)| {
                            value
                                .strip_prefix('"')
                                .and_then(|value| value.strip_suffix('"'))
                                .map(|escaped| record::constant { name, escaped })
                        })
                        .ok_or(error::syntax),
                );
            }
            // Only admitted module, annotation and declaration records remain.
        }
        None
    }
}
