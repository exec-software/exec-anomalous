//! Experimental single-document persistence policy, not a filesystem or driver.
//!
//! The caller exclusively grants a newly created disposable five-sector extent.
//! Only `format_disposable` formats it, and only if every byte is zero. Recovery
//! never formats or falls back to launch seed text. Sector zero is an immutable
//! header; slots zero/one use body/commit sectors 1/2 and 3/4. All integers are
//! little-endian; every sector is exactly 512 bytes and unused bytes are zero.
//!
//! Common offsets: magic[0..8], version:u16@8=1, size:u16@10=512,
//! volume:u64@16, object:u64@24, CRC32C:u32@508 over bytes[0..508]. Header
//! count:u32@12=5. Body/commit: slot:u64@32, sequence:u64@40, epoch:u64@48,
//! content_version:u64@56. Body length:u8@64, ASCII text[65..96]. Commit has
//! the body's stored CRC32C:u32@64, binding its matching complete body.
//! Magic is `ANDURHDR`, `ANDURBOD`, or `ANDURCMT`. CRC32C is corruption detection,
//! not authentication, collision resistance, authorization or anti-rollback.
//!
//! Formatting starts at sequence1/epoch0/version1. Each mount advances epoch;
//! each save advances content version; either advances sequence exactly once.
//! Thus sequence=epoch+version and slot=(sequence-1)%2. Mount durably publishes
//! its new epoch before granting any handles. The existing document wire uses
//! this epoch as its incarnation, independently of the persistent object key.
//!
//! Updates write only the inactive body, flush, write its commit, then flush.
//! A saved reply is released only afterward. The previous active pair remains
//! untouched. Any write/flush error is conservatively unknown and quarantines
//! the mounted store. Cached prior bytes are then not a recovery result.
//!
//! Crash guarantees assume completed flushes persist all preceding writes,
//! failures affect only requested sectors, and torn/corrupt records are detected
//! by the checksum. No atomic-sector guarantee is assumed. Byzantine devices,
//! checksum collisions, disk rollback and physical media failure are excluded.
//! All document/format policy remains userspace; numbers read from disk never
//! grant task, route or device authority. No real device integration is present.

use crate::{client_identity, document_store, operation, record, service_error, text_capacity};

pub const sector_size: usize = 512;
pub const sector_count: usize = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum io_error {
    failed,
}

/// Exclusive I/O to the granted disposable extent, using relative sectors.
///
/// A successful flush must make all preceding completed writes durable. A
/// failed operation may already have changed the device, including partially.
/// Implementors must not alias this extent through another live writer. Reads
/// return exactly one sector; the policy never accesses a sector outside 0..5.
pub trait sector_io {
    fn read_sector(&mut self, sector: u64, out: &mut [u8; sector_size]) -> Result<(), io_error>;
    fn write_sector(&mut self, sector: u64, bytes: &[u8; sector_size]) -> Result<(), io_error>;
    fn flush(&mut self) -> Result<(), io_error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum durable_error {
    invalid_identity,
    invalid_text,
    not_blank,
    invalid_header,
    no_valid_checkpoint,
    ambiguous_checkpoint,
    sequence_exhausted,
    epoch_exhausted,
    read_failed,
    outcome_unknown,
    quarantined,
    rejected(service_error),
}

/// Trusted expected identity of the granted extent, not a capability by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct disk_identity {
    volume: u64,
    object: u64,
}

impl disk_identity {
    pub fn try_new(volume: u64, object: u64) -> Result<Self, durable_error> {
        if volume == 0 || object == 0 {
            return Err(durable_error::invalid_identity);
        }
        Ok(Self { volume, object })
    }

    pub const fn volume(&self) -> u64 {
        self.volume
    }

    pub const fn object(&self) -> u64 {
        self.object
    }
}

/// Inert recovered content. It cannot restore an old open handle or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct recovered_document {
    sequence: u64,
    epoch: u64,
    version: u64,
    text: [u8; text_capacity],
    length: usize,
}

impl recovered_document {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn text(&self) -> &[u8] {
        &self.text[..self.length]
    }

    fn slot(&self) -> u64 {
        (self.sequence - 1) % 2
    }
}

