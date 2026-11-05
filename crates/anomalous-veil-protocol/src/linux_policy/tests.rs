use super::*;

struct mock_backend {
    snapshots: [channel_snapshot; 2],
    input: [u8; 64],
    output: [u8; 64],
    query_count: usize,
    read_count: usize,
    stage_count: usize,
    read_error: Option<native_status>,
    stage_error: Option<native_status>,
    stage: Option<(stage_kind, u64, u64)>,
    sequence: u64,
}

fn snapshot(phase: channel_phase, exhausted: bool) -> channel_snapshot {
    channel_snapshot::new(
        phase,
        exhausted,
        if matches!(phase, channel_phase::queued | channel_phase::delivered) {
            64
        } else {
            0
        },
    )
    .unwrap()
}

impl mock_backend {
    fn new() -> Self {
        Self {
            snapshots: [
                snapshot(channel_phase::empty, false),
                snapshot(channel_phase::queued, false),
            ],
            input: [0; 64],
            output: [0xa5; 64],
            query_count: 0,
            read_count: 0,
            stage_count: 0,
            read_error: None,
            stage_error: None,
            stage: None,
            sequence: 9,
        }
    }
}

impl backend for mock_backend {
    fn query(&mut self, capability: u64) -> Result<channel_snapshot, native_status> {
        self.query_count += 1;
        self.snapshots
            .get(capability as usize)
            .copied()
            .ok_or(native_status::denied)
    }
    fn caller_read(&mut self, _address: u64, destination: &mut [u8]) -> Result<(), native_status> {
        self.read_count += 1;
        if let Some(error) = self.read_error {
            return Err(error);
        }
        destination.copy_from_slice(&self.input[..destination.len()]);
        Ok(())
    }
    fn stage(
        &mut self,
        kind: stage_kind,
        address: u64,
        length: u64,
        source: &[u8],
    ) -> Result<stage_receipt, native_status> {
        self.stage_count += 1;
        if let Some(error) = self.stage_error {
            return Err(error);
        }
        self.stage = Some((kind, address, length));
        if kind == stage_kind::copyout {
            self.output[..source.len()].copy_from_slice(source);
        }
        Ok(stage_receipt {
            token: self.sequence,
            length: if kind == stage_kind::consume {
                self.snapshots[1].length() as u64
            } else {
                length
            },
        })
    }
}

fn run(backend: &mut mock_backend, number: u64, args: [u64; 3]) -> completion {
    evaluate(
        invocation::new(9, number, [args[0], args[1], args[2], 17, 18, 19]).unwrap(),
        backend,
    )
    .unwrap()
}

fn poll_record(fd: i32, events: u16) -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&fd.to_le_bytes());
    bytes[4..6].copy_from_slice(&events.to_le_bytes());
    bytes[6..].copy_from_slice(&0x55aa_u16.to_le_bytes());
    bytes
}

#[test]
fn syscall_widths_wrong_directions_zero_and_oversize_precede_backend_access() {
    let mut backend = mock_backend::new();
    assert_eq!(
        run(&mut backend, 1 << 32, [3, 0, 64]),
        completion::returned(bad_fd)
    );
    assert_eq!(
        run(&mut backend, 1, [4, 0, 3168]),
        completion::returned(bad_fd)
    );
    for (number, fd) in [(0, 4), (1, 3)] {
        assert_eq!(
            run(&mut backend, number, [fd | (1 << 32), u64::MAX, 0]),
            completion::returned(0)
        );
        assert_eq!(
            run(&mut backend, number, [fd, 0, 4097]),
            completion::returned(message_size)
        );
        assert_eq!(
            run(&mut backend, number, [fd, 0, u64::MAX]),
            completion::returned(message_size)
        );
    }
    assert_eq!(
        (backend.query_count, backend.read_count, backend.stage_count),
        (0, 0, 0)
    );
}

