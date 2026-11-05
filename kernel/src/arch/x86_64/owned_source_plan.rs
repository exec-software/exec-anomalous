#![cfg_attr(not(test), allow(dead_code))]
#![forbid(unsafe_code)]

//! Inert native launch geometry borrowing one sealed, copied source owner.
//!
//! Offsets describe bytes in that owner, never uploader addresses or physical
//! aliases. Validation neither installs mappings nor grants execution authority.
//! Population must retain the reader and independently validate physical access.

use super::mapping_plan::{mapping_budget, mapping_plan, mapping_request, reserved_request};
use super::multi_region_plan::{launch_page_budget, launch_region_count, source_capacity};
use super::owned_source::sealed_source_reader;
use super::user_entry_plan::user_entry_plan;
use crate::task::executable_interval_set;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct source_span {
    pub destination: u64,
    pub source_offset: u64,
    pub length: u64,
}

pub(super) struct launch_input<'a> {
    pub regions: &'a [mapping_request],
    pub reservations: &'a [reserved_request],
    pub budget: mapping_budget,
    pub spans: &'a [source_span],
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

pub(super) struct owned_source_image<'a> {
    plan: mapping_plan,
    admission: user_entry_plan,
    reader: sealed_source_reader<'a>,
    spans: &'a [source_span],
    stack_region: usize,
}

impl<'a> owned_source_image<'a> {
    pub(super) fn new(
        input: launch_input<'a>,
        reader: sealed_source_reader<'a>,
    ) -> Result<Self, image_error> {
        let (plan, admission) = validate_input(&input, reader.length())?;
        Ok(Self {
            plan,
            admission,
            reader,
            spans: input.spans,
            stack_region: input.stack_region,
        })
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        mapping_plan,
        user_entry_plan,
        sealed_source_reader<'a>,
        &'a [source_span],
        usize,
    ) {
        (
            self.plan,
            self.admission,
            self.reader,
            self.spans,
            self.stack_region,
        )
    }
}

