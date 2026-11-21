//! Shared nominal records. Handles alias typed fields for one execution;
//! returning from a function does not reclaim or duplicate a record.

use super::{error, kind, maximum_record_fields, maximum_records, program, value};

#[derive(Clone, Copy)]
pub(super) struct record_handle {
    descriptor: usize,
    layout: u8,
}

impl record_handle {
    pub(super) fn layout(self) -> u8 {
        self.layout
    }
}

/// Child references are handles, never embedded records. Forward references and
/// cycles need no recursive allocation, traversal, drop or reclamation.
#[derive(Clone, Copy)]
enum field_cell {
    integer(i64),
    record(record_handle),
    uninitialized,
}

#[derive(Clone, Copy)]
struct descriptor {
    start: usize,
    length: usize,
    layout: u8,
}

pub(super) struct record_arena {
    descriptors: [Option<descriptor>; maximum_records],
    fields: [field_cell; maximum_record_fields],
    used: usize,
    count: usize,
}

impl record_arena {
    pub(super) const fn new() -> Self {
        Self {
            descriptors: [None; maximum_records],
            fields: [field_cell::integer(0); maximum_record_fields],
            used: 0,
            count: 0,
        }
    }

    pub(super) fn clear(&mut self) {
        self.descriptors.fill(None);
        self.fields.fill(field_cell::integer(0));
        self.used = 0;
        self.count = 0;
    }

    pub(super) fn allocate(
        &mut self,
        program: &program<'_>,
        layout: u8,
    ) -> Result<record_handle, error> {
        let declaration = program
            .record_layouts()
            .get(usize::from(layout))
            .ok_or(error::invalid_state)?;
        let length = declaration.field_ids().len();
        if declaration.field_kinds().len() != length
            || declaration.field_kinds().iter().any(|field| match field {
                kind::integer => false,
                kind::record(child) => program.record_layouts().get(usize::from(*child)).is_none(),
                _ => true,
            })
        {
            return Err(error::invalid_state);
        }
        let end = self.used.checked_add(length).ok_or(error::record_limit)?;
        if self.count == maximum_records || end > maximum_record_fields {
            return Err(error::record_limit);
        }
        // Preflight the whole layout and both capacities before publication.
        // Missing i64 fields still read as zero. No integer sentinel can become
        // a nested handle, and no child is recursively allocated here.
        for (slot, field) in self.fields[self.used..end]
            .iter_mut()
            .zip(declaration.field_kinds())
        {
            *slot = if *field == kind::integer {
                field_cell::integer(0)
            } else {
                field_cell::uninitialized
            };
        }
        self.descriptors[self.count] = Some(descriptor {
            start: self.used,
            length,
            layout,
        });
        let handle = record_handle {
            descriptor: self.count,
            layout,
        };
        self.used = end;
        self.count += 1;
        Ok(handle)
    }

    fn validate_handle(
        &self,
        program: &program<'_>,
        handle: record_handle,
    ) -> Result<descriptor, error> {
        let descriptor = self
            .descriptors
            .get(..self.count)
            .and_then(|items| items.get(handle.descriptor))
            .and_then(|item| *item)
            .ok_or(error::invalid_state)?;
        let declaration = program
            .record_layouts()
            .get(usize::from(handle.layout))
            .ok_or(error::invalid_state)?;
        if descriptor.layout != handle.layout
            || descriptor.length != declaration.field_ids().len()
            || descriptor
                .start
                .checked_add(descriptor.length)
                .is_none_or(|end| end > self.used)
        {
            return Err(error::invalid_state);
        }
        Ok(descriptor)
    }

    fn address(
        &self,
        program: &program<'_>,
        handle: record_handle,
        field: u8,
    ) -> Result<(usize, kind), error> {
        let descriptor = self.validate_handle(program, handle)?;
        let declaration = &program.record_layouts()[usize::from(handle.layout)];
        let offset = declaration
            .field_ids()
            .iter()
            .position(|declared| *declared == field)
            .ok_or(error::invalid_state)?;
        let expected = declaration.field_kind(field).ok_or(error::invalid_state)?;
        Ok((descriptor.start + offset, expected))
    }

