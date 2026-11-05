#![allow(non_camel_case_types, non_upper_case_globals)]

//! Independent crash model: only successful flushes require volatile writes to
//! reach media. Failed operations may already have taken effect. The recovery
//! assertions concern known document bytes and versions, not the record codec.

use std::cell::RefCell;
use std::rc::Rc;

use anomalous_document_protocol::durable::{
    self, disk_identity, durable_error, durable_store, io_error, sector_count, sector_io,
    sector_size,
};
use anomalous_document_protocol::{client_identity, operation, record, service_error};

type sectors = [[u8; sector_size]; sector_count];

const original: &[u8] = b"const before = 1;";
const accepted: &[u8] = b"const after = 1;";
const candidate: &[u8] = b"const later = 1;";
const replacement: &[u8] = b"const newer = 1;";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum io_operation {
    read(usize),
    write(usize),
    flush,
}

#[derive(Clone, Copy, Debug)]
enum failure_effect {
    before,
    after,
    torn_prefix(usize),
    flush_prefix(usize),
}

#[derive(Clone, Copy, Debug)]
struct failure {
    operation: usize,
    effect: failure_effect,
}

struct disk_state {
    durable: sectors,
    volatile: sectors,
    // Both policies are legal before a flush: none or all of a completed write
    // may have reached media. Torn writes additionally model prefix persistence.
    write_through: bool,
    trace: Vec<io_operation>,
    failure: Option<failure>,
    stopped: bool,
}

#[derive(Clone)]
struct disk_observer(Rc<RefCell<disk_state>>);

struct model_disk {
    state: disk_observer,
}

impl model_disk {
    fn blank(write_through: bool) -> Self {
        Self::from_media([[0; sector_size]; sector_count], write_through)
    }

    fn from_media(durable: sectors, write_through: bool) -> Self {
        Self {
            state: disk_observer(Rc::new(RefCell::new(disk_state {
                durable,
                volatile: durable,
                write_through,
                trace: Vec::new(),
                failure: None,
                stopped: false,
            }))),
        }
    }

    fn observe(&self) -> disk_observer {
        self.state.clone()
    }
}

impl disk_observer {
    fn arm(&self, failure: Option<failure>) {
        let mut state = self.0.borrow_mut();
        assert!(!state.stopped, "a failed device must not be silently reset");
        state.trace.clear();
        state.failure = failure;
    }

    fn trace(&self) -> Vec<io_operation> {
        self.0.borrow().trace.clone()
    }

    fn media(&self) -> sectors {
        self.0.borrow().durable
    }

    fn volatile(&self) -> sectors {
        self.0.borrow().volatile
    }

    fn cold_boot(&self) -> model_disk {
        // Power loss discards every unflushed byte and all driver state. The
        // successor shares neither mutable RAM nor an authority with this disk.
        model_disk::from_media(self.media(), self.0.borrow().write_through)
    }
}

impl sector_io for model_disk {
    fn read_sector(&mut self, lba: u64, output: &mut [u8; sector_size]) -> Result<(), io_error> {
        let index = usize::try_from(lba).expect("LBA fits host usize");
        assert!(
            index < sector_count,
            "transport escaped its five-sector extent"
        );
        let mut state = self.state.0.borrow_mut();
        let effect = begin_operation(&mut state, io_operation::read(index))?;
        output.copy_from_slice(&state.volatile[index]);
        finish_operation(&mut state, effect)
    }

    fn write_sector(&mut self, lba: u64, input: &[u8; sector_size]) -> Result<(), io_error> {
        let index = usize::try_from(lba).expect("LBA fits host usize");
        assert!(
            index < sector_count,
            "transport escaped its five-sector extent"
        );
        let mut state = self.state.0.borrow_mut();
        let effect = begin_operation(&mut state, io_operation::write(index))?;
        if let Some(failure_effect::torn_prefix(prefix)) = effect {
            assert!(prefix <= sector_size);
            state.volatile[index][..prefix].copy_from_slice(&input[..prefix]);
            // A failing device may have persisted a prefix before reporting the
            // error, regardless of its usual completed-write caching policy.
            state.durable[index][..prefix].copy_from_slice(&input[..prefix]);
            state.stopped = true;
            return Err(io_error::failed);
        }
        state.volatile[index] = *input;
        if state.write_through {
            state.durable[index] = *input;
        }
        finish_operation(&mut state, effect)
    }

