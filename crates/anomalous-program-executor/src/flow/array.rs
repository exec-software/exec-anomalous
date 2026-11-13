//! Immutable typed arrays. Descriptors and scalar elements belong to one run;
//! copying a handle never releases storage or duplicates an allocation.

use super::{error, kind, maximum_array_elements, maximum_arrays, value, value_kind};

#[derive(Clone, Copy)]
pub(super) struct array_handle {
    descriptor: usize,
    element: kind,
}

impl array_handle {
    pub(super) fn kind(self) -> kind {
        match self.element {
            kind::integer => kind::array_integer,
            kind::text => kind::array_text,
            // Construction is private and accepts only these two scalar kinds.
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
struct descriptor {
    start: usize,
    length: usize,
    element: kind,
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
        if !matches!(element, kind::integer | kind::text)
            || items.iter().any(|item| value_kind(*item) != element)
        {
            return Err(error::invalid_state);
        }
        let end = self
            .used
            .checked_add(items.len())
            .ok_or(error::array_limit)?;
        if self.count == maximum_arrays || end > maximum_array_elements {
            return Err(error::array_limit);
        }
        // Validate every operand and both capacities before committing anything.
        self.elements[self.used..end].copy_from_slice(items);
        self.descriptors[self.count] = Some(descriptor {
            start: self.used,
            length: items.len(),
            element,
        });
        let handle = array_handle {
            descriptor: self.count,
            element,
        };
        self.used = end;
        self.count += 1;
        Ok(handle)
    }

    fn descriptor(&self, handle: array_handle) -> Result<descriptor, error> {
        let result = self
            .descriptors
            .get(..self.count)
            .and_then(|descriptors| descriptors.get(handle.descriptor))
            .and_then(|item| *item)
            .ok_or(error::invalid_state)?;
        if result.element != handle.element
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
        i64::try_from(self.descriptor(handle)?.length).map_err(|_| error::invalid_state)
    }

    pub(super) fn get(&self, handle: array_handle, index: i64) -> Result<value<'a>, error> {
        let descriptor = self.descriptor(handle)?;
        if let Ok(index) = usize::try_from(index)
            && index < descriptor.length
        {
            return Ok(self.elements[descriptor.start + index]);
        }
        // The pinned edition's default bounds mode returns the typed default.
        match descriptor.element {
            kind::integer => Ok(value::integer(0)),
            kind::text => Ok(value::text("")),
            _ => Err(error::invalid_state),
        }
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