#[test]
fn writes_preserve_record_state_and_only_propose_one_successful_publication() {
    for (phase, exhausted, expected) in [
        (channel_phase::closed, false, no_device),
        (channel_phase::queued, false, again),
        (channel_phase::delivered, false, again),
        (channel_phase::empty, true, overflow),
    ] {
        let mut backend = mock_backend::new();
        backend.snapshots[0] = snapshot(phase, exhausted);
        assert_eq!(
            run(&mut backend, 1, [3, 0x800100, 3168]),
            completion::returned(expected)
        );
        assert_eq!(backend.stage_count, 0);
    }
    let mut backend = mock_backend::new();
    backend.stage_error = Some(native_status::bad_copy);
    assert_eq!(
        run(&mut backend, 1, [3, 0, 3168]),
        completion::returned(bad_address)
    );
    assert_eq!(backend.stage, None);
    backend.stage_error = None;
    let returned = run(&mut backend, 1, [3, 0x800100, 3168]);
    assert_eq!(returned.result_bits, 3168);
    assert_eq!(returned.stage_token, 9);
    assert_eq!(backend.stage, Some((stage_kind::submit, 0x800100, 3168)));
    assert_eq!(backend.snapshots[0].phase(), channel_phase::empty);
}

#[test]
fn reads_preserve_queue_on_capacity_and_copy_failure_then_stage_atomic_consumption() {
    let mut backend = mock_backend::new();
    assert_eq!(
        run(&mut backend, 0, [4, 0x800100, 63]),
        completion::returned(message_size)
    );
    assert_eq!(backend.stage_count, 0);
    backend.stage_error = Some(native_status::bad_copy);
    assert_eq!(
        run(&mut backend, 0, [4, 0, 64]),
        completion::returned(bad_address)
    );
    assert_eq!(backend.stage, None);
    assert_eq!(backend.snapshots[1].phase(), channel_phase::queued);
    backend.stage_error = None;
    let returned = run(&mut backend, 0, [4, 0x800100, 4096]);
    assert_eq!(returned.result_bits, 64);
    assert_eq!(returned.stage_token, 9);
    assert_eq!(backend.stage, Some((stage_kind::consume, 0x800100, 4096)));
    assert_eq!(backend.snapshots[1].phase(), channel_phase::queued);
    for (phase, expected) in [
        (channel_phase::empty, again),
        (channel_phase::delivered, again),
        (channel_phase::closed, 0),
    ] {
        backend.snapshots[1] = snapshot(phase, false);
        assert_eq!(
            run(&mut backend, 0, [4, 0, 64]),
            completion::returned(expected)
        );
    }
}

#[test]
fn poll_readiness_matches_all_route_states_masks_and_duplicate_entries() {
    let mut backend = mock_backend::new();
    for phase in [
        channel_phase::empty,
        channel_phase::queued,
        channel_phase::delivered,
        channel_phase::closed,
    ] {
        for exhausted in [false, true] {
            backend.snapshots = [snapshot(phase, exhausted); 2];
            for events in [0, 1, 4, 5, 0xffff] {
                let expected_out = match phase {
                    channel_phase::empty if exhausted => 8,
                    channel_phase::empty => events & 4,
                    channel_phase::closed => 24,
                    _ => 0,
                };
                let expected_in = match phase {
                    channel_phase::queued => events & 1,
                    channel_phase::closed => 16,
                    _ => 0,
                };
                assert_eq!(poll_flags(3, events, backend.snapshots), expected_out);
                assert_eq!(poll_flags(4, events, backend.snapshots), expected_in);
                assert_eq!(poll_flags(-1, events, backend.snapshots), 0);
                assert_eq!(poll_flags(99, events, backend.snapshots), 32);
            }
        }
    }
    backend.snapshots = [
        snapshot(channel_phase::empty, false),
        snapshot(channel_phase::queued, false),
    ];
    for (index, (fd, events)) in [(3, 4), (3, 4), (-1, 5), (99, 0), (4, 1)]
        .into_iter()
        .enumerate()
    {
        backend.input[index * 8..index * 8 + 8].copy_from_slice(&poll_record(fd, events));
    }
    let original = backend.input;
    let returned = run(&mut backend, 7, [0x800100, 5, 0]);
    assert_eq!(returned.result_bits, 4);
    assert_eq!(returned.stage_token, 9);
    assert_eq!(backend.stage, Some((stage_kind::copyout, 0x800100, 40)));
    for (index, flags) in [4u16, 4, 0, 32, 1].into_iter().enumerate() {
        assert_eq!(
            &backend.output[index * 8..index * 8 + 6],
            &original[index * 8..index * 8 + 6]
        );
        assert_eq!(
            &backend.output[index * 8 + 6..index * 8 + 8],
            &flags.to_le_bytes()
        );
    }
    assert_eq!(backend.input, original);
}

