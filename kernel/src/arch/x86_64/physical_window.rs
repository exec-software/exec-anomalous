#![cfg_attr(not(test), allow(dead_code))]
#![forbid(unsafe_code)]

//! Inert geometry for a proposed supervisor physical-memory window.
//!
//! Nothing here installs a mapping, grants access, or establishes that physical
//! memory exists. A later owner must validate the exact page-table hierarchy,
//! effective permissions, hardware address width, physical availability and
//! ownership, collisions with resident kernel ranges, and safe activation.
//! Cacheability and suitability for RAM versus MMIO also require independent
//! validation; arithmetic cannot choose an access or caching policy.
//! In particular, a geometrically representable window may contain firmware
//! holes. Its extents are numbers, never pointers or physical-memory leases.

use super::paging::{self, page_address_mask, page_size};
use crate::memory::physical::physical_range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum physical_window_error {
    empty_extent,
    unaligned_physical_extent,
    unaligned_virtual_base,
    physical_extent_overflow,
    physical_address_out_of_range,
    virtual_extent_overflow,
    virtual_address_not_high_half,
    outside_physical_window,
    outside_virtual_window,
}

/// Checked high-half virtual geometry, with no mapping or access authority.
///
/// This is intentionally distinct from `physical_range`. Subpage extents are
/// permitted. The exclusive end must fit in `u64`, so this representation does
/// not describe an extent containing the final byte of the address space.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct planned_kernel_virtual_extent {
    base: u64,
    end: u64,
}

impl planned_kernel_virtual_extent {
    pub(super) fn new(base: u64, length: u64) -> Result<Self, physical_window_error> {
        if length == 0 {
            return Err(physical_window_error::empty_extent);
        }
        let end = base
            .checked_add(length)
            .ok_or(physical_window_error::virtual_extent_overflow)?;
        if !is_high_half(base) || !is_high_half(end - 1) {
            return Err(physical_window_error::virtual_address_not_high_half);
        }
        Ok(Self { base, end })
    }

    pub(super) const fn base(self) -> u64 {
        self.base
    }

    pub(super) const fn length(self) -> u64 {
        self.end - self.base
    }

    pub(super) const fn end(self) -> u64 {
        self.end
    }
}

/// A checked offset relation between physical and proposed kernel extents.
///
/// Construction validates only geometry. The caller chooses the virtual base;
/// no direct-map placement, stable ABI, permissions, or installed window is
/// selected here. The architectural page-entry address mask is an upper bound,
/// not evidence of a processor's supported width or usable physical storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct planned_physical_window {
    physical: physical_range,
    physical_end: u64,
    virtual_extent: planned_kernel_virtual_extent,
}

impl planned_physical_window {
    pub(super) fn new(
        physical: physical_range,
        virtual_base: u64,
    ) -> Result<Self, physical_window_error> {
        if physical.length == 0 {
            return Err(physical_window_error::empty_extent);
        }
        if !physical.base.is_multiple_of(page_size) || !physical.length.is_multiple_of(page_size) {
            return Err(physical_window_error::unaligned_physical_extent);
        }
        let physical_end = physical
            .end()
            .ok_or(physical_window_error::physical_extent_overflow)?;
        let physical_byte_mask = page_address_mask | (page_size - 1);
        if physical.base & !physical_byte_mask != 0 || (physical_end - 1) & !physical_byte_mask != 0
        {
            return Err(physical_window_error::physical_address_out_of_range);
        }
        if !virtual_base.is_multiple_of(page_size) {
            return Err(physical_window_error::unaligned_virtual_base);
        }
        let virtual_extent = planned_kernel_virtual_extent::new(virtual_base, physical.length)?;
        Ok(Self {
            physical,
            physical_end,
            virtual_extent,
        })
    }

    pub(super) const fn physical_extent(self) -> physical_range {
        self.physical
    }

    pub(super) const fn virtual_extent(self) -> planned_kernel_virtual_extent {
        self.virtual_extent
    }

    /// Calculate the proposed alias of a contained physical extent.
    ///
    /// Success is not permission to dereference the resulting virtual address.
    pub(super) fn plan_virtual_extent(
        self,
        physical: physical_range,
    ) -> Result<planned_kernel_virtual_extent, physical_window_error> {
        if physical.length == 0 {
            return Err(physical_window_error::empty_extent);
        }
        let end = physical
            .end()
            .ok_or(physical_window_error::physical_extent_overflow)?;
        if physical.base < self.physical.base || end > self.physical_end {
            return Err(physical_window_error::outside_physical_window);
        }
        let offset = physical.base - self.physical.base;
        let base = self
            .virtual_extent
            .base()
            .checked_add(offset)
            .ok_or(physical_window_error::virtual_extent_overflow)?;
        planned_kernel_virtual_extent::new(base, physical.length)
    }

