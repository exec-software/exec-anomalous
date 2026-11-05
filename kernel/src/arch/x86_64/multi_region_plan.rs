#![cfg_attr(not(test), allow(dead_code))]
#![forbid(unsafe_code)]

//! Native launch descriptions, with no executable-format or foreign-ABI policy.
//! A plan owns no frames. Sources stay immutably borrowed through population.

use super::mapping_plan::{mapping_budget, mapping_plan, mapping_request, reserved_request};
use super::user_entry_plan::user_entry_plan;
use crate::task::executable_interval_set;

pub(super) const launch_region_count: usize = 9;
pub(super) const launch_page_budget: u64 = 128;
pub(super) const source_capacity: usize = 16;

#[derive(Clone, Copy, Debug)]
pub(super) struct source_span<'a> {
    pub destination: u64,
    pub bytes: &'a [u8],
}

pub(super) struct launch_input<'a> {
    pub regions: &'a [mapping_request],
    pub reservations: &'a [reserved_request],
    pub budget: mapping_budget,
    pub sources: &'a [source_span<'a>],
    pub executable: executable_interval_set,
    pub entry: u64,
    pub stack_region: usize,
    pub stack_pointer: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum image_error {
    budget,
    mapping,
    source_extent,
    source_overlap,
    uninitialized_code,
    admission,
}

pub(super) struct multi_region_image<'a> {
    plan: mapping_plan,
    admission: user_entry_plan,
    sources: &'a [source_span<'a>],
    stack_region: usize,
}

impl<'a> multi_region_image<'a> {
    pub(super) fn new(input: launch_input<'a>) -> Result<Self, image_error> {
        if input.regions.len() != launch_region_count
            || input.budget.data_pages > launch_page_budget
            || input.sources.len() > source_capacity
        {
            return Err(image_error::budget);
        }
        let plan = mapping_plan::new(input.regions, input.reservations, input.budget)
            .map_err(|_| image_error::mapping)?;
        for (index, source) in input.sources.iter().enumerate() {
            let end = source
                .destination
                .checked_add(source.bytes.len() as u64)
                .filter(|end| *end > source.destination)
                .ok_or(image_error::source_extent)?;
            if !plan.regions().any(|region| {
                source.destination >= region.extent().start() && end <= region.extent().end()
            }) {
                return Err(image_error::source_extent);
            }
            if input.sources[..index].iter().any(|other| {
                source.destination < other.destination + other.bytes.len() as u64
                    && other.destination < end
            }) {
                return Err(image_error::source_overlap);
            }
        }
        for interval in input.executable.intervals() {
            if !input.sources.iter().any(|source| {
                source.destination <= interval.start
                    && source.destination + source.bytes.len() as u64 >= interval.end
            }) {
                return Err(image_error::uninitialized_code);
            }
        }
        let admission = user_entry_plan::with_executable_intervals(
            &plan,
            input.executable,
            input.entry,
            input.stack_region,
            input.stack_pointer,
        )
        .map_err(|_| image_error::admission)?;
        Ok(Self {
            plan,
            admission,
            sources: input.sources,
            stack_region: input.stack_region,
        })
    }

    pub(super) fn into_parts(
        self,
    ) -> (mapping_plan, user_entry_plan, &'a [source_span<'a>], usize) {
        (self.plan, self.admission, self.sources, self.stack_region)
    }
}