/// Explicitly initialize only a blank, separately provisioned disposable disk.
/// A failed format may leave an incomplete image; discard it explicitly rather
/// than formatting it again or treating it as an empty recovered document.
pub fn format_disposable<I: sector_io + ?Sized>(
    io: &mut I,
    identity: disk_identity,
    initial: &[u8],
) -> Result<(), durable_error> {
    crate::validate_text(initial).map_err(|_| durable_error::invalid_text)?;
    let mut sector = [0; sector_size];
    for index in 0..sector_count as u64 {
        io.read_sector(index, &mut sector)
            .map_err(|_| durable_error::read_failed)?;
        if sector.iter().any(|byte| *byte != 0) {
            return Err(durable_error::not_blank);
        }
    }
    let mut text = [0; text_capacity];
    text[..initial.len()].copy_from_slice(initial);
    let initial = recovered_document {
        sequence: 1,
        epoch: 0,
        version: 1,
        text,
        length: initial.len(),
    };
    io.write_sector(0, &header(identity))
        .map_err(|_| durable_error::outcome_unknown)?;
    publish(io, identity, &initial)
}

/// Read both independently checked slot pairs without writing any device byte.
/// Under the stated crash model, one torn inactive pair may be discarded. Two
/// complete pairs must describe consecutive legal transitions; inconsistent
/// but checksummed images fail closed rather than selecting an arbitrary winner.
pub fn recover<I: sector_io + ?Sized>(
    io: &mut I,
    identity: disk_identity,
) -> Result<recovered_document, durable_error> {
    let mut bytes = [0; sector_size];
    io.read_sector(0, &mut bytes)
        .map_err(|_| durable_error::read_failed)?;
    if bytes != header(identity) {
        return Err(durable_error::invalid_header);
    }
    let mut pairs = [None; 2];
    let mut commit = [0; sector_size];
    for (slot, pair) in pairs.iter_mut().enumerate() {
        io.read_sector(1 + slot as u64 * 2, &mut bytes)
            .map_err(|_| durable_error::read_failed)?;
        io.read_sector(2 + slot as u64 * 2, &mut commit)
            .map_err(|_| durable_error::read_failed)?;
        *pair = decode_pair(identity, slot as u64, &bytes, &commit);
    }
    match pairs {
        [Some(left), Some(right)] => {
            let (older, newer) = if left.sequence < right.sequence {
                (left, right)
            } else {
                (right, left)
            };
            let mounted = older.epoch.checked_add(1) == Some(newer.epoch)
                && older.version == newer.version
                && older.text() == newer.text();
            let saved =
                older.epoch == newer.epoch && older.version.checked_add(1) == Some(newer.version);
            if older.sequence.checked_add(1) != Some(newer.sequence) || !(mounted || saved) {
                return Err(durable_error::ambiguous_checkpoint);
            }
            Ok(newer)
        }
        [Some(value), None] | [None, Some(value)] => Ok(value),
        [None, None] => Err(durable_error::no_valid_checkpoint),
    }
}

/// One mounted writer, retaining the exclusive I/O borrow for its whole life.
/// This is neither Clone nor Copy. Drop it before remount/recovery. Existing
/// task/channel authentication still supplies each request's separate caller.
pub struct durable_store<'a, I: sector_io + ?Sized> {
    io: &'a mut I,
    identity: disk_identity,
    checkpoint: recovered_document,
    ram: document_store,
    quarantined: bool,
}

impl<'a, I: sector_io + ?Sized> durable_store<'a, I> {
    pub fn mount(
        io: &'a mut I,
        identity: disk_identity,
        authority: client_identity,
    ) -> Result<Self, durable_error> {
        let mut checkpoint = recover(io, identity)?;
        checkpoint.sequence = checkpoint
            .sequence
            .checked_add(1)
            .ok_or(durable_error::sequence_exhausted)?;
        checkpoint.epoch = checkpoint
            .epoch
            .checked_add(1)
            .ok_or(durable_error::epoch_exhausted)?;
        publish(io, identity, &checkpoint)?;
        let ram = document_store {
            authority,
            document: checkpoint.epoch,
            next_generation: 1,
            generation: 0,
            open: false,
            version: checkpoint.version,
            last_save_request: 0,
            text: checkpoint.text,
            length: checkpoint.length,
        };
        Ok(Self {
            io,
            identity,
            checkpoint,
            ram,
            quarantined: false,
        })
    }