    fn flush(&mut self) -> Result<(), io_error> {
        let mut state = self.state.0.borrow_mut();
        let effect = begin_operation(&mut state, io_operation::flush)?;
        if let Some(failure_effect::flush_prefix(prefix)) = effect {
            assert!(prefix <= sector_size);
            let volatile = state.volatile;
            for (persisted, pending) in state.durable.iter_mut().zip(volatile.iter()) {
                persisted[..prefix].copy_from_slice(&pending[..prefix]);
            }
            state.stopped = true;
            return Err(io_error::failed);
        }
        state.durable = state.volatile;
        finish_operation(&mut state, effect)
    }
}

fn begin_operation(
    state: &mut disk_state,
    operation: io_operation,
) -> Result<Option<failure_effect>, io_error> {
    state.trace.push(operation);
    if state.stopped {
        return Err(io_error::failed);
    }
    let effect = state
        .failure
        .filter(|failure| failure.operation == state.trace.len())
        .map(|failure| failure.effect);
    if matches!(effect, Some(failure_effect::before)) {
        state.stopped = true;
        return Err(io_error::failed);
    }
    if matches!(effect, Some(failure_effect::torn_prefix(_))) {
        assert!(matches!(operation, io_operation::write(_)));
    }
    if matches!(effect, Some(failure_effect::flush_prefix(_))) {
        assert_eq!(operation, io_operation::flush);
    }
    Ok(effect)
}

fn finish_operation(
    state: &mut disk_state,
    effect: Option<failure_effect>,
) -> Result<(), io_error> {
    if matches!(effect, Some(failure_effect::after)) {
        state.stopped = true;
        Err(io_error::failed)
    } else {
        Ok(())
    }
}

fn identity() -> disk_identity {
    disk_identity::try_new(0x4411, 0xd201).unwrap()
}

fn authority() -> client_identity {
    client_identity::try_new(1, 1).unwrap()
}

fn successor() -> client_identity {
    client_identity::try_new(1, 3).unwrap()
}

