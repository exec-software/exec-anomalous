//! Private userspace policy for one read-only file-mapping preparation proof.
//! This recognizes a bounded Linux request and checks retained file-service
//! state; it neither maps memory nor returns a Linux result. The embedding
//! supplies an authenticated current invocation and the native binding belonging
//! to `snapshot.source()`. Only the native controller can authenticate those
//! facts, reject overlap with the caller's complete plan and prepare/cancel.

use crate::file_service::{
    client_identity, descriptor_handle, file_service, file_snapshot, immutable_file_source,
};
use crate::invocation;
use crate::source_mapping::{page_limit, page_size, source_mapping_request, user_read_only};
use crate::source_read::source_binding;

pub const first_mapping_sequence: u64 = 11;
pub const proof_base: u64 = 0x1_0000_0000;

pub fn first_file_mapping<const capacity: usize, source: immutable_file_source + ?Sized>(
    request: invocation,
    binding: source_binding,
    service: &file_service<'_, capacity, source>,
    owner: client_identity,
    handle: descriptor_handle,
    snapshot: file_snapshot<'_, source>,
) -> Option<source_mapping_request> {
    let [address, length, protection, flags, descriptor, offset] = request.arguments();
    // Deliberately recognize only this proof's complete register shape. This
    // does not establish broader Linux mmap flag or integer-narrowing policy.
    if request.sequence() != first_mapping_sequence
        || request.number() != 9
        || address != 0
        || !(1..=page_limit * page_size).contains(&length)
        || protection != 1
        || flags != 0x802
        || descriptor != 3
        || offset != 0
        || handle.fd() != 3
        || service.last_sequence() >= request.sequence()
        || service.handle_for_fd(owner, 3).ok() != Some(handle)
        || service.file(owner, handle).ok() != Some(snapshot)
        || snapshot.byte_length() != binding.length()
    {
        return None;
    }
    let rounded_length = length.checked_add(page_size - 1)? & !(page_size - 1);
    source_mapping_request::decode(
        request.sequence(),
        binding,
        [
            request.sequence(),
            binding.grant(),
            proof_base,
            length,
            binding.length().min(rounded_length),
            user_read_only,
        ],
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_service::{namespace, namespace_entry, publication_result};
    use crate::source_read::source_grant;

    fn owner() -> client_identity {
        client_identity::new(7, 1).unwrap()
    }

    fn binding() -> source_binding {
        source_binding::new(source_grant, 3, 0x1234).unwrap()
    }

    fn request() -> invocation {
        invocation::new(11, 9, [0, 8193, 1, 0x802, 3, 0]).unwrap()
    }

    fn fixture() -> (file_snapshot<'static>, [namespace_entry<'static>; 1]) {
        let snapshot = file_snapshot::new(1, 1, b"abc").unwrap();
        (
            snapshot,
            [namespace_entry::new(b"/lib/libc", snapshot).unwrap()],
        )
    }

    #[test]
    fn live_snapshot_preparation_does_not_commit_a_file_operation() {
        let (snapshot, entries) = fixture();
        let namespace = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<2>::new(owner(), 5, 3).unwrap();
        let handle = service
            .open(owner(), 4, &namespace, b"/lib/libc", true, |_| {
                publication_result::committed
            })
            .unwrap();
        let prepared =
            first_file_mapping(request(), binding(), &service, owner(), handle, snapshot).unwrap();
        assert_eq!(prepared.base(), proof_base);
        assert_eq!(prepared.byte_length(), 8193);
        assert_eq!(prepared.populated_byte_length(), 3);
        assert_eq!((prepared.data_pages(), prepared.virtual_pages()), (1, 3));
        assert_eq!(service.last_sequence(), 4);
        assert_eq!(service.open_count(), 1);
        assert_eq!(service.offset(owner(), handle), Ok(0));
        assert_eq!(service.close_on_exec(owner(), handle), Ok(true));
        assert!(!service.is_fenced());
    }

    #[test]
    fn every_unrecognized_linux_register_shape_remains_unsupported() {
        let (snapshot, entries) = fixture();
        let namespace = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<2>::new(owner(), 5, 3).unwrap();
        let handle = service
            .open(owner(), 4, &namespace, b"/lib/libc", false, |_| {
                publication_result::committed
            })
            .unwrap();
        for (index, value) in [
            (0, proof_base),
            (1, 0),
            (1, page_limit * page_size + 1),
            (1, u64::MAX),
            (2, 0),
            (2, 3),
            (2, 5),
            (2, (1 << 32) | 1),
            (3, 2),
            (3, 0x812),
            (3, 0x822),
            (3, (1 << 32) | 0x802),
            (4, 4),
            (4, (1 << 32) | 3),
            (5, 4096),
        ] {
            let mut arguments = request().arguments();
            arguments[index] = value;
            assert_eq!(
                first_file_mapping(
                    invocation::new(11, 9, arguments).unwrap(),
                    binding(),
                    &service,
                    owner(),
                    handle,
                    snapshot,
                ),
                None,
                "field {index} value {value}"
            );
        }
        for (sequence, number) in [(10, 9), (12, 9), (11, 0)] {
            assert_eq!(
                first_file_mapping(
                    invocation::new(sequence, number, request().arguments()).unwrap(),
                    binding(),
                    &service,
                    owner(),
                    handle,
                    snapshot,
                ),
                None
            );
        }
        assert_eq!(service.last_sequence(), 4);
    }

    #[test]
    fn same_numeric_file_identity_cannot_substitute_a_different_snapshot() {
        let (snapshot, entries) = fixture();
        let namespace = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<2>::new(owner(), 5, 3).unwrap();
        let handle = service
            .open(owner(), 4, &namespace, b"/lib/libc", false, |_| {
                publication_result::committed
            })
            .unwrap();
        let changed = file_snapshot::new(1, 1, b"xyz").unwrap();
        assert_eq!(
            first_file_mapping(request(), binding(), &service, owner(), handle, changed),
            None
        );
        let wrong_length = source_binding::new(source_grant, 4, binding().fnv64()).unwrap();
        assert_eq!(
            first_file_mapping(request(), wrong_length, &service, owner(), handle, snapshot),
            None
        );
        let wrong_owner = client_identity::new(7, 2).unwrap();
        assert_eq!(
            first_file_mapping(
                request(),
                binding(),
                &service,
                wrong_owner,
                handle,
                snapshot
            ),
            None
        );
    }

    #[test]
    fn reused_descriptor_number_needs_the_live_handle_and_service_incarnation() {
        let (snapshot, entries) = fixture();
        let namespace = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<2>::new(owner(), 5, 3).unwrap();
        let stale = service
            .open(owner(), 4, &namespace, b"/lib/libc", false, |_| {
                publication_result::committed
            })
            .unwrap();
        service
            .close(owner(), 5, stale, |_| publication_result::committed)
            .unwrap();
        let live = service
            .open(owner(), 6, &namespace, b"/lib/libc", false, |_| {
                publication_result::committed
            })
            .unwrap();
        assert_eq!(live.fd(), stale.fd());
        assert_ne!(live, stale);
        assert_eq!(
            first_file_mapping(request(), binding(), &service, owner(), stale, snapshot),
            None
        );
        assert!(
            first_file_mapping(request(), binding(), &service, owner(), live, snapshot).is_some()
        );
        let mut replacement = file_service::<2>::new(owner(), 6, 3).unwrap();
        let replacement_handle = replacement
            .open(owner(), 4, &namespace, b"/lib/libc", false, |_| {
                publication_result::committed
            })
            .unwrap();
        assert_eq!(replacement_handle.fd(), stale.fd());
        assert_eq!(
            first_file_mapping(request(), binding(), &replacement, owner(), stale, snapshot),
            None
        );
        service.retire(owner()).unwrap();
        assert_eq!(
            first_file_mapping(request(), binding(), &service, owner(), live, snapshot),
            None
        );
    }

    #[test]
    fn mapping_recognition_rejects_a_file_watermark_at_the_pending_sequence() {
        let (snapshot, entries) = fixture();
        let namespace = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<2>::new(owner(), 5, 3).unwrap();
        let handle = service
            .open(owner(), 11, &namespace, b"/lib/libc", false, |_| {
                publication_result::committed
            })
            .unwrap();
        assert_eq!(
            first_file_mapping(request(), binding(), &service, owner(), handle, snapshot),
            None
        );
    }
}