    /// Semantic rejection performs no I/O and changes no state. A save first
    /// runs all existing RAM validation on a private candidate, then publishes
    /// the candidate durably, then installs it and exposes its exact reply.
    /// `outcome_unknown` must not become a negative save acknowledgement. Even a
    /// stale-request rejection is not evidence that an earlier attempt failed.
    pub fn execute(
        &mut self,
        authority: client_identity,
        request: &record,
    ) -> Result<record, durable_error> {
        if self.quarantined {
            return Err(durable_error::quarantined);
        }
        let mut candidate = copy_ram(&self.ram);
        let reply = candidate
            .execute(authority, request)
            .map_err(durable_error::rejected)?;
        if request.kind() == operation::save {
            let checkpoint = recovered_document {
                sequence: self
                    .checkpoint
                    .sequence
                    .checked_add(1)
                    .ok_or(durable_error::sequence_exhausted)?,
                epoch: self.checkpoint.epoch,
                version: candidate.version,
                text: candidate.text,
                length: candidate.length,
            };
            if let Err(error) = publish(self.io, self.identity, &checkpoint) {
                self.quarantined = true;
                return Err(error);
            }
            self.checkpoint = checkpoint;
        }
        self.ram = candidate;
        Ok(reply)
    }

    pub const fn document(&self) -> u64 {
        self.ram.document
    }

    pub const fn version(&self) -> u64 {
        self.ram.version
    }

    /// Last published in-memory content; after quarantine it is not a claim
    /// about the newest on-disk checkpoint. Drop and recover to resolve that.
    pub fn stored_text(&self) -> &[u8] {
        self.ram.stored_text()
    }

    pub const fn is_open(&self) -> bool {
        self.ram.open
    }

    pub const fn is_quarantined(&self) -> bool {
        self.quarantined
    }

    pub const fn sequence(&self) -> u64 {
        self.checkpoint.sequence
    }
}

fn copy_ram(store: &document_store) -> document_store {
    document_store {
        authority: store.authority,
        document: store.document,
        next_generation: store.next_generation,
        generation: store.generation,
        open: store.open,
        version: store.version,
        last_save_request: store.last_save_request,
        text: store.text,
        length: store.length,
    }
}

fn publish<I: sector_io + ?Sized>(
    io: &mut I,
    identity: disk_identity,
    value: &recovered_document,
) -> Result<(), durable_error> {
    let body = encode_body(identity, value);
    let commit = encode_commit(identity, value, read_u32(&body, 508));
    io.write_sector(1 + value.slot() * 2, &body)
        .map_err(|_| durable_error::outcome_unknown)?;
    io.flush().map_err(|_| durable_error::outcome_unknown)?;
    io.write_sector(2 + value.slot() * 2, &commit)
        .map_err(|_| durable_error::outcome_unknown)?;
    io.flush().map_err(|_| durable_error::outcome_unknown)
}

fn common(magic: &[u8; 8], identity: disk_identity) -> [u8; sector_size] {
    let mut bytes = [0; sector_size];
    bytes[..8].copy_from_slice(magic);
    bytes[8..12].copy_from_slice(&[1, 0, 0, 2]);
    put_u64(&mut bytes, 16, identity.volume);
    put_u64(&mut bytes, 24, identity.object);
    bytes
}

fn header(identity: disk_identity) -> [u8; sector_size] {
    let mut bytes = common(b"ANDURHDR", identity);
    bytes[12..16].copy_from_slice(&(sector_count as u32).to_le_bytes());
    seal(&mut bytes);
    bytes
}

fn checkpoint_fields(bytes: &mut [u8; sector_size], value: &recovered_document) {
    put_u64(bytes, 32, value.slot());
    put_u64(bytes, 40, value.sequence);
    put_u64(bytes, 48, value.epoch);
    put_u64(bytes, 56, value.version);
}

fn encode_body(identity: disk_identity, value: &recovered_document) -> [u8; sector_size] {
    let mut bytes = common(b"ANDURBOD", identity);
    checkpoint_fields(&mut bytes, value);
    bytes[64] = value.length as u8;
    bytes[65..65 + value.length].copy_from_slice(value.text());
    seal(&mut bytes);
    bytes
}