#[test]
fn poll_bounds_and_full_copy_failures_do_not_stage_partial_output() {
    let mut backend = mock_backend::new();
    for args in [[u64::MAX, 9, 0], [0, 1, 1], [0, 0, u32::MAX as u64]] {
        assert_eq!(
            run(&mut backend, 7, args),
            completion::returned(invalid_argument)
        );
    }
    assert_eq!(
        run(&mut backend, 7, [u64::MAX, 1 << 32, 1 << 32]),
        completion::returned(0)
    );
    assert_eq!(
        (backend.query_count, backend.read_count, backend.stage_count),
        (0, 0, 0)
    );
    backend.input[..8].copy_from_slice(&poll_record(3, 4));
    backend.read_error = Some(native_status::bad_copy);
    assert_eq!(
        run(&mut backend, 7, [0x801ff8, 2, 0]),
        completion::returned(bad_address)
    );
    assert_eq!(backend.stage_count, 0);
    backend.read_error = None;
    backend.stage_error = Some(native_status::bad_copy);
    let before = backend.input;
    assert_eq!(
        run(&mut backend, 7, [0x400100, 1, 0]),
        completion::returned(bad_address)
    );
    assert_eq!(backend.input, before);
    assert_eq!(backend.output, [0xa5; 64]);
    assert_eq!(backend.stage, None);
}

#[test]
fn actions_unknown_calls_and_stage_receipts_have_separate_native_meaning() {
    let mut backend = mock_backend::new();
    for number in [u64::MAX, 0x8000_0000, 61] {
        assert_eq!(
            run(&mut backend, number, [0; 3]),
            completion::returned(not_implemented)
        );
    }
    assert_eq!(
        run(&mut backend, 24, [0; 3]),
        completion {
            result_bits: 0,
            action: completion_action::yield_caller,
            stage_token: 0
        }
    );
    assert_eq!(
        run(&mut backend, 60, [0x1234, 0, 0]),
        completion {
            result_bits: 0x34,
            action: completion_action::retire_caller,
            stage_token: 0
        }
    );
    assert_eq!(
        (backend.query_count, backend.read_count, backend.stage_count),
        (0, 0, 0)
    );
    assert_eq!(
        staged(
            9,
            stage_receipt {
                token: 8,
                length: 64
            },
            64,
            64
        ),
        Err(policy_error::invalid_stage_receipt)
    );
    assert_eq!(
        staged(
            9,
            stage_receipt {
                token: 9,
                length: 63
            },
            64,
            64
        ),
        Err(policy_error::invalid_stage_receipt)
    );
    for (status, errno) in [
        (native_status::not_ready, again),
        (native_status::denied, bad_fd),
        (native_status::bad_copy, bad_address),
        (native_status::closed, no_device),
        (native_status::too_small, message_size),
        (native_status::exhausted, overflow),
        (native_status::invalid, invalid_argument),
    ] {
        assert_eq!(native_error(status), errno);
    }
}