fn with_accepted_store<R>(
    write_through: bool,
    operation: impl FnOnce(&mut durable_store<'_, model_disk>, disk_observer, record) -> R,
) -> R {
    let mut disk = model_disk::blank(write_through);
    let observation = disk.observe();
    durable::format_disposable(&mut disk, identity(), original).unwrap();
    let mut store = durable_store::mount(&mut disk, identity(), authority()).unwrap();
    let opened = store
        .execute(authority(), &record::open(store.document(), 1).unwrap())
        .unwrap();
    let first = record::save(store.document(), opened.generation(), 1, 1, 2, accepted).unwrap();
    let reply = store.execute(authority(), &first).unwrap();
    assert_eq!(reply.version(), 2);
    assert_eq!(reply.text(), accepted);
    let request = record::save(store.document(), opened.generation(), 2, 2, 4, candidate).unwrap();
    operation(&mut store, observation, request)
}

fn recover_semantics(observation: &disk_observer) -> (u64, Vec<u8>) {
    let media = observation.media();
    let mut rebooted = observation.cold_boot();
    let recovered = durable::recover(&mut rebooted, identity()).unwrap();
    assert_eq!(rebooted.observe().media(), media, "recovery wrote media");
    assert!(
        rebooted
            .observe()
            .trace()
            .iter()
            .all(|operation| matches!(operation, io_operation::read(_)))
    );
    (recovered.version(), recovered.text().to_vec())
}

fn successful_save_trace(write_through: bool) -> Vec<io_operation> {
    with_accepted_store(write_through, |store, observation, request| {
        observation.arm(None);
        let reply = store.execute(authority(), &request).unwrap();
        assert_eq!(reply.kind(), operation::saved);
        assert_eq!(reply.document(), request.document());
        assert_eq!(reply.generation(), request.generation());
        assert_eq!(reply.request(), request.request());
        assert_eq!(reply.base_version(), 2);
        assert_eq!(reply.version(), 3);
        assert_eq!(reply.revision(), 4);
        assert_eq!(reply.text(), candidate);
        assert!(!store.is_quarantined());
        assert_eq!(recover_semantics(&observation), (3, candidate.to_vec()));
        let trace = observation.trace();
        assert_eq!(trace.last(), Some(&io_operation::flush));
        assert!(trace.iter().any(|op| matches!(op, io_operation::write(_))));
        trace
    })
}

#[derive(Clone, Copy)]
enum recovery_expectation {
    old,
    new,
    old_or_new,
}

fn save_expectation(
    trace: &[io_operation],
    failure: failure,
    write_through: bool,
) -> recovery_expectation {
    // A commit write follows the body flush. The old checkpoint is mandatory
    // before that commit can reach media, not merely one permitted answer.
    let commit = trace
        .iter()
        .rposition(|op| matches!(op, io_operation::write(_)))
        .unwrap()
        + 1;
    if failure.operation < commit
        || (failure.operation == commit && matches!(failure.effect, failure_effect::before))
    {
        return recovery_expectation::old;
    }
    if failure.operation == trace.len()
        && matches!(
            failure.effect,
            failure_effect::after | failure_effect::flush_prefix(sector_size)
        )
    {
        return recovery_expectation::new;
    }
    if failure.operation == commit {
        match failure.effect {
            failure_effect::torn_prefix(sector_size) => return recovery_expectation::new,
            failure_effect::after if write_through => return recovery_expectation::new,
            failure_effect::after => return recovery_expectation::old,
            _ => {}
        }
    }
    if write_through && failure.operation > commit {
        return recovery_expectation::new;
    }
    if !write_through
        && failure.operation == trace.len()
        && matches!(failure.effect, failure_effect::before)
    {
        return recovery_expectation::old;
    }
    recovery_expectation::old_or_new
}

fn failed_save(write_through: bool, failure: failure, expected: recovery_expectation) {
    with_accepted_store(write_through, |store, observation, request| {
        observation.arm(Some(failure));
        assert_eq!(
            store.execute(authority(), &request),
            Err(durable_error::outcome_unknown),
            "failure {failure:?}, write_through {write_through}"
        );
        assert!(store.is_quarantined());
        assert_eq!(store.version(), 2);
        assert_eq!(store.stored_text(), accepted);

        let trace = observation.trace();
        let media = observation.media();
        let volatile = observation.volatile();
        for probe in [
            request,
            record::read(store.document(), request.generation(), 3).unwrap(),
            record::close(store.document(), request.generation(), 4).unwrap(),
            record::open(store.document(), 5).unwrap(),
        ] {
            assert_eq!(
                store.execute(authority(), &probe),
                Err(durable_error::quarantined)
            );
        }
        assert_eq!(observation.trace(), trace, "quarantine issued more I/O");
        assert_eq!(observation.media(), media);
        assert_eq!(observation.volatile(), volatile);

        let recovered = recover_semantics(&observation);
        let old = (2, accepted.to_vec());
        let new = (3, candidate.to_vec());
        match expected {
            recovery_expectation::old => {
                assert_eq!(
                    recovered, old,
                    "uncommitted candidate replaced acknowledged bytes"
                );
            }
            recovery_expectation::new => {
                assert_eq!(recovered, new, "completed commit lost its snapshot");
            }
            recovery_expectation::old_or_new => assert!(
                recovered == old || recovered == new,
                "failure {failure:?}, write_through {write_through}, recovered {recovered:?}"
            ),
        }
    });
}

#[test]
fn acknowledged_save_survives_fresh_media_and_service_state() {
    for write_through in [false, true] {
        successful_save_trace(write_through);
    }
}

#[test]
fn failure_before_and_after_every_save_operation_never_invents_a_snapshot() {
    for write_through in [false, true] {
        let trace = successful_save_trace(write_through);
        for (index, _) in trace.iter().enumerate() {
            for effect in [failure_effect::before, failure_effect::after] {
                let failure = failure {
                    operation: index + 1,
                    effect,
                };
                failed_save(
                    write_through,
                    failure,
                    save_expectation(&trace, failure, write_through),
                );
            }
        }
    }
}

#[test]
fn every_torn_prefix_of_each_reused_slot_write_recovers_only_old_or_new() {
    for write_through in [false, true] {
        let trace = successful_save_trace(write_through);
        for (index, operation) in trace.iter().enumerate() {
            if !matches!(operation, io_operation::write(_)) {
                continue;
            }
            for prefix in 0..=sector_size {
                let failure = failure {
                    operation: index + 1,
                    effect: failure_effect::torn_prefix(prefix),
                };
                failed_save(
                    write_through,
                    failure,
                    save_expectation(&trace, failure, write_through),
                );
            }
        }
    }
}

#[test]
fn partially_persisted_failed_flush_never_acknowledges_unknown_bytes() {
    for write_through in [false, true] {
        let trace = successful_save_trace(write_through);
        for (index, operation) in trace.iter().enumerate() {
            if *operation != io_operation::flush {
                continue;
            }
            for prefix in 0..=sector_size {
                let failure = failure {
                    operation: index + 1,
                    effect: failure_effect::flush_prefix(prefix),
                };
                failed_save(
                    write_through,
                    failure,
                    save_expectation(&trace, failure, write_through),
                );
            }
        }
    }
}

fn format_trial(
    write_through: bool,
    failure: Option<failure>,
    require_valid: bool,
) -> Vec<io_operation> {
    let mut disk = model_disk::blank(write_through);
    let observation = disk.observe();
    observation.arm(failure);
    let result = durable::format_disposable(&mut disk, identity(), original);
    let trace = observation.trace();
    assert_eq!(result.is_err(), failure.is_some());
    let mut rebooted = observation.cold_boot();
    let recovered = durable::recover(&mut rebooted, identity());
    match recovered {
        Ok(document) => {
            assert_eq!(document.version(), 1);
            assert_eq!(document.epoch(), 0);
            assert_eq!(document.text(), original);
        }
        Err(error) => {
            assert!(failure.is_some());
            assert!(
                !require_valid,
                "a completed final flush lost the initial document"
            );
            assert!(matches!(
                error,
                durable_error::invalid_header | durable_error::no_valid_checkpoint
            ));
        }
    }
    trace
}

#[test]
fn interrupted_format_never_recovers_invented_content() {
    for write_through in [false, true] {
        let trace = format_trial(write_through, None, true);
        for (index, operation) in trace.iter().enumerate() {
            for effect in [failure_effect::before, failure_effect::after] {
                format_trial(
                    write_through,
                    Some(failure {
                        operation: index + 1,
                        effect,
                    }),
                    index + 1 == trace.len() && matches!(effect, failure_effect::after),
                );
            }
            if matches!(operation, io_operation::write(_)) {
                for prefix in 0..=sector_size {
                    format_trial(
                        write_through,
                        Some(failure {
                            operation: index + 1,
                            effect: failure_effect::torn_prefix(prefix),
                        }),
                        false,
                    );
                }
            } else if *operation == io_operation::flush {
                for prefix in 0..=sector_size {
                    format_trial(
                        write_through,
                        Some(failure {
                            operation: index + 1,
                            effect: failure_effect::flush_prefix(prefix),
                        }),
                        index + 1 == trace.len() && prefix == sector_size,
                    );
                }
            }
        }
    }
}

fn mount_trial(
    write_through: bool,
    failure: Option<failure>,
    require_advance: bool,
) -> Vec<io_operation> {
    let media = with_accepted_store(write_through, |_, observation, _| observation.media());
    let mut disk = model_disk::from_media(media, write_through);
    let observation = disk.observe();
    let before = durable::recover(&mut disk, identity()).unwrap();
    observation.arm(failure);
    {
        let mounted = durable_store::mount(&mut disk, identity(), successor());
        assert_eq!(mounted.is_err(), failure.is_some());
        if let Ok(store) = mounted {
            assert_eq!(store.document(), before.epoch() + 1);
            assert_eq!(store.version(), 2);
            assert_eq!(store.stored_text(), accepted);
        }
    }
    let trace = observation.trace();
    let mut rebooted = observation.cold_boot();
    let recovered = durable::recover(&mut rebooted, identity()).unwrap();
    assert_eq!(recovered.version(), 2);
    assert_eq!(recovered.text(), accepted);
    assert!(recovered.epoch() == before.epoch() || recovered.epoch() == before.epoch() + 1);
    assert!(
        recovered.sequence() == before.sequence() || recovered.sequence() == before.sequence() + 1
    );
    if require_advance {
        assert_eq!(recovered.epoch(), before.epoch() + 1);
        assert_eq!(recovered.sequence(), before.sequence() + 1);
    }
    trace
}

#[test]
fn interrupted_mount_preserves_content_and_never_resurrects_a_service_epoch() {
    for write_through in [false, true] {
        let trace = mount_trial(write_through, None, true);
        for (index, operation) in trace.iter().enumerate() {
            for effect in [failure_effect::before, failure_effect::after] {
                mount_trial(
                    write_through,
                    Some(failure {
                        operation: index + 1,
                        effect,
                    }),
                    index + 1 == trace.len() && matches!(effect, failure_effect::after),
                );
            }
            if matches!(operation, io_operation::write(_)) {
                for prefix in 0..=sector_size {
                    mount_trial(
                        write_through,
                        Some(failure {
                            operation: index + 1,
                            effect: failure_effect::torn_prefix(prefix),
                        }),
                        false,
                    );
                }
            } else if *operation == io_operation::flush {
                for prefix in 0..=sector_size {
                    mount_trial(
                        write_through,
                        Some(failure {
                            operation: index + 1,
                            effect: failure_effect::flush_prefix(prefix),
                        }),
                        index + 1 == trace.len() && prefix == sector_size,
                    );
                }
            }
        }
    }
}

#[test]
fn fresh_service_reads_durable_bytes_and_rejects_previous_epoch_and_authority() {
    for write_through in [false, true] {
        let (observation, stale) = with_accepted_store(write_through, |_, observation, request| {
            (observation, request)
        });
        let mut rebooted = observation.cold_boot();
        let successor_observation = rebooted.observe();
        {
            let mut store = durable_store::mount(&mut rebooted, identity(), successor()).unwrap();
            assert!(store.document() > stale.document());
            assert_eq!(store.version(), 2);
            assert_eq!(store.stored_text(), accepted);
            assert!(!store.is_open());
            let opened = store
                .execute(successor(), &record::open(store.document(), 1).unwrap())
                .unwrap();
            assert_eq!(opened.text(), accepted);
            assert_eq!(opened.version(), 2);
            successor_observation.arm(None);
            assert_eq!(
                store.execute(authority(), &stale),
                Err(durable_error::rejected(service_error::wrong_authority))
            );
            assert_eq!(
                store.execute(successor(), &stale),
                Err(durable_error::rejected(service_error::wrong_document))
            );
            assert!(successor_observation.trace().is_empty());
            let next =
                record::save(store.document(), opened.generation(), 1, 2, 2, replacement).unwrap();
            let reply = store.execute(successor(), &next).unwrap();
            assert_eq!(reply.version(), 3);
            assert_eq!(reply.text(), replacement);
        }
        assert_eq!(
            recover_semantics(&successor_observation),
            (3, replacement.to_vec())
        );
        assert_eq!(recover_semantics(&observation), (2, accepted.to_vec()));
    }
}

#[test]
fn semantic_rejections_do_not_issue_io_or_change_saved_state() {
    with_accepted_store(false, |store, observation, valid| {
        let generation = valid.generation();
        let document = valid.document();
        let other = client_identity::try_new(2, 1).unwrap();
        let probes = [
            (other, valid, service_error::wrong_authority),
            (
                authority(),
                record::read(document + 1, generation, 3).unwrap(),
                service_error::wrong_document,
            ),
            (
                authority(),
                record::read(document, generation + 1, 3).unwrap(),
                service_error::stale_handle,
            ),
            (
                authority(),
                record::save(document, generation, 2, 9, 4, candidate).unwrap(),
                service_error::version_conflict,
            ),
            (
                authority(),
                record::save(document, generation, 1, 2, 4, candidate).unwrap(),
                service_error::stale_request,
            ),
            (
                authority(),
                record::open(document, 3).unwrap(),
                service_error::already_open,
            ),
        ];
        let media = observation.media();
        let volatile = observation.volatile();
        observation.arm(None);
        for (caller, request, error) in probes {
            assert_eq!(
                store.execute(caller, &request),
                Err(durable_error::rejected(error))
            );
            assert_eq!(store.version(), 2);
            assert_eq!(store.stored_text(), accepted);
            assert!(store.is_open());
            assert!(!store.is_quarantined());
        }
        store
            .execute(
                authority(),
                &record::close(document, generation, 4).unwrap(),
            )
            .unwrap();
        assert_eq!(
            store.execute(authority(), &record::read(document, generation, 5).unwrap()),
            Err(durable_error::rejected(service_error::closed_handle))
        );
        assert!(observation.trace().is_empty());
        assert_eq!(observation.media(), media);
        assert_eq!(observation.volatile(), volatile);
    });
}

#[test]
fn formatter_rejects_every_nonblank_byte_without_a_write_or_flush() {
    for offset in 0..sector_count * sector_size {
        let mut media = [[0; sector_size]; sector_count];
        media[offset / sector_size][offset % sector_size] = 1;
        let mut disk = model_disk::from_media(media, false);
        let observation = disk.observe();
        assert_eq!(
            durable::format_disposable(&mut disk, identity(), original),
            Err(durable_error::not_blank)
        );
        assert_eq!(observation.media(), media);
        assert_eq!(observation.volatile(), media);
        assert!(
            observation
                .trace()
                .iter()
                .all(|operation| matches!(operation, io_operation::read(_)))
        );
    }
}

#[test]
fn wrong_volume_or_object_never_mounts_or_rewrites_an_existing_disk() {
    let media = with_accepted_store(false, |_, observation, _| observation.media());
    for wrong in [
        disk_identity::try_new(0x4412, 0xd201).unwrap(),
        disk_identity::try_new(0x4411, 0xd202).unwrap(),
    ] {
        let mut disk = model_disk::from_media(media, false);
        let observation = disk.observe();
        assert!(durable::recover(&mut disk, wrong).is_err());
        assert!(durable_store::mount(&mut disk, wrong, authority()).is_err());
        assert_eq!(observation.media(), media);
        assert_eq!(observation.volatile(), media);
        assert!(
            observation
                .trace()
                .iter()
                .all(|operation| matches!(operation, io_operation::read(_)))
        );
    }
}