fn encode_commit(
    identity: disk_identity,
    value: &recovered_document,
    body_crc: u32,
) -> [u8; sector_size] {
    let mut bytes = common(b"ANDURCMT", identity);
    checkpoint_fields(&mut bytes, value);
    bytes[64..68].copy_from_slice(&body_crc.to_le_bytes());
    seal(&mut bytes);
    bytes
}

fn decode_pair(
    identity: disk_identity,
    slot: u64,
    body: &[u8; sector_size],
    commit: &[u8; sector_size],
) -> Option<recovered_document> {
    let sequence = read_u64(body, 40);
    let epoch = read_u64(body, 48);
    let version = read_u64(body, 56);
    let length = usize::from(body[64]);
    if sequence == 0
        || version == 0
        || epoch.checked_add(version) != Some(sequence)
        || (epoch == 0 && (sequence != 1 || version != 1))
        || length > text_capacity
        || (sequence - 1) % 2 != slot
    {
        return None;
    }
    let text_bytes = &body[65..65 + length];
    crate::validate_text(text_bytes).ok()?;
    let mut text = [0; text_capacity];
    text[..length].copy_from_slice(text_bytes);
    let value = recovered_document {
        sequence,
        epoch,
        version,
        text,
        length,
    };
    // Re-encoding checks every byte: magic, identity, slot, reserved padding and
    // both checksums, not just a decoded prefix. The commit binds this exact body.
    if *body != encode_body(identity, &value)
        || *commit != encode_commit(identity, &value, read_u32(body, 508))
    {
        return None;
    }
    Some(value)
}

