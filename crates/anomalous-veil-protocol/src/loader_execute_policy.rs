//! Pure userspace recognition of the next observed Linux libc mapping request.
//!
//! This recognizes exactly sequence 12's fixed read/execute subrange of the
//! earlier immutable source mapping. It returns inert native intent, without
//! a native call, permission mutation, Linux result or descriptor publication.
//! The embedding must authenticate the pending invocation and bind the actual
//! retained mapping/source to this snapshot; matching numbers or digests do not
//! establish those facts. The sequence-11 creation policy is unchanged.

use crate::file_service::{
    client_identity, descriptor_handle, file_service, file_snapshot, immutable_file_source,
};
use crate::invocation;
use crate::loader_mapping_policy::{first_mapping_sequence, proof_base};
use crate::source_mapping_execute::{
    source_mapping_execute_request, source_mapping_retained_context, user_read_execute,
};
use crate::source_read::source_binding;

pub const executable_mapping_sequence: u64 = 12;
pub const executable_mapping_base: u64 = 0x1_0002_8000;
pub const executable_mapping_length: u64 = 0x198000;
pub const executable_mapping_offset: u64 = 0x28000;

/// `retained_snapshot` is the snapshot retained from the sequence-11 mapping,
/// not a newly resolved namespace entry. Its association with the native owner
/// is an embedding obligation; this check compares it with the current handle's
/// actual provider binding, including snapshot identity and immutable contents.
pub fn executable_file_mapping<const capacity: usize, source: immutable_file_source + ?Sized>(
    request: invocation,
    binding: source_binding,
    retained: source_mapping_retained_context,
    service: &file_service<'_, capacity, source>,
    owner: client_identity,
    handle: descriptor_handle,
    retained_snapshot: file_snapshot<'_, source>,
) -> Option<source_mapping_execute_request> {
    // No narrowing of foreign register words and no general MAP_FIXED policy.
    // This proof can only replace permissions of its unchanged source range.
    if request.sequence() != executable_mapping_sequence
        || request.number() != 9
        || request.arguments()
            != [
                executable_mapping_base,
                executable_mapping_length,
                5,
                0x812,
                3,
                executable_mapping_offset,
            ]
        || retained.creation().sequence() != first_mapping_sequence
        || retained.creation().base() != proof_base
        || handle.fd() != 3
        || service.last_sequence() >= request.sequence()
        || service.handle_for_fd(owner, 3).ok() != Some(handle)
        || service.file(owner, handle).ok() != Some(retained_snapshot)
        || retained_snapshot.byte_length() != binding.length()
    {
        return None;
    }
    source_mapping_execute_request::decode(
        request.sequence(),
        binding,
        retained,
        [
            request.sequence(),
            retained.handle().generation(),
            retained.handle().incarnation(),
            binding.grant(),
            binding.length(),
            binding.fnv64(),
            executable_mapping_base,
            executable_mapping_length,
            executable_mapping_offset,
            user_read_execute,
            0,
            0,
        ],
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_service::{namespace, namespace_entry, publication_result, source_error};
    use crate::source_mapping::{source_mapping_request, user_read_only};
    use crate::source_mapping_lifetime::source_mapping_handle;
    use crate::source_read::source_grant;

    // Synthetic immutable provider avoids allocating a libc-sized test buffer.
    // Numeric identity here is fixture state, never evidence of a real source.
    #[derive(Debug, Eq, PartialEq)]
    struct fixture_source {
        identity: u64,
        length: u64,
    }

    impl immutable_file_source for fixture_source {
        type read_context = ();

        fn byte_length(&self) -> u64 {
            self.length
        }

        fn read_at(
            &self,
            _context: &mut (),
            offset: u64,
            destination: &mut [u8],
        ) -> Result<usize, source_error> {
            if offset
                .checked_add(destination.len() as u64)
                .is_none_or(|end| end > self.length)
            {
                return Err(source_error::invalid_extent);
            }
            destination.fill(self.identity as u8);
            Ok(destination.len())
        }
    }

    static source: fixture_source = fixture_source {
        identity: 1,
        length: 2_190_608,
    };
    static other_source: fixture_source = fixture_source {
        identity: 2,
        length: 2_190_608,
    };

    fn owner() -> client_identity {
        client_identity::new(7, 1).unwrap()
    }

    fn binding() -> source_binding {
        source_binding::new(source_grant, source.length, 0x1234).unwrap()
    }

    fn retained() -> source_mapping_retained_context {
        let creation = source_mapping_request::decode(
            11,
            binding(),
            [
                11,
                source_grant,
                proof_base,
                2_231_696,
                source.length,
                user_read_only,
            ],
        )
        .unwrap();
        source_mapping_retained_context::new(
            creation,
            binding(),
            source_mapping_handle::new(17, 19).unwrap(),
        )
        .unwrap()
    }

    fn request() -> invocation {
        invocation::new(12, 9, [0x1_0002_8000, 0x198000, 5, 0x812, 3, 0x28000]).unwrap()
    }

    fn fixture_entries() -> [namespace_entry<'static, fixture_source>; 1] {
        let snapshot = file_snapshot::from_source(1, 1, &source).unwrap();
        [namespace_entry::new(b"/lib/libc", snapshot).unwrap()]
    }

    fn fixture<'a>(
        entries: &'a [namespace_entry<'a, fixture_source>; 1],
    ) -> (
        file_service<'a, 2, fixture_source>,
        descriptor_handle,
        file_snapshot<'a, fixture_source>,
    ) {
        let snapshot = entries[0].file();
        let namespace = namespace::new(b"/lib", entries).unwrap();
        let mut service = file_service::<2, fixture_source>::new(owner(), 5, 3).unwrap();
        let handle = service
            .open(owner(), 4, &namespace, b"/lib/libc", true, |_| {
                publication_result::committed
            })
            .unwrap();
        service
            .read_into(
                owner(),
                10,
                handle,
                832,
                832,
                &mut (),
                &mut [0; 832],
                |_| publication_result::committed,
            )
            .unwrap();
        (service, handle, snapshot)
    }

    fn unchanged(service: &file_service<'_, 2, fixture_source>, handle: descriptor_handle) {
        assert_eq!(service.last_sequence(), 10);
        assert_eq!(service.open_count(), 1);
        assert_eq!(service.handle_for_fd(owner(), 3), Ok(handle));
        assert_eq!(service.offset(owner(), handle), Ok(832));
        assert_eq!(service.close_on_exec(owner(), handle), Ok(true));
        assert!(!service.is_fenced());
    }

    #[test]
    fn exact_linux_request_yields_only_native_geometry_and_keeps_descriptor_state() {
        let entries = fixture_entries();
        let (service, handle, snapshot) = fixture(&entries);
        let proposed = executable_file_mapping(
            request(),
            binding(),
            retained(),
            &service,
            owner(),
            handle,
            snapshot,
        )
        .unwrap();
        assert_eq!(proposed.sequence(), 12);
        assert_eq!(proposed.base(), 0x1_0002_8000);
        assert_eq!(proposed.byte_length(), 0x198000);
        assert_eq!(proposed.source_offset(), 0x28000);
        assert_eq!(proposed.permissions(), user_read_execute);
        assert_eq!(proposed.handle(), retained().handle());
        assert_eq!(proposed.binding(), binding());
        assert_eq!((proposed.first_page(), proposed.page_count()), (40, 408));
        unchanged(&service, handle);
        // Recognition is repeatable, not a consumable native authorization.
        assert_eq!(
            executable_file_mapping(
                request(),
                binding(),
                retained(),
                &service,
                owner(),
                handle,
                snapshot,
            ),
            Some(proposed)
        );
        unchanged(&service, handle);
    }

    #[test]
    fn unrecognized_foreign_shapes_preserve_every_descriptor_observation() {
        let entries = fixture_entries();
        let (service, handle, snapshot) = fixture(&entries);
        for (index, value) in [
            (0, 0),
            (0, proof_base),
            (0, 0x1_0002_8001),
            (1, 0),
            (1, 0x197000),
            (1, 0x198001),
            (1, u64::MAX),
            (2, 1),
            (2, 3),
            (2, 7),
            (2, (1 << 32) | 5),
            (3, 0x802),
            (3, 0x12),
            (3, (1 << 32) | 0x812),
            (4, 0),
            (4, 4),
            (4, (1 << 32) | 3),
            (5, 0),
            (5, 0x29000),
            (5, u64::MAX),
        ] {
            let mut arguments = request().arguments();
            arguments[index] = value;
            assert_eq!(
                executable_file_mapping(
                    invocation::new(12, 9, arguments).unwrap(),
                    binding(),
                    retained(),
                    &service,
                    owner(),
                    handle,
                    snapshot,
                ),
                None,
                "field {index} value {value}"
            );
            unchanged(&service, handle);
        }
        for (sequence, number) in [
            (11, 9),
            (13, 9),
            ((1 << 32) | 12, 9),
            (12, 0),
            (12, (1 << 32) | 9),
        ] {
            assert_eq!(
                executable_file_mapping(
                    invocation::new(sequence, number, request().arguments()).unwrap(),
                    binding(),
                    retained(),
                    &service,
                    owner(),
                    handle,
                    snapshot,
                ),
                None
            );
            unchanged(&service, handle);
        }
    }

    #[test]
    fn different_source_binding_snapshot_and_owner_cannot_be_substituted() {
        let entries = fixture_entries();
        let (service, handle, snapshot) = fixture(&entries);
        let changed = file_snapshot::from_source(1, 1, &other_source).unwrap();
        assert_eq!(changed.object(), snapshot.object());
        assert_eq!(changed.version(), snapshot.version());
        assert_eq!(changed.byte_length(), snapshot.byte_length());
        assert_ne!(changed, snapshot);
        assert_eq!(
            executable_file_mapping(
                request(),
                binding(),
                retained(),
                &service,
                owner(),
                handle,
                changed,
            ),
            None
        );
        for changed_binding in [
            source_binding::new(source_grant, source.length + 1, 0x1234).unwrap(),
            source_binding::new(source_grant, source.length, 0x5678).unwrap(),
        ] {
            assert_eq!(
                executable_file_mapping(
                    request(),
                    changed_binding,
                    retained(),
                    &service,
                    owner(),
                    handle,
                    snapshot,
                ),
                None
            );
        }
        assert_eq!(
            executable_file_mapping(
                request(),
                binding(),
                retained(),
                &service,
                client_identity::new(7, 2).unwrap(),
                handle,
                snapshot,
            ),
            None
        );
        unchanged(&service, handle);
    }

    #[test]
    fn creation_sequence_base_and_populated_extent_bound_permission_replacement() {
        let entries = fixture_entries();
        let (service, handle, snapshot) = fixture(&entries);
        for (sequence, base, length) in [
            (10, proof_base, 2_231_696),
            (12, proof_base, 2_231_696),
            (11, proof_base - 4096, 2_231_696),
            (11, proof_base, 0x1bf000),
        ] {
            let rounded = (length + 4095) & !4095;
            let creation = source_mapping_request::decode(
                sequence,
                binding(),
                [
                    sequence,
                    source_grant,
                    base,
                    length,
                    source.length.min(rounded),
                    user_read_only,
                ],
            )
            .unwrap();
            let changed =
                source_mapping_retained_context::new(creation, binding(), retained().handle())
                    .unwrap();
            assert_eq!(
                executable_file_mapping(
                    request(),
                    binding(),
                    changed,
                    &service,
                    owner(),
                    handle,
                    snapshot,
                ),
                None
            );
            unchanged(&service, handle);
        }
    }

    #[test]
    fn reused_or_foreign_descriptor_and_retired_service_are_refused() {
        let entries = fixture_entries();
        let snapshot = entries[0].file();
        let namespace = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<2, fixture_source>::new(owner(), 5, 3).unwrap();
        let stale = service
            .open(owner(), 4, &namespace, b"/lib/libc", true, |_| {
                publication_result::committed
            })
            .unwrap();
        service
            .close(owner(), 5, stale, |_| publication_result::committed)
            .unwrap();
        let live = service
            .open(owner(), 6, &namespace, b"/lib/libc", true, |_| {
                publication_result::committed
            })
            .unwrap();
        assert_eq!(live.fd(), stale.fd());
        assert_ne!(live, stale);
        assert_eq!(
            executable_file_mapping(
                request(),
                binding(),
                retained(),
                &service,
                owner(),
                stale,
                snapshot,
            ),
            None
        );
        assert!(
            executable_file_mapping(
                request(),
                binding(),
                retained(),
                &service,
                owner(),
                live,
                snapshot,
            )
            .is_some()
        );
        assert_eq!(service.last_sequence(), 6);
        assert_eq!(service.offset(owner(), live), Ok(0));
        assert_eq!(service.open_count(), 1);
        let (other_service, other_handle, _) = fixture(&entries);
        let mut replacement = file_service::<2, fixture_source>::new(owner(), 6, 3).unwrap();
        replacement
            .open(owner(), 4, &namespace, b"/lib/libc", true, |_| {
                publication_result::committed
            })
            .unwrap();
        assert_eq!(
            executable_file_mapping(
                request(),
                binding(),
                retained(),
                &replacement,
                owner(),
                other_handle,
                snapshot,
            ),
            None
        );
        unchanged(&other_service, other_handle);
        service.retire(owner()).unwrap();
        assert_eq!(
            executable_file_mapping(
                request(),
                binding(),
                retained(),
                &service,
                owner(),
                live,
                snapshot,
            ),
            None
        );
        assert!(service.is_fenced());
        assert_eq!(service.open_count(), 0);
        assert_eq!(service.last_sequence(), 6);
    }

    #[test]
    fn file_watermark_at_the_pending_call_refuses_without_changing_other_state() {
        let entries = fixture_entries();
        let (mut service, handle, snapshot) = fixture(&entries);
        service
            .metadata(owner(), 12, handle, |_| publication_result::committed)
            .unwrap();
        assert_eq!(
            executable_file_mapping(
                request(),
                binding(),
                retained(),
                &service,
                owner(),
                handle,
                snapshot,
            ),
            None
        );
        assert_eq!(service.last_sequence(), 12);
        assert_eq!(service.open_count(), 1);
        assert_eq!(service.handle_for_fd(owner(), 3), Ok(handle));
        assert_eq!(service.offset(owner(), handle), Ok(832));
        assert_eq!(service.close_on_exec(owner(), handle), Ok(true));
        assert!(!service.is_fenced());
    }
}
