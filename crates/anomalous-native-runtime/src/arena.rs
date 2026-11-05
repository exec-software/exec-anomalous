use core::mem;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum arena_error {
    zero_length,
    invalid_alignment,
    address_overflow,
    exhausted,
}

/// Every successful allocation is disjoint and remains live for the original
/// storage borrow. Exhaustion cannot invalidate any earlier allocation.
pub struct arena<'a> {
    remaining: &'a mut [u8],
    used: usize,
    capacity: usize,
}

impl<'a> arena<'a> {
    pub fn new(storage: &'a mut [u8]) -> Self {
        let capacity = storage.len();
        Self {
            remaining: storage,
            used: 0,
            capacity,
        }
    }

    #[must_use]
    pub const fn used(&self) -> usize {
        self.used
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub fn remaining(&self) -> usize {
        self.remaining.len()
    }

    /// Align the actual address, not merely an offset within an assumed-aligned
    /// slice. Validate every calculation before consuming any storage. Returned
    /// bytes are initialized as supplied by the caller; this does not zero them.
    pub fn allocate(
        &mut self,
        length: usize,
        alignment: usize,
    ) -> Result<&'a mut [u8], arena_error> {
        if length == 0 {
            return Err(arena_error::zero_length);
        }
        if !alignment.is_power_of_two() {
            return Err(arena_error::invalid_alignment);
        }
        let address = self.remaining.as_ptr() as usize;
        let aligned = address
            .checked_add(alignment - 1)
            .ok_or(arena_error::address_overflow)?
            & !(alignment - 1);
        let padding = aligned - address;
        let consumed = padding
            .checked_add(length)
            .ok_or(arena_error::address_overflow)?;
        aligned
            .checked_add(length)
            .ok_or(arena_error::address_overflow)?;
        if consumed > self.remaining.len() {
            return Err(arena_error::exhausted);
        }
        let used = self
            .used
            .checked_add(consumed)
            .ok_or(arena_error::address_overflow)?;
        let (_, tail) = mem::take(&mut self.remaining).split_at_mut(padding);
        let (allocation, remaining) = tail.split_at_mut(length);
        self.remaining = remaining;
        self.used = used;
        Ok(allocation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_actual_unaligned_storage_and_keeps_live_allocations_disjoint() {
        let mut storage = [0xa5; 257];
        let mut allocator = arena::new(&mut storage[1..]);
        let first = allocator.allocate(17, 32).unwrap();
        let second = allocator.allocate(33, 64).unwrap();
        assert_eq!(first.as_ptr() as usize % 32, 0);
        assert_eq!(second.as_ptr() as usize % 64, 0);
        assert!(first.iter().all(|byte| *byte == 0xa5));
        first.fill(0x31);
        second.fill(0x52);
        assert!(first.iter().all(|byte| *byte == 0x31));
        assert!(second.iter().all(|byte| *byte == 0x52));
        assert!(first.as_ptr() as usize + first.len() <= second.as_ptr() as usize);
        assert_eq!(
            allocator.used() + allocator.remaining(),
            allocator.capacity()
        );
    }

    #[test]
    fn rejected_requests_preserve_cursor_storage_and_existing_allocations() {
        let mut storage = [0x6a; 96];
        let mut allocator = arena::new(&mut storage);
        let live = allocator.allocate(8, 1).unwrap();
        live.fill(0x19);
        let used = allocator.used();
        for (size, alignment, expected) in [
            (0, 1, arena_error::zero_length),
            (1, 0, arena_error::invalid_alignment),
            (1, 3, arena_error::invalid_alignment),
            (97, 1, arena_error::exhausted),
            (usize::MAX, 1, arena_error::address_overflow),
        ] {
            assert_eq!(allocator.allocate(size, alignment).unwrap_err(), expected);
            assert_eq!(allocator.used(), used);
            assert_eq!(allocator.remaining(), 88);
            assert_eq!(live, &[0x19; 8]);
        }
        let rest = allocator.allocate(88, 1).unwrap();
        assert_eq!(rest, &[0x6a; 88]);
        assert_eq!(allocator.allocate(1, 1), Err(arena_error::exhausted));
        assert_eq!(allocator.used(), 96);
        assert_eq!(live, &[0x19; 8]);
    }

    #[test]
    fn padding_costs_capacity_and_exhaustion_never_advances() {
        let mut storage = [0u8; 64];
        let mut allocator = arena::new(&mut storage[1..]);
        let first = allocator.allocate(1, 1).unwrap();
        first[0] = 7;
        let before = allocator.used();
        assert_eq!(allocator.allocate(64, 64), Err(arena_error::exhausted));
        assert_eq!(allocator.used(), before);
        let remaining = allocator.remaining();
        assert_eq!(allocator.allocate(remaining, 1).unwrap().len(), remaining);
        assert_eq!(first, &[7]);
        assert_eq!(allocator.remaining(), 0);
    }
}