fn put_u64(bytes: &mut [u8; sector_size], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_u64(bytes: &[u8; sector_size], offset: usize) -> u64 {
    let mut value = [0; 8];
    value.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(value)
}

fn read_u32(bytes: &[u8; sector_size], offset: usize) -> u32 {
    let mut value = [0; 4];
    value.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(value)
}

fn seal(bytes: &mut [u8; sector_size]) {
    let value = crc32c(&bytes[..508]);
    bytes[508..].copy_from_slice(&value.to_le_bytes());
}

fn crc32c(bytes: &[u8]) -> u32 {
    let mut value = !0u32;
    for byte in bytes {
        value ^= u32::from(*byte);
        for _ in 0..8 {
            value = (value >> 1) ^ (0x82f6_3b78 & 0u32.wrapping_sub(value & 1));
        }
    }
    !value
}

#[cfg(test)]
mod tests {
    extern crate std;

    use self::std::vec::Vec;
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum event {
        read(u64),
        write(u64),
        flush,
    }

    struct memory_io {
        bytes: [[u8; sector_size]; sector_count],
        events: Vec<event>,
        fail_at: Option<usize>,
    }

    impl memory_io {
        fn new() -> Self {
            Self {
                bytes: [[0; sector_size]; sector_count],
                events: Vec::new(),
                fail_at: None,
            }
        }

        fn record(&mut self, value: event) -> Result<(), io_error> {
            self.events.push(value);
            if self.fail_at == Some(self.events.len()) {
                Err(io_error::failed)
            } else {
                Ok(())
            }
        }
    }

    impl sector_io for memory_io {
        fn read_sector(
            &mut self,
            sector: u64,
            out: &mut [u8; sector_size],
        ) -> Result<(), io_error> {
            self.record(event::read(sector))?;
            *out = self.bytes[sector as usize];
            Ok(())
        }

        fn write_sector(&mut self, sector: u64, bytes: &[u8; sector_size]) -> Result<(), io_error> {
            self.record(event::write(sector))?;
            self.bytes[sector as usize] = *bytes;
            Ok(())
        }

        fn flush(&mut self) -> Result<(), io_error> {
            self.record(event::flush)
        }
    }

    fn identity() -> disk_identity {
        disk_identity::try_new(0x1234, 0x5678).unwrap()
    }

    fn authority() -> client_identity {
        client_identity::try_new(3, 7).unwrap()
    }

    fn formatted() -> memory_io {
        let mut io = memory_io::new();
        format_disposable(&mut io, identity(), b"const before = 1;").unwrap();
        io.events.clear();
        io
    }

    fn opened(io: &mut memory_io) -> durable_store<'_, memory_io> {
        let mut store = durable_store::mount(io, identity(), authority()).unwrap();
        let reply = store
            .execute(authority(), &record::open(store.document(), 1).unwrap())
            .unwrap();
        assert_eq!(reply.generation(), 1);
        store
    }

    fn save(store: &durable_store<'_, memory_io>) -> record {
        record::save(
            store.document(),
            1,
            1,
            store.version(),
            2,
            b"const after = 1;",
        )
        .unwrap()
    }

    fn fixture(sequence: u64, epoch: u64, version: u64, bytes: &[u8]) -> recovered_document {
        let mut text = [0; text_capacity];
        text[..bytes.len()].copy_from_slice(bytes);
        recovered_document {
            sequence,
            epoch,
            version,
            text,
            length: bytes.len(),
        }
    }

    fn install(io: &mut memory_io, value: &recovered_document) {
        let body = encode_body(identity(), value);
        io.bytes[1 + value.slot() as usize * 2] = body;
        io.bytes[2 + value.slot() as usize * 2] =
            encode_commit(identity(), value, read_u32(&body, 508));
    }

    #[test]
    fn crc32c_matches_standard_check_value_and_empty_input() {
        assert_eq!(crc32c(b"123456789"), 0xe306_9283);
        assert_eq!(crc32c(b""), 0);
    }

    #[test]
    fn formatting_is_explicit_blank_only_and_validates_before_writes() {
        assert_eq!(
            disk_identity::try_new(0, 1),
            Err(durable_error::invalid_identity)
        );
        assert_eq!(
            disk_identity::try_new(1, 0),
            Err(durable_error::invalid_identity)
        );
        let mut io = memory_io::new();
        assert_eq!(
            format_disposable(&mut io, identity(), b"bad\n"),
            Err(durable_error::invalid_text)
        );
        assert!(io.events.is_empty());
        io.bytes[4][511] = 1;
        let before = io.bytes;
        assert_eq!(
            format_disposable(&mut io, identity(), b"initial"),
            Err(durable_error::not_blank)
        );
        assert_eq!(io.bytes, before);
        assert!(
            io.events
                .iter()
                .all(|value| matches!(value, event::read(_)))
        );
        let mut io = formatted();
        let before = io.bytes;
        assert_eq!(
            format_disposable(&mut io, identity(), b"overwrite"),
            Err(durable_error::not_blank)
        );
        assert_eq!(io.bytes, before);
    }

    #[test]
    fn save_reply_follows_ordered_commit_and_remount_revokes_old_incarnation() {
        let mut io = formatted();
        let old_document;
        {
            let mut store = opened(&mut io);
            old_document = store.document();
            assert_eq!((old_document, store.sequence(), store.version()), (1, 2, 1));
            let request = save(&store);
            let start = store.io.events.len();
            let reply = store.execute(authority(), &request).unwrap();
            assert_eq!(reply.kind(), operation::saved);
            assert_eq!(reply.text(), request.text());
            assert_eq!(
                (
                    reply.request(),
                    reply.revision(),
                    reply.base_version(),
                    reply.version()
                ),
                (1, 2, 1, 2)
            );
            assert_eq!(
                &store.io.events[start..],
                &[event::write(1), event::flush, event::write(2), event::flush]
            );
            assert_eq!((store.sequence(), store.version()), (3, 2));
            let count = store.io.events.len();
            store
                .execute(authority(), &record::close(old_document, 1, 2).unwrap())
                .unwrap();
            assert_eq!(store.io.events.len(), count);
        }
        let recovered = recover(&mut io, identity()).unwrap();
        assert_eq!(
            (recovered.sequence(), recovered.epoch(), recovered.version()),
            (3, 1, 2)
        );
        assert_eq!(recovered.text(), b"const after = 1;");
        let mut store = durable_store::mount(&mut io, identity(), authority()).unwrap();
        assert_eq!(
            (store.document(), store.sequence(), store.version()),
            (2, 4, 2)
        );
        let count = store.io.events.len();
        let old = record::save(old_document, 1, 2, 2, 3, b"stale").unwrap();
        assert_eq!(
            store.execute(authority(), &old),
            Err(durable_error::rejected(service_error::wrong_document))
        );
        assert_eq!(store.io.events.len(), count);
        let open = store
            .execute(authority(), &record::open(store.document(), 1).unwrap())
            .unwrap();
        assert_eq!((open.generation(), open.version()), (1, 2));
        assert_eq!(open.text(), b"const after = 1;");
    }

    #[test]
    fn every_semantic_rejection_preserves_ram_checkpoint_and_device() {
        let mut io = formatted();
        let mut store = opened(&mut io);
        let good = save(&store);
        for (caller, request, error) in [
            (
                client_identity::try_new(3, 8).unwrap(),
                good,
                service_error::wrong_authority,
            ),
            (
                authority(),
                record::save(99, 1, 1, 1, 2, b"x").unwrap(),
                service_error::wrong_document,
            ),
            (
                authority(),
                record::save(store.document(), 2, 1, 1, 2, b"x").unwrap(),
                service_error::stale_handle,
            ),
            (
                authority(),
                record::save(store.document(), 1, 1, 2, 2, b"x").unwrap(),
                service_error::version_conflict,
            ),
            (
                authority(),
                record::open(store.document(), 2).unwrap(),
                service_error::already_open,
            ),
        ] {
            let before_ram = copy_ram(&store.ram);
            let before_checkpoint = store.checkpoint;
            let before_bytes = store.io.bytes;
            let count = store.io.events.len();
            assert_eq!(
                store.execute(caller, &request),
                Err(durable_error::rejected(error))
            );
            assert_eq!(store.ram, before_ram);
            assert_eq!(store.checkpoint, before_checkpoint);
            assert_eq!(store.io.bytes, before_bytes);
            assert_eq!(store.io.events.len(), count);
        }
        store.execute(authority(), &good).unwrap();
        let count = store.io.events.len();
        assert_eq!(
            store.execute(authority(), &good),
            Err(durable_error::rejected(service_error::stale_request))
        );
        assert_eq!(store.io.events.len(), count);
    }

    #[test]
    fn every_update_io_error_is_unknown_and_quarantines_without_ram_publication() {
        for failure in 1..=4 {
            let mut io = formatted();
            let mut store = opened(&mut io);
            let request = save(&store);
            let before_ram = copy_ram(&store.ram);
            let before_checkpoint = store.checkpoint;
            store.io.fail_at = Some(store.io.events.len() + failure);
            assert_eq!(
                store.execute(authority(), &request),
                Err(durable_error::outcome_unknown)
            );
            assert!(store.is_quarantined());
            assert_eq!(store.ram, before_ram);
            assert_eq!(store.checkpoint, before_checkpoint);
            let count = store.io.events.len();
            for request in [
                request,
                record::read(store.document(), 1, 2).unwrap(),
                record::close(store.document(), 1, 3).unwrap(),
            ] {
                assert_eq!(
                    store.execute(authority(), &request),
                    Err(durable_error::quarantined)
                );
            }
            assert_eq!(store.io.events.len(), count);
        }
    }

    #[test]
    fn body_and_commit_are_fully_canonical_and_bind_expected_identity() {
        let value = fixture(3, 1, 2, b"hello");
        let body = encode_body(identity(), &value);
        let commit = encode_commit(identity(), &value, read_u32(&body, 508));
        assert_eq!(decode_pair(identity(), 0, &body, &commit), Some(value));
        for offset in 0..sector_size {
            let mut bad_body = body;
            bad_body[offset] ^= 1;
            assert_eq!(decode_pair(identity(), 0, &bad_body, &commit), None);
            let mut bad_commit = commit;
            bad_commit[offset] ^= 1;
            assert_eq!(decode_pair(identity(), 0, &body, &bad_commit), None);
        }
        for offset in [12, 13, 14, 15, 70, 96, 507] {
            let mut bad_body = body;
            bad_body[offset] = 1;
            seal(&mut bad_body);
            let matched_commit = encode_commit(identity(), &value, read_u32(&bad_body, 508));
            assert_eq!(decode_pair(identity(), 0, &bad_body, &matched_commit), None);
        }
        let foreign = disk_identity::try_new(identity().volume() + 1, identity().object()).unwrap();
        let foreign_body = encode_body(foreign, &value);
        let foreign_commit = encode_commit(foreign, &value, read_u32(&foreign_body, 508));
        assert_eq!(
            decode_pair(identity(), 0, &foreign_body, &foreign_commit),
            None
        );
        let mut bad_commit = commit;
        bad_commit[68] = 1;
        seal(&mut bad_commit);
        assert_eq!(decode_pair(identity(), 0, &body, &bad_commit), None);
    }

    #[test]
    fn header_and_read_failures_never_silently_reseed_or_fall_back() {
        let mut io = formatted();
        let foreign = disk_identity::try_new(7, 8).unwrap();
        let before = io.bytes;
        assert_eq!(
            recover(&mut io, foreign),
            Err(durable_error::invalid_header)
        );
        assert_eq!(io.bytes, before);
        io.bytes[0][32] = 1;
        seal(&mut io.bytes[0]);
        assert_eq!(
            recover(&mut io, identity()),
            Err(durable_error::invalid_header)
        );
        for failure in 1..=5 {
            let mut io = formatted();
            io.fail_at = Some(failure);
            assert_eq!(
                recover(&mut io, identity()),
                Err(durable_error::read_failed)
            );
            assert!(
                io.events
                    .iter()
                    .all(|event| matches!(event, event::read(_)))
            );
        }
        let mut io = formatted();
        io.bytes[1..].fill([0; sector_size]);
        assert_eq!(
            recover(&mut io, identity()),
            Err(durable_error::no_valid_checkpoint)
        );
    }

    #[test]
    fn complete_but_inconsistent_checkpoint_pairs_fail_closed() {
        let mut io = formatted();
        install(&mut io, &fixture(4, 1, 3, b"skipped"));
        assert_eq!(
            recover(&mut io, identity()),
            Err(durable_error::ambiguous_checkpoint)
        );
        let mut io = formatted();
        install(&mut io, &fixture(2, 1, 1, b"changed during mount"));
        assert_eq!(
            recover(&mut io, identity()),
            Err(durable_error::ambiguous_checkpoint)
        );
        let mut io = formatted();
        install(&mut io, &fixture(3, 2, 1, b"a"));
        install(&mut io, &fixture(4, 1, 3, b"b"));
        assert_eq!(
            recover(&mut io, identity()),
            Err(durable_error::ambiguous_checkpoint)
        );
    }

    #[test]
    fn counter_overflow_is_rejected_before_writes_and_never_wraps() {
        let mut io = formatted();
        install(&mut io, &fixture(u64::MAX, u64::MAX - 1, 1, b"last"));
        assert!(matches!(
            durable_store::mount(&mut io, identity(), authority()),
            Err(durable_error::sequence_exhausted)
        ));
        assert!(
            io.events
                .iter()
                .all(|event| matches!(event, event::read(_)))
        );
        let mut io = formatted();
        let mut store = opened(&mut io);
        store.checkpoint.sequence = u64::MAX;
        let before_ram = copy_ram(&store.ram);
        let count = store.io.events.len();
        let request = save(&store);
        assert_eq!(
            store.execute(authority(), &request),
            Err(durable_error::sequence_exhausted)
        );
        assert_eq!(store.ram, before_ram);
        assert_eq!(store.io.events.len(), count);
        assert!(!store.is_quarantined());
        let invalid = fixture(1, u64::MAX, 2, b"overflow");
        let body = encode_body(identity(), &invalid);
        let commit = encode_commit(identity(), &invalid, read_u32(&body, 508));
        assert_eq!(decode_pair(identity(), 0, &body, &commit), None);
    }

    #[test]
    fn inactive_slot_reuse_cannot_destroy_the_last_acknowledged_pair() {
        let mut io = formatted();
        {
            let mut store = opened(&mut io);
            let first = save(&store);
            store.execute(authority(), &first).unwrap();
            let next = record::save(store.document(), 1, 2, 2, 3, b"second").unwrap();
            let active_body = store.io.bytes[1];
            let active_commit = store.io.bytes[2];
            store.io.fail_at = Some(store.io.events.len() + 2); // Body written; first flush fails.
            assert_eq!(
                store.execute(authority(), &next),
                Err(durable_error::outcome_unknown)
            );
            assert_eq!(store.io.bytes[1], active_body);
            assert_eq!(store.io.bytes[2], active_commit);
        }
        io.fail_at = None;
        let recovered = recover(&mut io, identity()).unwrap();
        assert_eq!(
            (recovered.version(), recovered.text()),
            (2, b"const after = 1;".as_slice())
        );
    }
}