/// Sources have already passed whole-span and nonoverlap checks. Return exact
/// initialization intent, including zero-filled holes, BSS and page slack.
pub(super) fn initialized_byte(sources: &[source_span<'_>], address: u64) -> u8 {
    sources
        .iter()
        .find_map(|source| {
            address
                .checked_sub(source.destination)
                .and_then(|offset| usize::try_from(offset).ok())
                .and_then(|offset| source.bytes.get(offset))
                .copied()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::x86_64::paging::{page_permissions, page_size};
    use crate::task::executable_interval;

    const stack_base: u64 = 0x7fff00000000;
    const stack_pages: u64 = 16;
    const main_bytes: [u8; 16] = *b"main-code-prefix";
    const interpreter_bytes: [u8; 16] = *b"interpreter-code";
    const data_bytes: [u8; 19] = *b"unaligned-data-span";

    fn regions(stack_page_count: u64) -> [mapping_request; 9] {
        [
            (0x40000000, 2, false, false),
            (0x40002000, 4, false, true),
            (0x40006000, 1, false, false),
            (0x40007000, 2, true, false),
            (0x60000000, 1, false, false),
            (0x60001000, 47, false, true),
            (0x60030000, 11, false, false),
            (0x6003b000, 4, true, false),
            (stack_base, stack_page_count, true, false),
        ]
        .map(
            |(virtual_address, page_count, writable, executable)| mapping_request {
                virtual_address,
                page_count,
                permissions: page_permissions {
                    user: true,
                    writable,
                    executable,
                },
            },
        )
    }

    fn guards(stack_page_count: u64) -> [reserved_request; 3] {
        [
            (0, 512),
            (stack_base - page_size, 1),
            (stack_base + stack_page_count * page_size, 1),
        ]
        .map(|(virtual_address, page_count)| reserved_request {
            virtual_address,
            page_count,
        })
    }

    fn sources() -> [source_span<'static>; 3] {
        [
            source_span {
                destination: 0x40002000,
                bytes: &main_bytes,
            },
            source_span {
                destination: 0x60001005,
                bytes: &interpreter_bytes,
            },
            source_span {
                destination: 0x40007ffb,
                bytes: &data_bytes,
            },
        ]
    }

    fn executable() -> executable_interval_set {
        executable_interval_set::new(&[
            executable_interval {
                start: 0x40002003,
                end: 0x4000200b,
            },
            executable_interval {
                start: 0x60001007,
                end: 0x60001015,
            },
        ])
        .unwrap()
    }

    fn input<'a>(
        regions: &'a [mapping_request],
        reservations: &'a [reserved_request],
        sources: &'a [source_span<'a>],
    ) -> launch_input<'a> {
        launch_input {
            regions,
            reservations,
            budget: mapping_budget {
                data_pages: 88,
                table_frames: 8,
            },
            sources,
            executable: executable(),
            entry: 0x60001007,
            stack_region: 8,
            stack_pointer: stack_base + stack_pages * page_size,
        }
    }

    fn error(input: launch_input<'_>) -> image_error {
        multi_region_image::new(input)
            .err()
            .expect("hostile launch must reject")
    }

    #[test]
    fn validated_image_retains_source_borrows_and_exact_executable_holes() {
        let mut regions = regions(stack_pages);
        regions.reverse(); // Admission indexes refer to canonical mapping order.
        let guards = guards(stack_pages);
        let borrowed_main = main_bytes;
        let borrowed_interpreter = interpreter_bytes;
        let mut sources = sources();
        sources[0].bytes = &borrowed_main;
        sources[1].bytes = &borrowed_interpreter;
        let image = multi_region_image::new(input(&regions, &guards, &sources)).unwrap();
        let (plan, admission, retained, stack_region) = image.into_parts();
        assert!(core::ptr::eq(retained, &sources[..]));
        assert!(core::ptr::eq(retained[0].bytes, &borrowed_main[..]));
        assert!(core::ptr::eq(retained[1].bytes, &borrowed_interpreter[..]));
        assert_eq!(borrowed_main, main_bytes);
        assert_eq!(borrowed_interpreter, interpreter_bytes);
        assert_eq!(plan.regions().count(), 9);
        assert_eq!(
            plan.regions()
                .map(|region| region.extent().page_count())
                .sum::<u64>(),
            88
        );
        assert_eq!(plan.tables().len(), 8);
        assert_eq!(stack_region, 8);
        assert_eq!(admission.bounds().entry(), 0x60001007);
        let rsp = stack_base + stack_pages * page_size;
        for rip in [0x40002003, 0x4000200a, 0x60001007, 0x60001014] {
            assert!(admission.accepts_return(rip, rsp));
        }
        for rip in [
            0x40002000, 0x40002002, 0x4000200b, 0x50000000, 0x60001005, 0x60001015,
        ] {
            assert!(!admission.accepts_return(rip, rsp));
        }
    }

    #[test]
    fn initialized_bytes_preserve_non_aligned_prefix_and_zero_bss_slack_and_gaps() {
        let regions = regions(stack_pages);
        let guards = guards(stack_pages);
        let sources = sources();
        let (_, _, retained, _) = multi_region_image::new(input(&regions, &guards, &sources))
            .unwrap()
            .into_parts();
        // Literal boundary checks are independent of source-span iteration.
        for (address, expected) in [
            (0x40002000, b'm'),
            (0x40002002, b'i'),
            (0x4000200f, b'x'),
            (0x60001005, b'i'),
            (0x60001014, b'e'),
            (0x40007ffb, b'u'),
            (0x40007fff, b'i'),
            (0x40008000, b'g'),
            (0x4000800d, b'n'),
        ] {
            assert_eq!(
                initialized_byte(retained, address),
                expected,
                "byte {address:x}"
            );
        }
        for address in 0x40007000..0x40007ffb {
            assert_eq!(initialized_byte(retained, address), 0);
        }
        for address in 0x4000800e..0x40009000 {
            assert_eq!(initialized_byte(retained, address), 0);
        }
        for address in [
            0,
            0x40002010,
            0x40005fff,
            0x40006000,
            0x50000000,
            0x60001004,
            0x60001015,
            0x6003e2df,
            stack_base,
            stack_base + stack_pages * page_size - 1,
            u64::MAX,
        ] {
            assert_eq!(initialized_byte(retained, address), 0, "zero {address:x}");
        }
    }

    #[test]
    fn source_extents_reject_overflow_unmapped_empty_and_adjacent_region_crossing() {
        let regions = regions(stack_pages);
        let guards = guards(stack_pages);
        for destination in [
            0,
            0x50000000,
            stack_base + stack_pages * page_size,
            u64::MAX - 3,
            0x40001ffb,
            0x40006ffb,
            0x40008ffb,
        ] {
            let mut sources = sources();
            sources[2].destination = destination;
            assert_eq!(
                error(input(&regions, &guards, &sources)),
                image_error::source_extent
            );
            assert_eq!(sources[2].bytes, &data_bytes);
        }
        let mut empty = sources();
        empty[2].bytes = &[];
        assert_eq!(
            error(input(&regions, &guards, &empty)),
            image_error::source_extent
        );
        assert_eq!(
            error(input(&regions, &guards, &[])),
            image_error::uninitialized_code
        );
        let mut boundary = sources();
        boundary[2].destination = 0x40009000 - data_bytes.len() as u64;
        assert!(multi_region_image::new(input(&regions, &guards, &boundary)).is_ok());
    }

    #[test]
    fn source_overlap_and_partial_code_coverage_cannot_claim_initialized_execution() {
        let regions = regions(stack_pages);
        let guards = guards(stack_pages);
        for destination in [0x40001fff, 0x40002000, 0x40002001, 0x4000200f] {
            let mut overlap = sources();
            overlap[2].destination = destination;
            // The first extent straddles R/RX and fails before overlap policy.
            let expected = if destination == 0x40001fff {
                image_error::source_extent
            } else {
                image_error::source_overlap
            };
            assert_eq!(error(input(&regions, &guards, &overlap)), expected);
            overlap.reverse();
            assert_eq!(error(input(&regions, &guards, &overlap)), expected);
        }
        let mut short = sources();
        short[0].bytes = &main_bytes[..10];
        assert_eq!(
            error(input(&regions, &guards, &short)),
            image_error::uninitialized_code
        );
        let split = [
            source_span {
                destination: 0x40002000,
                bytes: &main_bytes[..8],
            },
            source_span {
                destination: 0x40002008,
                bytes: &main_bytes[8..],
            },
            sources()[1],
        ];
        // The declared initialized interval must be covered by one admitted
        // source. Even adjacent partial claims do not implicitly merge.
        assert_eq!(
            error(input(&regions, &guards, &split)),
            image_error::uninitialized_code
        );
        let mut outside = input(&regions, &guards, &short);
        outside.executable = executable_interval_set::new(&[
            executable_interval {
                start: 0x40002003,
                end: 0x4000200a,
            },
            executable_interval {
                start: 0x40007ffb,
                end: 0x4000800e,
            },
        ])
        .unwrap();
        outside.entry = 0x40007ffb;
        // Fully initialized data still cannot become executable authority.
        assert_eq!(error(outside), image_error::admission);
    }

    #[test]
    fn entry_and_stack_require_exact_admission_not_only_copied_bytes() {
        let regions = regions(stack_pages);
        let guards = guards(stack_pages);
        let sources = sources();
        for entry in [
            0, 0x40002002, 0x4000200b, 0x40007ffb, 0x50000000, 0x60001015,
        ] {
            let mut candidate = input(&regions, &guards, &sources);
            candidate.entry = entry;
            assert_eq!(error(candidate), image_error::admission);
        }
        for stack_region in [0, 1, 9] {
            let mut candidate = input(&regions, &guards, &sources);
            candidate.stack_region = stack_region;
            assert_eq!(error(candidate), image_error::admission);
        }
        for stack_pointer in [
            stack_base + 240,
            stack_base + 257,
            stack_base + stack_pages * page_size + 16,
        ] {
            let mut candidate = input(&regions, &guards, &sources);
            candidate.stack_pointer = stack_pointer;
            assert_eq!(error(candidate), image_error::admission);
        }
        for missing in 1..=2 {
            let incomplete = [guards[0], guards[3 - missing]];
            assert_eq!(
                error(input(&regions, &incomplete, &sources)),
                image_error::admission
            );
        }
        let mut overlapping = regions;
        overlapping[2].virtual_address = overlapping[1].virtual_address;
        assert_eq!(
            error(input(&overlapping, &guards, &sources)),
            image_error::mapping
        );
    }

    #[test]
    fn region_page_table_and_source_budgets_are_exact() {
        let regions = regions(stack_pages);
        let guards = guards(stack_pages);
        let sources = sources();
        assert_eq!(
            error(input(&regions[..8], &guards, &sources)),
            image_error::budget
        );
        let ten = core::array::from_fn::<_, 10, _>(|index| regions[index.min(8)]);
        assert_eq!(error(input(&ten, &guards, &sources)), image_error::budget);
        for (pages, expected) in [
            (0, image_error::mapping),
            (87, image_error::mapping),
            (129, image_error::budget),
        ] {
            let mut candidate = input(&regions, &guards, &sources);
            candidate.budget.data_pages = pages;
            assert_eq!(error(candidate), expected);
        }
        for tables in [0, 7, 33] {
            let mut candidate = input(&regions, &guards, &sources);
            candidate.budget.table_frames = tables;
            assert_eq!(error(candidate), image_error::mapping);
        }
        let full_regions = self::regions(56);
        let full_guards = self::guards(56);
        let mut exact = input(&full_regions, &full_guards, &sources);
        exact.budget.data_pages = 128;
        exact.stack_pointer = stack_base + 56 * page_size;
        assert!(multi_region_image::new(exact).is_ok());
        let overflow_regions = self::regions(57);
        let overflow_guards = self::guards(57);
        let mut overflow = input(&overflow_regions, &overflow_guards, &sources);
        overflow.budget.data_pages = 128;
        overflow.stack_pointer = stack_base + 57 * page_size;
        assert_eq!(error(overflow), image_error::mapping);

        let mut spans = core::array::from_fn::<_, 17, _>(|index| source_span {
            destination: 0x40007000 + index as u64,
            bytes: &data_bytes[..1],
        });
        spans[..3].copy_from_slice(&sources);
        assert!(multi_region_image::new(input(&regions, &guards, &spans[..16])).is_ok());
        assert_eq!(error(input(&regions, &guards, &spans)), image_error::budget);
    }
}
