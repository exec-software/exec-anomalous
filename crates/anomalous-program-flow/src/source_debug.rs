//! Profile-10 descriptive metadata. No path is opened, and lexical type names
//! never participate in operand validation, service derivation or main intent.

use super::{admitted_program, error, parse_profile, validation_workspace};

pub const derivation_profile: u32 = 10;
pub const maximum_path_bytes: usize = 16384;
pub const maximum_binding_bytes: usize = 128;
pub const maximum_bindings: usize = 128;
pub const maximum_scope_depth: usize = 1024;

/// Validate completely before publishing any output bytes.
pub fn encode<'a>(
    edition: &'a [u8],
    output: &mut [u8],
    workspace: &mut validation_workspace<'a>,
) -> Result<usize, error> {
    super::encode_profile(edition, output, workspace, parse_profile::source_debug)
}

pub fn admit<'a, 'w>(
    image: &'a [u8],
    ceiling: u64,
    workspace: &'w mut validation_workspace<'a>,
) -> Result<admitted_program<'a, 'w>, error> {
    super::admit_profile(image, ceiling, workspace, parse_profile::source_debug)
}

/// A validated string borrowed from exact edition bytes. Only escaped quotes
/// and backslashes occur; decoding is explicit and needs no allocation.
#[derive(Clone, Copy, Debug)]
pub struct quoted_text<'a> {
    escaped: &'a str,
}

impl<'a> quoted_text<'a> {
    /// Encoded contents, excluding the enclosing double quotes.
    pub const fn escaped(self) -> &'a str {
        self.escaped
    }

    pub fn bytes(self) -> decoded_bytes<'a> {
        decoded_bytes {
            remaining: self.escaped.as_bytes(),
        }
    }

    pub fn decoded_len(self) -> usize {
        self.bytes().count()
    }

    /// A short destination is left completely unchanged.
    pub fn copy_to(self, output: &mut [u8]) -> Result<usize, error> {
        let length = self.decoded_len();
        if output.len() < length {
            return Err(error::output);
        }
        for (slot, byte) in output.iter_mut().zip(self.bytes()) {
            *slot = byte;
        }
        Ok(length)
    }
}

#[derive(Clone, Debug)]
pub struct decoded_bytes<'a> {
    remaining: &'a [u8],
}

impl Iterator for decoded_bytes<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        let (&first, rest) = self.remaining.split_first()?;
        self.remaining = rest;
        if first == b'\\' {
            // Construction is private and follows complete escape validation.
            let (&escaped, rest) = self.remaining.split_first()?;
            self.remaining = rest;
            Some(escaped)
        } else {
            Some(first)
        }
    }
}

impl core::iter::FusedIterator for decoded_bytes<'_> {}

#[derive(Clone, Copy, Debug)]
pub struct binding<'a> {
    pub name: quoted_text<'a>,
    pub storage: quoted_text<'a>,
    pub lexical_type: quoted_text<'a>,
    pub scope_depth: usize,
}

/// A validated per-instruction record. Bindings are descriptive snapshots and
/// need not name a declared local or an admitted operand type.
#[derive(Clone, Copy, Debug)]
pub struct record<'a> {
    instruction_index: usize,
    path: quoted_text<'a>,
    binding_count: usize,
    encoded_bindings: &'a str,
}

impl<'a> record<'a> {
    /// Zero-based index within the containing function, including label ops.
    pub const fn instruction_index(self) -> usize {
        self.instruction_index
    }

    pub const fn path(self) -> quoted_text<'a> {
        self.path
    }

    pub const fn binding_count(self) -> usize {
        self.binding_count
    }

    pub fn bindings(self) -> bindings<'a> {
        bindings {
            fields: fields {
                remaining: self.encoded_bindings,
            },
            remaining_count: self.binding_count,
        }
    }
}

#[derive(Clone, Debug)]
pub struct bindings<'a> {
    fields: fields<'a>,
    remaining_count: usize,
}

impl<'a> Iterator for bindings<'a> {
    type Item = binding<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining_count == 0 {
            return None;
        }
        // Every binding was checked before the immutable plan was published.
        let result = self.fields.binding().ok()?;
        self.remaining_count -= 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining_count, Some(self.remaining_count))
    }
}

impl ExactSizeIterator for bindings<'_> {}
impl core::iter::FusedIterator for bindings<'_> {}