    /// Calculate the physical geometry corresponding to a proposed alias.
    ///
    /// The result is not an allocator claim, availability check, or access grant.
    pub(super) fn plan_physical_extent(
        self,
        virtual_extent: planned_kernel_virtual_extent,
    ) -> Result<physical_range, physical_window_error> {
        if virtual_extent.base() < self.virtual_extent.base()
            || virtual_extent.end() > self.virtual_extent.end()
        {
            return Err(physical_window_error::outside_virtual_window);
        }
        let offset = virtual_extent.base() - self.virtual_extent.base();
        let base = self
            .physical
            .base
            .checked_add(offset)
            .ok_or(physical_window_error::physical_extent_overflow)?;
        Ok(physical_range::new(base, virtual_extent.length()))
    }
}

const fn is_high_half(address: u64) -> bool {
    address & (1 << 63) != 0 && paging::is_canonical(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test placements only; these do not choose an architectural direct-map base.
    const test_virtual_base: u64 = 0xffff_9000_0000_0000;
    const test_physical_base: u64 = 0x0020_0000;

    fn window() -> planned_physical_window {
        planned_physical_window::new(
            physical_range::new(test_physical_base, 4 * page_size),
            test_virtual_base,
        )
        .unwrap()
    }

    #[test]
    fn planned_geometry_keeps_physical_and_virtual_domains_distinct() {
        let window = window();
        assert_eq!(
            window.physical_extent(),
            physical_range::new(test_physical_base, 4 * page_size)
        );
        let virtual_extent = window.virtual_extent();
        assert_eq!(virtual_extent.base(), test_virtual_base);
        assert_eq!(virtual_extent.length(), 4 * page_size);
        assert_eq!(virtual_extent.end(), test_virtual_base + 4 * page_size);
        assert_eq!(
            window.plan_virtual_extent(window.physical_extent()),
            Ok(virtual_extent)
        );
        assert_eq!(
            window.plan_physical_extent(virtual_extent),
            Ok(window.physical_extent())
        );
        assert_eq!(
            window.plan_virtual_extent(physical_range::new(test_virtual_base, 1)),
            Err(physical_window_error::outside_physical_window)
        );
        assert_eq!(
            planned_kernel_virtual_extent::new(test_physical_base, 1),
            Err(physical_window_error::virtual_address_not_high_half)
        );
    }

    #[test]
    fn every_byte_round_trips_including_unaligned_page_edges() {
        let window = window();
        for offset in 0..window.physical_extent().length {
            let physical = physical_range::new(test_physical_base + offset, 1);
            let virtual_extent = window.plan_virtual_extent(physical).unwrap();
            assert_eq!(virtual_extent.base(), test_virtual_base + offset);
            assert_eq!(virtual_extent.length(), 1);
            assert_eq!(window.plan_physical_extent(virtual_extent), Ok(physical));
        }
        let crossing = physical_range::new(test_physical_base + page_size - 1, page_size + 2);
        let virtual_extent = window.plan_virtual_extent(crossing).unwrap();
        assert_eq!(virtual_extent.base(), test_virtual_base + page_size - 1);
        assert_eq!(virtual_extent.length(), page_size + 2);
        assert_eq!(window.plan_physical_extent(virtual_extent), Ok(crossing));
    }

    #[test]
    fn physical_extent_must_be_nonempty_aligned_and_representable() {
        for (physical, error) in [
            (
                physical_range::new(0, 0),
                physical_window_error::empty_extent,
            ),
            (
                physical_range::new(1, page_size),
                physical_window_error::unaligned_physical_extent,
            ),
            (
                physical_range::new(0, page_size - 1),
                physical_window_error::unaligned_physical_extent,
            ),
            (
                physical_range::new(u64::MAX - page_size + 1, page_size),
                physical_window_error::physical_extent_overflow,
            ),
            (
                physical_range::new(page_address_mask + page_size, page_size),
                physical_window_error::physical_address_out_of_range,
            ),
            (
                physical_range::new(page_address_mask, 2 * page_size),
                physical_window_error::physical_address_out_of_range,
            ),
        ] {
            assert_eq!(
                planned_physical_window::new(physical, test_virtual_base),
                Err(error)
            );
        }
    }

    #[test]
    fn physical_mask_endpoints_are_geometry_without_availability_claims() {
        for base in [0, page_address_mask] {
            let physical = physical_range::new(base, page_size);
            let window = planned_physical_window::new(physical, test_virtual_base).unwrap();
            let last_byte = physical_range::new(base + page_size - 1, 1);
            let virtual_extent = window.plan_virtual_extent(last_byte).unwrap();
            assert_eq!(virtual_extent.base(), test_virtual_base + page_size - 1);
            assert_eq!(window.plan_physical_extent(virtual_extent), Ok(last_byte));
        }
    }

    #[test]
    fn virtual_window_rejects_low_half_canonical_holes_and_alignment() {
        let physical = physical_range::new(0, page_size);
        for base in [
            0,
            0x0000_7fff_ffff_f000,
            0x0000_8000_0000_0000,
            0xffff_7fff_ffff_f000,
        ] {
            assert_eq!(
                planned_physical_window::new(physical, base),
                Err(physical_window_error::virtual_address_not_high_half)
            );
        }
        assert_eq!(
            planned_physical_window::new(physical, test_virtual_base + 1),
            Err(physical_window_error::unaligned_virtual_base)
        );
        let first_high_page =
            planned_physical_window::new(physical, 0xffff_8000_0000_0000).unwrap();
        assert_eq!(
            first_high_page.virtual_extent().base(),
            0xffff_8000_0000_0000
        );
        assert_eq!(
            planned_physical_window::new(physical, u64::MAX - page_size + 1),
            Err(physical_window_error::virtual_extent_overflow)
        );
    }

    #[test]
    fn typed_virtual_extents_reject_empty_overflow_and_hole_crossing() {
        for (base, length, error) in [
            (test_virtual_base, 0, physical_window_error::empty_extent),
            (u64::MAX, 1, physical_window_error::virtual_extent_overflow),
            (
                0xffff_7fff_ffff_ffff,
                2,
                physical_window_error::virtual_address_not_high_half,
            ),
            (
                0x0000_7fff_ffff_ffff,
                2,
                physical_window_error::virtual_address_not_high_half,
            ),
        ] {
            assert_eq!(planned_kernel_virtual_extent::new(base, length), Err(error));
        }
        let last_representable = planned_kernel_virtual_extent::new(u64::MAX - 1, 1).unwrap();
        assert_eq!(last_representable.end(), u64::MAX);
    }

    #[test]
    fn physical_translation_rejects_every_noncontained_extent() {
        let window = window();
        let end = window.physical_extent().end().unwrap();
        for physical in [
            physical_range::new(test_physical_base - 1, 1),
            physical_range::new(test_physical_base - 1, 2),
            physical_range::new(end, 1),
            physical_range::new(end - 1, 2),
            physical_range::new(test_physical_base - page_size, 6 * page_size),
        ] {
            assert_eq!(
                window.plan_virtual_extent(physical),
                Err(physical_window_error::outside_physical_window)
            );
        }
        assert_eq!(
            window.plan_virtual_extent(physical_range::new(test_physical_base, 0)),
            Err(physical_window_error::empty_extent)
        );
        assert_eq!(
            window.plan_virtual_extent(physical_range::new(u64::MAX, 1)),
            Err(physical_window_error::physical_extent_overflow)
        );
    }

    #[test]
    fn virtual_translation_rejects_noncontained_and_unrelated_extents() {
        let window = window();
        let end = window.virtual_extent().end();
        for (base, length) in [
            (test_virtual_base - 1, 1),
            (test_virtual_base - 1, 2),
            (end, 1),
            (end - 1, 2),
            (test_virtual_base - page_size, 6 * page_size),
        ] {
            let virtual_extent = planned_kernel_virtual_extent::new(base, length).unwrap();
            assert_eq!(
                window.plan_physical_extent(virtual_extent),
                Err(physical_window_error::outside_virtual_window)
            );
        }
        let unrelated = planned_physical_window::new(
            window.physical_extent(),
            test_virtual_base + 8 * page_size,
        )
        .unwrap();
        assert_eq!(
            window.plan_physical_extent(unrelated.virtual_extent()),
            Err(physical_window_error::outside_virtual_window)
        );
    }
}
