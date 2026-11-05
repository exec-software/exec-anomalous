use super::*;
use crate::channel_snapshot;

struct mock_backend {
    base: u64,
    pending_base: Option<u64>,
    pending_copy: Option<[u8; 8]>,
    output: [u8; 16],
    queries: usize,
    sets: usize,
    copies: usize,
    error: Option<native_status>,
    receipt: stage_receipt,
}

impl mock_backend {
    fn new() -> Self {
        Self {
            base: 0,
            pending_base: None,
            pending_copy: None,
            output: [0xa5; 16],
            queries: 0,
            sets: 0,
            copies: 0,
            error: None,
            receipt: stage_receipt {
                token: 7,
                length: 0,
            },
        }
    }
    fn commit(&mut self) {
        if let Some(base) = self.pending_base.take() {
            self.base = base;
        }
        if let Some(bytes) = self.pending_copy.take() {
            self.output[4..12].copy_from_slice(&bytes);
        }
    }
}

impl linux_policy::backend for mock_backend {
    fn query(&mut self, _: u64) -> Result<channel_snapshot, native_status> {
        panic!("unexpected channel query")
    }
    fn caller_read(&mut self, _: u64, _: &mut [u8]) -> Result<(), native_status> {
        panic!("unexpected caller read")
    }
    fn stage(
        &mut self,
        kind: stage_kind,
        address: u64,
        length: u64,
        source: &[u8],
    ) -> Result<stage_receipt, native_status> {
        self.copies += 1;
        assert_eq!((kind, length, source.len()), (stage_kind::copyout, 8, 8));
        if let Some(error) = self.error {
            return Err(error);
        }
        if address != 0x800004 {
            return Err(native_status::bad_copy);
        }
        self.pending_copy = Some(source.try_into().unwrap());
        Ok(self.receipt)
    }
}
impl backend for mock_backend {
    fn caller_fs_query(&mut self) -> Result<u64, native_status> {
        self.queries += 1;
        self.error.map_or(Ok(self.base), Err)
    }
    fn caller_fs_stage(&mut self, base: u64) -> Result<stage_receipt, native_status> {
        self.sets += 1;
        if let Some(error) = self.error {
            return Err(error);
        }
        self.pending_base = Some(base);
        Ok(self.receipt)
    }
}
fn request(option: u64, argument: u64) -> invocation {
    invocation::new(7, 158, [option, argument, u64::MAX, 3, 4, 5]).unwrap()
}