pub(super) fn is_record(line: &str) -> bool {
    line.strip_prefix('#')
        .and_then(|contents| contents.split_ascii_whitespace().next())
        == Some("source_debug")
}

pub(super) fn validate(line: &str, expected_index: usize) -> Result<(), error> {
    let record = header(line)?;
    if record.instruction_index != expected_index {
        return Err(error::syntax);
    }
    let mut fields = fields {
        remaining: record.encoded_bindings,
    };
    let empty = quoted_text { escaped: "" };
    let mut names = [empty; maximum_bindings];
    let mut storage = [empty; maximum_bindings];
    for index in 0..record.binding_count {
        let binding = fields.binding()?;
        if names[..index]
            .iter()
            .any(|prior| prior.bytes().eq(binding.name.bytes()))
            || storage[..index]
                .iter()
                .any(|prior| prior.bytes().eq(binding.storage.bytes()))
        {
            return Err(error::syntax);
        }
        names[index] = binding.name;
        storage[index] = binding.storage;
    }
    if !super::trim(fields.remaining).is_empty() {
        return Err(error::syntax);
    }
    Ok(())
}

/// Read only the header of a completely checked stored record. Per-binding
/// views remain borrowed and decode their bounded strings on demand.
pub(super) fn view(line: &str) -> Option<record<'_>> {
    header(line).ok()
}

fn header(line: &str) -> Result<record<'_>, error> {
    let contents = super::trim(line.strip_prefix('#').ok_or(error::syntax)?);
    let mut fields = fields {
        remaining: contents.strip_prefix("source_debug").ok_or(error::syntax)?,
    };
    let instruction_index = fields.number()?;
    let path = fields.quoted(maximum_path_bytes)?;
    let binding_count = fields.number()?;
    if binding_count > maximum_bindings {
        return Err(error::limit);
    }
    Ok(record {
        instruction_index,
        path,
        binding_count,
        encoded_bindings: fields.remaining,
    })
}

#[derive(Clone, Debug)]
struct fields<'a> {
    remaining: &'a str,
}

impl<'a> fields<'a> {
    fn number(&mut self) -> Result<usize, error> {
        let remaining = super::trim(self.remaining);
        let end = remaining
            .bytes()
            .position(|byte| matches!(byte, b' ' | b'\t'))
            .unwrap_or(remaining.len());
        let value = super::unsigned(&remaining[..end])?;
        self.remaining = &remaining[end..];
        Ok(value)
    }

    fn quoted(&mut self, maximum: usize) -> Result<quoted_text<'a>, error> {
        let remaining = super::trim(self.remaining);
        if !remaining.starts_with('"') {
            return Err(error::syntax);
        }
        let bytes = remaining.as_bytes();
        let mut position = 1;
        let mut decoded = 0;
        while let Some(&byte) = bytes.get(position) {
            if byte == b'"' {
                if decoded == 0
                    || bytes
                        .get(position + 1)
                        .is_some_and(|byte| !matches!(byte, b' ' | b'\t'))
                {
                    return Err(error::syntax);
                }
                let value = quoted_text {
                    escaped: &remaining[1..position],
                };
                self.remaining = &remaining[position + 1..];
                return Ok(value);
            }
            if byte == b'\\' {
                position += 1;
                match bytes.get(position).ok_or(error::syntax)? {
                    b'"' | b'\\' => {}
                    b'n' | b'r' | b't' | b'v' | b'f' | b'0' => return Err(error::syntax),
                    _ => return Err(error::unsupported),
                }
            } else if byte < 0x20 || byte == 0x7f {
                return Err(error::syntax);
            }
            decoded += 1;
            if decoded > maximum {
                return Err(error::limit);
            }
            position += 1;
        }
        Err(error::syntax)
    }

    fn binding(&mut self) -> Result<binding<'a>, error> {
        let name = self.quoted(maximum_binding_bytes)?;
        let storage = self.quoted(maximum_binding_bytes)?;
        let lexical_type = self.quoted(maximum_binding_bytes)?;
        let scope_depth = self.number()?;
        if scope_depth > maximum_scope_depth {
            return Err(error::limit);
        }
        Ok(binding {
            name,
            storage,
            lexical_type,
            scope_depth,
        })
    }
}
