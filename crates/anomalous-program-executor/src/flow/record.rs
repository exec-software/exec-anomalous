//! Shared nominal records. Handles alias mutable i64 cells for one execution;
//! returning from a function does not reclaim or duplicate a record.

use super::{error, maximum_record_fields, maximum_records, program};

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

#[derive(Clone, Copy)]
struct descriptor {
    start: usize,
    length: usize,
    layout: u8,
}

pub(super) struct record_arena {
    descriptors: [Option<descriptor>; maximum_records],
    fields: [i64; maximum_record_fields],
    used: usize,
    count: usize,
}

impl record_arena {
    pub(super) const fn new() -> Self {
        Self {
            descriptors: [None; maximum_records],
            fields: [0; maximum_record_fields],
            used: 0,
            count: 0,
        }
    }

    pub(super) fn clear(&mut self) {
        self.descriptors.fill(None);
        self.fields.fill(0);
        self.used = 0;
        self.count = 0;
    }

    pub(super) fn allocate(
        &mut self,
        program: &program<'_>,
        layout: u8,
    ) -> Result<record_handle, error> {
        let length = program
            .record_layouts()
            .get(usize::from(layout))
            .ok_or(error::invalid_state)?
            .field_ids()
            .len();
        let end = self.used.checked_add(length).ok_or(error::record_limit)?;
        if self.count == maximum_records || end > maximum_record_fields {
            return Err(error::record_limit);
        }
        // Validate layout and both capacities before committing any storage.
        // Missing i64 fields in the pinned VM read as zero.
        self.fields[self.used..end].fill(0);
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

    fn address(
        &self,
        program: &program<'_>,
        handle: record_handle,
        field: u8,
    ) -> Result<usize, error> {
        let descriptor = self
            .descriptors
            .get(..self.count)
            .and_then(|items| items.get(handle.descriptor))
            .and_then(|item| *item)
            .ok_or(error::invalid_state)?;
        let layout = program
            .record_layouts()
            .get(usize::from(handle.layout))
            .ok_or(error::invalid_state)?;
        if descriptor.layout != handle.layout
            || descriptor.length != layout.field_ids().len()
            || descriptor
                .start
                .checked_add(descriptor.length)
                .is_none_or(|end| end > self.used)
        {
            return Err(error::invalid_state);
        }
        let offset = layout
            .field_ids()
            .iter()
            .position(|declared| *declared == field)
            .ok_or(error::invalid_state)?;
        Ok(descriptor.start + offset)
    }

    pub(super) fn get(
        &self,
        program: &program<'_>,
        handle: record_handle,
        field: u8,
    ) -> Result<i64, error> {
        Ok(self.fields[self.address(program, handle, field)?])
    }

    pub(super) fn set(
        &mut self,
        program: &program<'_>,
        handle: record_handle,
        field: u8,
        number: i64,
    ) -> Result<(), error> {
        let address = self.address(program, handle, field)?;
        self.fields[address] = number;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn is_clear(&self) -> bool {
        self.used == 0
            && self.count == 0
            && self.descriptors.iter().all(Option::is_none)
            && self.fields.iter().all(|number| *number == 0)
    }
}