// Kept pure so hostile geometry tests need not fabricate sealed ownership.
// Only `new` combines these facts with an actual borrowed source reader.
pub(super) fn validate_input(
    input: &launch_input<'_>,
    source_length: u64,
) -> Result<(mapping_plan, user_entry_plan), image_error> {
    if input.regions.len() != launch_region_count
        || input.budget.data_pages > launch_page_budget
        || input.spans.len() > source_capacity
    {
        return Err(image_error::budget);
    }
    if source_length == 0 {
        return Err(image_error::source_extent);
    }
    let plan = mapping_plan::new(input.regions, input.reservations, input.budget)
        .map_err(|_| image_error::mapping)?;
    for (index, span) in input.spans.iter().enumerate() {
        let end = span
            .destination
            .checked_add(span.length)
            .filter(|end| *end > span.destination)
            .ok_or(image_error::source_extent)?;
        span.source_offset
            .checked_add(span.length)
            .filter(|end| *end <= source_length)
            .ok_or(image_error::source_extent)?;
        if !plan.regions().any(|region| {
            region.permissions().user
                && span.destination >= region.extent().start()
                && end <= region.extent().end()
        }) {
            return Err(image_error::source_extent);
        }
        // Earlier spans have already passed checked end arithmetic. Source
        // ranges may repeat: the sealed bytes are immutable. Destinations may not.
        if input.spans[..index].iter().any(|other| {
            span.destination < other.destination + other.length && other.destination < end
        }) {
            return Err(image_error::source_overlap);
        }
    }
    for interval in input.executable.intervals() {
        if !input.spans.iter().any(|span| {
            span.destination <= interval.start && span.destination + span.length >= interval.end
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
    Ok((plan, admission))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::x86_64::paging::{page_permissions, page_size};
    use crate::task::executable_interval;

    const stack_base: u64 = 0x7fff00000000;
    const source_length: u64 = 128;

    fn regions(stack_pages: u64) -> [mapping_request; 9] {
        [
            (0x40000000, 2, false, false),
            (0x40002000, 4, false, true),
            (0x40006000, 1, false, false),
            (0x40007000, 2, true, false),
            (0x60000000, 1, false, false),
            (0x60001000, 47, false, true),
            (0x60030000, 11, false, false),
            (0x6003b000, 4, true, false),
            (stack_base, stack_pages, true, false),
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

    fn guards(stack_pages: u64) -> [reserved_request; 3] {
        [
            (0, 512),
            (stack_base - page_size, 1),
            (stack_base + stack_pages * page_size, 1),
        ]
        .map(|(virtual_address, page_count)| reserved_request {
            virtual_address,
            page_count,
        })
    }

    fn spans() -> [source_span; 3] {
        [
            source_span {
                destination: 0x40002000,
                source_offset: 9,
                length: 16,
            },
            source_span {
                destination: 0x60001005,
                source_offset: 51,
                length: 16,
            },
            source_span {
                destination: 0x40007ffb,
                source_offset: 109,
                length: 19,
            },
        ]
    }

    fn input<'a>(
        regions: &'a [mapping_request],
        reservations: &'a [reserved_request],
        spans: &'a [source_span],
    ) -> launch_input<'a> {
        launch_input {
            regions,
            reservations,
            budget: mapping_budget {
                data_pages: 88,
                table_frames: 8,
            },
            spans,
            executable: executable_interval_set::new(&[
                executable_interval {
                    start: 0x40002003,
                    end: 0x4000200b,
                },
                executable_interval {
                    start: 0x60001007,
                    end: 0x60001015,
                },
            ])
            .unwrap(),
            entry: 0x60001007,
            stack_region: 8,
            stack_pointer: stack_base + 16 * page_size,
        }
    }

    fn error(input: &launch_input<'_>, length: u64) -> image_error {
        validate_input(input, length)
            .err()
            .expect("hostile metadata must reject")
    }

    #[test]
    fn modeled_sealed_reader_roundtrips_through_the_actual_launch_owner() {
        let bytes = core::array::from_fn::<_, 128, _>(|index| (index as u8) ^ 0x5a);
        let regions = regions(16);
        let guards = guards(16);
        let spans = spans();
        let (entry, stack_region, returned_spans) =
            super::super::owned_source::with_test_sealed_reader(&bytes, |reader| {
                let uploader = reader.uploader();
                let grant = reader.grant();
                let image = owned_source_image::new(input(&regions, &guards, &spans), reader)
                    .unwrap_or_else(|error| panic!("{error:?}"));
                let (plan, admission, reader, retained_spans, stack_region) = image.into_parts();
                assert_eq!(reader.length(), bytes.len() as u64);
                assert_eq!(reader.uploader(), uploader);
                assert_eq!(reader.grant(), grant);
                assert_eq!(plan.regions().count(), 9);
                assert_eq!(plan.data_pages(), 88);
                assert_eq!(retained_spans, &spans);
                let rsp = stack_base + 16 * page_size;
                assert!(admission.accepts_return(0x40002003, rsp));
                assert!(admission.accepts_return(0x60001014, rsp));
                assert!(!admission.accepts_return(0x50000000, rsp));
                assert!(!admission.accepts_return(0x60001015, rsp));
                // Only copied inert facts leave the callback, never a reader,
                // alias, physical lease or source-backed lifetime.
                (
                    admission.bounds().entry(),
                    stack_region,
                    retained_spans.len(),
                )
            });
        assert_eq!((entry, stack_region, returned_spans), (0x60001007, 8, 3));
        super::super::owned_source::with_test_sealed_reader(&bytes[..127], |reader| {
            assert_eq!(
                owned_source_image::new(input(&regions, &guards, &spans), reader).err(),
                Some(image_error::source_extent)
            );
        });
    }

    #[test]
    fn exact_source_offsets_keep_disjoint_code_and_high_stack_admission() {
        let mut regions = regions(16);
        regions.reverse();
        let guards = guards(16);
        let spans = spans();
        let (plan, admission) =
            validate_input(&input(&regions, &guards, &spans), source_length).unwrap();
        assert_eq!(plan.regions().count(), 9);
        assert_eq!(plan.data_pages(), 88);
        assert_eq!(plan.tables().len(), 8);
        assert_eq!(admission.bounds().entry(), 0x60001007);
        assert_eq!(spans[2].source_offset + spans[2].length, source_length);
        let rsp = stack_base + 16 * page_size;
        for rip in [0x40002003, 0x4000200a, 0x60001007, 0x60001014] {
            assert!(admission.accepts_return(rip, rsp));
        }
        for rip in [0x40002002, 0x4000200b, 0x50000000, 0x60001006, 0x60001015] {
            assert!(!admission.accepts_return(rip, rsp));
        }
    }

    #[test]
    fn source_length_and_both_end_arithmetics_are_checked_before_use() {
        let regions = regions(16);
        let guards = guards(16);
        let original = spans();
        for length in [0, 127] {
            assert_eq!(
                error(&input(&regions, &guards, &original), length),
                image_error::source_extent
            );
        }
        for (destination, source_offset, length) in [
            (0x40007ffb, 109, 0),
            (0x40007ffb, 128, 1),
            (0x40007ffb, u64::MAX - 3, 19),
            (u64::MAX - 3, 109, 19),
            (0x40007ffb, 0, u64::MAX),
        ] {
            let mut candidate = original;
            candidate[2] = source_span {
                destination,
                source_offset,
                length,
            };
            assert_eq!(
                error(&input(&regions, &guards, &candidate), source_length),
                image_error::source_extent
            );
        }
        let mut reuse = original;
        reuse[1].source_offset = reuse[0].source_offset;
        assert!(validate_input(&input(&regions, &guards, &reuse), source_length).is_ok());
        assert_eq!(original[2].source_offset, 109);
    }

    #[test]
    fn destination_must_fit_one_region_and_never_overlap_another_span() {
        let regions = regions(16);
        let guards = guards(16);
        for destination in [
            0,
            0x50000000,
            0x40001ffb,
            0x40006ffb,
            0x40008ffb,
            stack_base + 16 * page_size,
        ] {
            let mut candidate = spans();
            candidate[2].destination = destination;
            assert_eq!(
                error(&input(&regions, &guards, &candidate), source_length),
                image_error::source_extent
            );
        }
        for destination in [0x40002000, 0x40002001, 0x4000200f] {
            let mut candidate = spans();
            candidate[2].destination = destination;
            assert_eq!(
                error(&input(&regions, &guards, &candidate), source_length),
                image_error::source_overlap
            );
            candidate.reverse();
            assert_eq!(
                error(&input(&regions, &guards, &candidate), source_length),
                image_error::source_overlap
            );
        }
        let mut adjacent = spans();
        adjacent[2].destination = 0x40002010;
        assert!(validate_input(&input(&regions, &guards, &adjacent), source_length).is_ok());
    }

    #[test]
    fn initialized_coverage_cannot_merge_spans_or_turn_data_into_code() {
        let regions = regions(16);
        let guards = guards(16);
        let original = spans();
        assert_eq!(
            error(&input(&regions, &guards, &[]), source_length),
            image_error::uninitialized_code
        );
        let mut short = original;
        short[0].length = 10;
        assert_eq!(
            error(&input(&regions, &guards, &short), source_length),
            image_error::uninitialized_code
        );
        let split = [
            source_span {
                length: 8,
                ..original[0]
            },
            source_span {
                destination: 0x40002008,
                source_offset: 17,
                length: 8,
            },
            original[1],
        ];
        assert_eq!(
            error(&input(&regions, &guards, &split), source_length),
            image_error::uninitialized_code
        );
        let mut data_code = input(&regions, &guards, &original);
        data_code.executable = executable_interval_set::new(&[
            executable_interval {
                start: 0x40002003,
                end: 0x4000200b,
            },
            executable_interval {
                start: 0x40007ffb,
                end: 0x4000800e,
            },
        ])
        .unwrap();
        data_code.entry = 0x40007ffb;
        assert_eq!(error(&data_code, source_length), image_error::admission);
    }

    #[test]
    fn entry_stack_permissions_and_guards_remain_independent_of_sealing() {
        let regions = regions(16);
        let guards = guards(16);
        let spans = spans();
        for entry in [0x40002002, 0x4000200b, 0x40007ffb, 0x50000000, 0x60001015] {
            let mut candidate = input(&regions, &guards, &spans);
            candidate.entry = entry;
            assert_eq!(error(&candidate, source_length), image_error::admission);
        }
        for stack_region in [0, 1, 9] {
            let mut candidate = input(&regions, &guards, &spans);
            candidate.stack_region = stack_region;
            assert_eq!(error(&candidate, source_length), image_error::admission);
        }
        for stack_pointer in [
            stack_base + 240,
            stack_base + 257,
            stack_base + 16 * page_size + 16,
        ] {
            let mut candidate = input(&regions, &guards, &spans);
            candidate.stack_pointer = stack_pointer;
            assert_eq!(error(&candidate, source_length), image_error::admission);
        }
        assert_eq!(
            error(&input(&regions, &guards[..2], &spans), source_length),
            image_error::admission
        );
        assert_eq!(
            error(&input(&regions, &guards[2..], &spans), source_length),
            image_error::admission
        );
        let mut writable_code = regions;
        writable_code[1].permissions.writable = true;
        assert_eq!(
            error(&input(&writable_code, &guards, &spans), source_length),
            image_error::mapping
        );
    }

    #[test]
    fn region_page_table_and_span_operation_budgets_remain_bounded() {
        let regions = regions(16);
        let guards = guards(16);
        let spans = spans();
        assert_eq!(
            error(&input(&regions[..8], &guards, &spans), source_length),
            image_error::budget
        );
        let ten = core::array::from_fn::<_, 10, _>(|index| regions[index.min(8)]);
        assert_eq!(
            error(&input(&ten, &guards, &spans), source_length),
            image_error::budget
        );
        for (pages, expected) in [
            (0, image_error::mapping),
            (87, image_error::mapping),
            (129, image_error::budget),
        ] {
            let mut candidate = input(&regions, &guards, &spans);
            candidate.budget.data_pages = pages;
            assert_eq!(error(&candidate, source_length), expected);
        }
        for tables in [0, 7, 33] {
            let mut candidate = input(&regions, &guards, &spans);
            candidate.budget.table_frames = tables;
            assert_eq!(error(&candidate, source_length), image_error::mapping);
        }
        let full_regions = self::regions(56);
        let full_guards = self::guards(56);
        let mut full = input(&full_regions, &full_guards, &spans);
        full.budget.data_pages = 128;
        full.budget.table_frames = 32;
        full.stack_pointer = stack_base + 56 * page_size;
        assert!(validate_input(&full, source_length).is_ok());
        let overflow_regions = self::regions(57);
        let overflow_guards = self::guards(57);
        let mut overflow = input(&overflow_regions, &overflow_guards, &spans);
        overflow.budget.data_pages = 128;
        overflow.stack_pointer = stack_base + 57 * page_size;
        assert_eq!(error(&overflow, source_length), image_error::mapping);
        let mut many = core::array::from_fn::<_, 17, _>(|index| source_span {
            destination: 0x40007000 + index as u64,
            source_offset: 0,
            length: 1,
        });
        many[..3].copy_from_slice(&spans);
        assert!(validate_input(&input(&regions, &guards, &many[..16]), source_length).is_ok());
        assert_eq!(
            error(&input(&regions, &guards, &many), source_length),
            image_error::budget
        );
    }
}