    pub(super) fn get<'a>(
        &self,
        program: &program<'_>,
        handle: record_handle,
        field: u8,
    ) -> Result<value<'a>, error> {
        let (address, expected) = self.address(program, handle, field)?;
        match (self.fields[address], expected) {
            (field_cell::integer(number), kind::integer) => Ok(value::integer(number)),
            (field_cell::record(child), kind::record(layout)) if child.layout == layout => {
                self.validate_handle(program, child)?;
                Ok(value::record(child))
            }
            (field_cell::uninitialized, kind::record(_)) => Err(error::record_uninitialized),
            _ => Err(error::invalid_state),
        }
    }

    pub(super) fn set(
        &mut self,
        program: &program<'_>,
        handle: record_handle,
        field: u8,
        item: value<'_>,
    ) -> Result<(), error> {
        let (address, expected) = self.address(program, handle, field)?;
        let cell = match (item, expected) {
            (value::integer(number), kind::integer) => field_cell::integer(number),
            (value::record(child), kind::record(layout)) if child.layout == layout => {
                self.validate_handle(program, child)?;
                field_cell::record(child)
            }
            _ => return Err(error::invalid_state),
        };
        // Validate receiver, field, exact child layout and live child identity
        // before mutation. A copied handle preserves the same shared child.
        self.fields[address] = cell;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn is_clear(&self) -> bool {
        self.used == 0
            && self.count == 0
            && self.descriptors.iter().all(Option::is_none)
            && self
                .fields
                .iter()
                .all(|cell| matches!(cell, field_cell::integer(0)))
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use anomalous_program_flow::{nested_records, validation_workspace};
    use anomalous_program_manifest::maximum_image_bytes;
    use std::{boxed::Box, vec};

    #[test]
    fn invalid_children_and_receivers_cannot_mutate_a_nested_field() {
        std::thread::scope(|scope| {
            std::thread::Builder::new().stack_size(8 * 1024 * 1024).spawn_scoped(scope, || {
                let source = b"# NIL Textual Module\nmodule arena\n# Struct definitions\nstruct parent\n child: child\nend struct\nstruct child\n number: i64\nend struct\nstruct other\n number: i64\nend struct\n@ai_context(\"Check private handle defenses\")\n# declaration_line 1\nfunction main -> i64:\nPUSH_CONST 0\nRET\nend function\n";
                let mut validation = Box::new(validation_workspace::new());
                let mut image = vec![0; maximum_image_bytes];
                let size = nested_records::encode(source, &mut image, &mut validation).unwrap();
                let admitted = nested_records::admit(&image[..size], 0, &mut validation).unwrap();
                let program = admitted.program();
                let mut arena = Box::new(record_arena::new());
                let parent = arena.allocate(program, 0).unwrap();
                let child = arena.allocate(program, 1).unwrap();
                let other = arena.allocate(program, 2).unwrap();
                let child_field = program.record_layouts()[0].field_ids()[0];
                let number_field = program.record_layouts()[1].field_ids()[0];
                for invalid in [value::integer(0), value::record(other), value::record(record_handle { descriptor: maximum_records, layout: 1 }), value::record(record_handle { descriptor: child.descriptor, layout: 2 }), value::boolean(false)] {
                    assert_eq!(arena.set(program, parent, child_field, invalid), Err(error::invalid_state));
                    assert!(matches!(arena.get(program, parent, child_field), Err(error::record_uninitialized)));
                }
                arena.set(program, child, number_field, value::integer(42)).unwrap();
                arena.set(program, parent, child_field, value::record(child)).unwrap();
                let value::record(alias) = arena.get(program, parent, child_field).unwrap() else { panic!("missing child") };
                assert_eq!(alias.descriptor, child.descriptor);
                assert!(matches!(arena.get(program, alias, number_field), Ok(value::integer(42))));
                assert_eq!(arena.set(program, child, number_field, value::record(parent)), Err(error::invalid_state));
                assert!(matches!(arena.get(program, child, number_field), Ok(value::integer(42))));
                assert_eq!(arena.set(program, parent, u8::MAX, value::record(child)), Err(error::invalid_state));
                let invalid_owner = record_handle { descriptor: parent.descriptor, layout: 1 };
                assert_eq!(arena.set(program, invalid_owner, child_field, value::record(child)), Err(error::invalid_state));
                assert!(matches!(arena.get(program, invalid_owner, child_field), Err(error::invalid_state)));
                let before = (arena.count, arena.used);
                assert!(matches!(arena.allocate(program, u8::MAX), Err(error::invalid_state)));
                assert_eq!((arena.count, arena.used), before);
                arena.clear();
                assert!(arena.is_clear());
                assert!(matches!(arena.get(program, parent, child_field), Err(error::invalid_state)));
            }).unwrap().join().unwrap();
        });
    }
}
