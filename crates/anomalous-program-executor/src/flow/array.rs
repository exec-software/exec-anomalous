//! Typed arrays and indexed buffers in a fixed arena. A handle names a
//! descriptor rather than an offset, so aliases survive array growth.

use super::{error, kind, maximum_array_elements, maximum_arrays, value, value_kind};

#[derive(Clone, Copy)]
pub(super) struct array_handle {
    descriptor: usize,
    storage: storage_kind,
}

impl array_handle {
    pub(super) fn kind(self) -> kind {
        match self.storage {
            storage_kind::integer_array => kind::array_integer,
            storage_kind::text_array => kind::array_text,
            storage_kind::integer_buffer => kind::buffer_integer,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum storage_kind {
    integer_array,
    text_array,
    integer_buffer,
}

#[derive(Clone, Copy)]
struct descriptor {
    start: usize,
    length: usize,
    storage: storage_kind,
}

impl descriptor {
    fn address(self, index: i64) -> Option<usize> {
        let index = usize::try_from(index).ok()?;
        (index < self.length).then(|| self.start + index)
    }
}

pub(super) struct array_arena<'a> {
    descriptors: [Option<descriptor>; maximum_arrays],
    elements: [value<'a>; maximum_array_elements],
    used: usize,
    count: usize,
}

impl<'a> array_arena<'a> {
    pub(super) const fn new() -> Self {
        Self {
            descriptors: [None; maximum_arrays],
            elements: [value::integer(0); maximum_array_elements],
            used: 0,
            count: 0,
        }
    }

    pub(super) fn clear(&mut self) {
        self.descriptors.fill(None);
        self.elements.fill(value::integer(0));
        self.used = 0;
        self.count = 0;
    }

    pub(super) fn allocate(
        &mut self,
        items: &[value<'a>],
        element: kind,
    ) -> Result<array_handle, error> {
        let storage = match element {
            kind::integer => storage_kind::integer_array,
            kind::text => storage_kind::text_array,
            _ => return Err(error::invalid_state),
        };
        if items.iter().any(|item| value_kind(*item) != element) {
            return Err(error::invalid_state);
        }
        let start = self.used;
        let handle = self.reserve(items.len(), storage)?;
        self.elements[start..self.used].copy_from_slice(items);
        Ok(handle)
    }

    pub(super) fn allocate_buffer(&mut self, count: i64) -> Result<array_handle, error> {
        let length = usize::try_from(count.max(1)).map_err(|_| error::array_limit)?;
        let start = self.used;
        let handle = self.reserve(length, storage_kind::integer_buffer)?;
        self.elements[start..self.used].fill(value::integer(0));
        Ok(handle)
    }

    fn end_after(&self, charge: usize) -> Result<usize, error> {
        self.used
            .checked_add(charge)
            .filter(|end| *end <= maximum_array_elements)
            .ok_or(error::array_limit)
    }

    fn reserve(&mut self, length: usize, storage: storage_kind) -> Result<array_handle, error> {
        let end = self.end_after(length)?;
        if self.count == maximum_arrays {
            return Err(error::array_limit);
        }
        self.descriptors[self.count] = Some(descriptor {
            start: self.used,
            length,
            storage,
        });
        let handle = array_handle {
            descriptor: self.count,
            storage,
        };
        self.used = end;
        self.count += 1;
        Ok(handle)
    }

    fn descriptor(&self, handle: array_handle) -> Result<descriptor, error> {
        let slot = self
            .descriptors
            .get(..self.count)
            .and_then(|descriptors| descriptors.get(handle.descriptor))
            .ok_or(error::invalid_state)?;
        let result = slot.ok_or(match handle.storage {
            storage_kind::integer_buffer => error::buffer_released,
            _ => error::invalid_state,
        })?;
        if result.storage != handle.storage
            || result
                .start
                .checked_add(result.length)
                .is_none_or(|end| end > self.used)
        {
            return Err(error::invalid_state);
        }
        Ok(result)
    }

    pub(super) fn length(&self, handle: array_handle) -> Result<i64, error> {
        if handle.storage == storage_kind::integer_buffer {
            return Err(error::invalid_state);
        }
        i64::try_from(self.descriptor(handle)?.length).map_err(|_| error::invalid_state)
    }

    pub(super) fn get(&self, handle: array_handle, index: i64) -> Result<value<'a>, error> {
        let descriptor = self.descriptor(handle)?;
        if let Some(address) = descriptor.address(index) {
            return Ok(self.elements[address]);
        }
        // The pinned edition's default bounds mode returns the typed default.
        match descriptor.storage {
            storage_kind::integer_array | storage_kind::integer_buffer => Ok(value::integer(0)),
            storage_kind::text_array => Ok(value::text("")),
        }
    }

    pub(super) fn set(
        &mut self,
        handle: array_handle,
        index: i64,
        number: i64,
    ) -> Result<(), error> {
        if handle.storage == storage_kind::text_array {
            return Err(error::invalid_state);
        }
        if let Some(address) = self.descriptor(handle)?.address(index) {
            self.elements[address] = value::integer(number);
        }
        Ok(())
    }

    pub(super) fn append(&mut self, handle: array_handle, number: i64) -> Result<(), error> {
        if handle.storage != storage_kind::integer_array {
            return Err(error::invalid_state);
        }
        let mut descriptor = self.descriptor(handle)?;
        let old_end = descriptor.start + descriptor.length;
        let at_tail = old_end == self.used;
        let charge = if at_tail { 1 } else { descriptor.length + 1 };
        let end = self.end_after(charge)?;
        // Moving an extent charges every new cell. Keep the descriptor identity
        // and finish the capacity check before modifying either extent.
        if !at_tail {
            let old = descriptor.start..old_end;
            self.elements.copy_within(old.clone(), self.used);
            self.elements[old].fill(value::integer(0));
            descriptor.start = self.used;
        }
        self.elements[end - 1] = value::integer(number);
        descriptor.length += 1;
        self.descriptors[handle.descriptor] = Some(descriptor);
        self.used = end;
        Ok(())
    }

    pub(super) fn release(&mut self, handle: array_handle) -> Result<(), error> {
        if handle.storage != storage_kind::integer_buffer {
            return Err(error::invalid_state);
        }
        let descriptor = self.descriptor(handle)?;
        self.elements[descriptor.start..descriptor.start + descriptor.length]
            .fill(value::integer(0));
        // A tombstone revokes every alias. Keep the descriptor and cell
        // charges for the whole execution so stale handles cannot revive.
        self.descriptors[handle.descriptor] = None;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn is_clear(&self) -> bool {
        self.used == 0
            && self.count == 0
            && self.descriptors.iter().all(Option::is_none)
            && self
                .elements
                .iter()
                .all(|item| matches!(item, value::integer(0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(arena: &array_arena<'_>, handle: array_handle, index: i64) -> i64 {
        let value::integer(number) = arena.get(handle, index).unwrap() else {
            panic!("expected integer cell");
        };
        number
    }

    #[test]
    fn relocated_array_preserves_aliases_and_scrubs_old_cells() {
        let mut arena = array_arena::new();
        let first = arena
            .allocate(&[value::integer(17)], kind::integer)
            .unwrap();
        let alias = first;
        let neighbor = arena
            .allocate(&[value::integer(91)], kind::integer)
            .unwrap();
        arena.append(first, 23).unwrap();
        assert_eq!(arena.length(alias), Ok(2));
        assert_eq!(number(&arena, alias, 0), 17);
        assert_eq!(number(&arena, alias, 1), 23);
        assert_eq!(number(&arena, neighbor, 0), 91);
        assert!(matches!(arena.elements[0], value::integer(0)));
        assert_eq!(arena.used, 4);

        arena.set(alias, 1, 42).unwrap();
        arena.append(first, 9).unwrap();
        assert_eq!(arena.used, 5);
        assert_eq!(number(&arena, first, 1), 42);
        assert_eq!(number(&arena, first, 2), 9);
    }

    #[test]
    fn failed_growth_keeps_descriptor_values_and_capacity() {
        let mut arena = array_arena::new();
        let array = arena
            .allocate(&[value::integer(42), value::integer(73)], kind::integer)
            .unwrap();
        let buffer = arena.allocate_buffer(1020).unwrap();
        // There is room to append two tail cells, but moving this extent
        // needs three cells and cannot consume any of the remaining capacity.
        assert_eq!(arena.append(array, 11), Err(error::array_limit));
        assert_eq!(arena.used, 1022);
        assert_eq!(arena.length(array), Ok(2));
        assert_eq!(number(&arena, array, 1), 73);
        assert_eq!(number(&arena, buffer, 0), 0);
        let tail = arena.allocate(&[value::integer(7)], kind::integer).unwrap();
        assert_eq!(arena.append(tail, 8), Ok(()));
        assert_eq!(arena.append(tail, 9), Err(error::array_limit));
        assert_eq!(number(&arena, tail, 1), 8);
    }

    #[test]
    fn release_scrubs_and_revokes_aliases_without_refunding_capacity() {
        let mut arena = array_arena::new();
        let buffer = arena.allocate_buffer(1023).unwrap();
        let alias = buffer;
        arena.set(alias, 1022, 42).unwrap();
        arena.release(buffer).unwrap();
        assert!(matches!(arena.elements[1022], value::integer(0)));
        assert!(matches!(arena.get(alias, 0), Err(error::buffer_released)));
        assert_eq!(arena.set(alias, -1, 55), Err(error::buffer_released));
        assert_eq!(arena.release(alias), Err(error::buffer_released));
        assert!(matches!(arena.allocate_buffer(2), Err(error::array_limit)));

        let fresh = arena.allocate_buffer(0).unwrap();
        assert_ne!(fresh.descriptor, alias.descriptor);
        assert_eq!(number(&arena, fresh, 0), 0);
        arena.set(fresh, 0, 9).unwrap();
        arena.set(fresh, 1, 3).unwrap();
        assert_eq!(number(&arena, fresh, 0), 9);
        assert_eq!(number(&arena, fresh, 1), 0);
        arena.clear();
        assert!(arena.is_clear());
    }

    #[test]
    fn arrays_and_released_buffers_share_descriptor_budget() {
        let mut arena = array_arena::new();
        for _ in 0..maximum_arrays - 1 {
            let buffer = arena.allocate_buffer(i64::MIN).unwrap();
            arena.release(buffer).unwrap();
        }
        let empty = arena.allocate(&[], kind::integer).unwrap();
        assert!(matches!(arena.allocate_buffer(1), Err(error::array_limit)));
        assert!(matches!(
            arena.allocate(&[], kind::integer),
            Err(error::array_limit)
        ));
        arena.append(empty, 42).unwrap();
        assert_eq!(number(&arena, empty, 0), 42);
    }
}