#[test]
fn fs_values_are_full_width_staged_and_zero_or_unmapped_bases_are_valid() {
    assert_eq!(linux_task_size_max, 0x7fff_ffff_f000);
    for base in [0, 0x800020, 0x7fff12345000, linux_task_size_max - 1] {
        let mut backend = mock_backend::new();
        let reply = evaluate(request(arch_set_fs as u64, base), &mut backend).unwrap();
        assert_eq!(reply.result_bits, 0);
        assert_eq!(reply.stage_token, 7);
        assert_eq!(backend.base, 0);
        assert_eq!(backend.pending_base, Some(base));
        backend.commit();
        assert_eq!(backend.base, base);
    }
}
#[test]
fn set_rejects_linux_reserved_top_page_noncanonical_and_kernel_values_without_effects() {
    for base in [
        linux_task_size_max,
        linux_task_size_max + 1,
        native_user_limit - 1,
        native_user_limit,
        0x0001000000000000,
        0xffff800000000000,
        u64::MAX,
    ] {
        let mut backend = mock_backend::new();
        backend.base = 0x800020;
        assert_eq!(
            evaluate(request(arch_set_fs as u64, base), &mut backend).unwrap(),
            completion::returned(-1)
        );
        assert_eq!((backend.sets, backend.queries, backend.copies), (0, 0, 0));
        assert!(backend.pending_base.is_none());
        assert!(backend.pending_copy.is_none());
        assert_eq!(backend.base, 0x800020);
        assert_eq!(backend.output, [0xa5; 16]);
    }
}
#[test]
fn get_preserves_native_canonical_values_above_linux_set_limit() {
    for base in [linux_task_size_max, native_user_limit - 1] {
        let mut backend = mock_backend::new();
        backend.base = base;
        backend.receipt.length = 8;
        let reply = evaluate(request(arch_get_fs as u64, 0x800004), &mut backend).unwrap();
        assert_eq!((reply.result_bits, reply.stage_token), (0, 7));
        assert_eq!(backend.pending_copy, Some(base.to_le_bytes()));
        assert_eq!(backend.output, [0xa5; 16]);
        assert_eq!(backend.sets, 0);
    }
}
#[test]
fn get_copies_exact_little_endian_bytes_only_after_commit_and_keeps_canaries() {
    let mut backend = mock_backend::new();
    backend.base = 0x7fff12345000;
    backend.receipt.length = 8;
    let reply = evaluate(request(arch_get_fs as u64, 0x800004), &mut backend).unwrap();
    assert_eq!((reply.result_bits, reply.stage_token), (0, 7));
    assert_eq!(backend.output, [0xa5; 16]);
    assert_eq!(backend.pending_copy, Some(backend.base.to_le_bytes()));
    backend.commit();
    assert_eq!(&backend.output[..4], &[0xa5; 4]);
    assert_eq!(&backend.output[4..12], &backend.base.to_le_bytes());
    assert_eq!(&backend.output[12..], &[0xa5; 4]);
}
#[test]
fn get_bad_destinations_and_backend_errors_leave_entire_output_unchanged() {
    for destination in [0, 0x400000, 0x801ffc, u64::MAX] {
        let mut backend = mock_backend::new();
        backend.receipt.length = 8;
        assert_eq!(
            evaluate(request(arch_get_fs as u64, destination), &mut backend).unwrap(),
            completion::returned(-14)
        );
        assert_eq!(backend.output, [0xa5; 16]);
        assert!(backend.pending_copy.is_none());
    }
    for (status, errno) in [
        (native_status::denied, -1),
        (native_status::invalid, -1),
        (native_status::exhausted, -75),
    ] {
        let mut backend = mock_backend::new();
        backend.error = Some(status);
        assert_eq!(
            evaluate(request(arch_set_fs as u64, 0x800020), &mut backend).unwrap(),
            completion::returned(errno)
        );
        assert!(backend.pending_base.is_none());
    }
}
#[test]
fn exact_stage_receipts_and_valid_observations_are_required() {
    for (option, length) in [(arch_set_fs, 0), (arch_get_fs, 8)] {
        for receipt in [
            stage_receipt { token: 6, length },
            stage_receipt {
                token: 7,
                length: length + 1,
            },
        ] {
            let mut backend = mock_backend::new();
            backend.receipt = receipt;
            assert_eq!(
                evaluate(request(option as u64, 0x800004), &mut backend),
                Err(policy_error::invalid_stage_receipt)
            );
            assert_eq!(backend.output, [0xa5; 16]);
            assert_eq!(backend.base, 0);
        }
    }
    let mut backend = mock_backend::new();
    backend.base = native_user_limit;
    assert_eq!(
        evaluate(request(arch_get_fs as u64, 0x800004), &mut backend),
        Err(policy_error::invalid_fs_observation)
    );
    assert_eq!(backend.copies, 0);
}
#[test]
fn option_uses_linux_int_width_and_unsupported_gs_never_touches_native_state() {
    for option in [0x1001, 0x1004, 0, 0xffff_ffff] {
        let mut backend = mock_backend::new();
        assert_eq!(
            evaluate(request(option, 0), &mut backend).unwrap(),
            completion::returned(-22)
        );
        assert_eq!((backend.sets, backend.queries, backend.copies), (0, 0, 0));
    }
    let mut backend = mock_backend::new();
    assert_eq!(
        evaluate(request(0xface000000001002, 0), &mut backend)
            .unwrap()
            .stage_token,
        7
    );
}
#[test]
fn old_policy_remains_zero_tls_and_non_tls_calls_delegate_exactly() {
    let mut backend = mock_backend::new();
    assert_eq!(
        linux_policy::evaluate(request(arch_set_fs as u64, 0), &mut backend).unwrap(),
        completion::returned(-38)
    );
    for number in [24, 60, 999] {
        let request = invocation::new(7, number, [2, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(
            evaluate(request, &mut backend).unwrap(),
            linux_policy::evaluate(request, &mut backend).unwrap()
        );
    }
}
