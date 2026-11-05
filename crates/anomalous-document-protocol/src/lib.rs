#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Experimental native document policy, entirely in userspace. Copied record
//! fields correlate requests; the separately supplied route identity authorizes
//! them. A saved reply proves this RAM service accepted an exact snapshot. It
//! makes no claim about filesystem persistence or transport acknowledgement.

pub const record_size: usize = 128;
pub const text_capacity: usize = 31;

mod startup;
pub use startup::{arena_byte_count, startup_descriptor, startup_size};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum operation {
    open = 1,
    opened = 2,
    read = 3,
    read_result = 4,
    save = 5,
    saved = 6,
    close = 7,
    closed = 8,
    rejected = 9,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum codec_error {
    length,
    magic,
    version,
    reserved,
    operation,
    status,
    fields,
    text,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum service_error {
    wrong_authority = 1,
    wrong_document = 2,
    already_open = 3,
    stale_handle = 4,
    closed_handle = 5,
    version_conflict = 6,
    stale_request = 7,
    generation_exhausted = 8,
    version_exhausted = 9,
    not_request = 10,
}

/// An identity from trusted route configuration, never from record decoding.
/// Task and generation must both be nonzero. The transport/controller remains
/// responsible for authenticating the actual sender before invoking the store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct client_identity {
    task: u64,
    generation: u64,
}

impl client_identity {
    pub fn try_new(task: u64, generation: u64) -> Result<Self, codec_error> {
        if task == 0 || generation == 0 {
            return Err(codec_error::fields);
        }
        Ok(Self { task, generation })
    }

    #[must_use]
    pub fn task(&self) -> u64 {
        self.task
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authority() -> client_identity {
        client_identity::try_new(1, 7).unwrap()
    }

    fn store() -> document_store {
        document_store::try_new(authority(), 0xd001, b"const before = 1;").unwrap()
    }

    fn snapshot(store: &document_store) -> document_store {
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

    fn open(store: &mut document_store) -> record {
        store
            .execute(authority(), &record::open(0xd001, 1).unwrap())
            .unwrap()
    }

    fn assert_rejected_unchanged(
        store: &mut document_store,
        owner: client_identity,
        request: record,
        error: service_error,
    ) {
        let before = snapshot(store);
        assert_eq!(store.execute(owner, &request), Err(error));
        assert_eq!(*store, before);
        let reply = record::rejection_for(&request, error).unwrap();
        assert_eq!(reply.kind(), operation::rejected);
        assert_eq!(reply.status(), Some(error));
        assert_eq!(reply.document(), request.document());
        assert_eq!(reply.generation(), request.generation());
        assert_eq!(reply.request(), request.request());
        assert_eq!(reply.base_version(), request.base_version());
        assert_eq!(reply.revision(), request.revision());
        assert_eq!(reply.text(), request.text());
        assert_eq!(record::decode(&reply.encode()), Ok(reply));
    }

    #[test]
    fn open_save_read_close_reopen_retains_exact_ram_bytes_with_new_handle() {
        let mut store = store();
        let opened = open(&mut store);
        assert_eq!(opened.kind(), operation::opened);
        assert_eq!(opened.generation(), 1);
        assert_eq!(opened.version(), 1);
        assert_eq!(opened.text(), b"const before = 1;");
        let request = record::save(
            opened.document(),
            opened.generation(),
            1,
            1,
            2,
            b"const after = 1;",
        )
        .unwrap();
        let saved = store.execute(authority(), &request).unwrap();
        assert_eq!(saved.kind(), operation::saved);
        assert_eq!(saved.document(), request.document());
        assert_eq!(saved.generation(), request.generation());
        assert_eq!(saved.request(), request.request());
        assert_eq!(saved.base_version(), 1);
        assert_eq!(saved.version(), 2);
        assert_eq!(saved.revision(), 2);
        assert_eq!(saved.text(), b"const after = 1;");
        let read = record::read(0xd001, 1, 2).unwrap();
        let contents = store.execute(authority(), &read).unwrap();
        assert_eq!(contents.kind(), operation::read_result);
        assert_eq!(contents.text(), saved.text());
        assert_eq!(contents.version(), 2);
        let closed = store
            .execute(authority(), &record::close(0xd001, 1, 3).unwrap())
            .unwrap();
        assert_eq!(closed.kind(), operation::closed);
        assert!(!store.is_open());
        let reopened = open(&mut store);
        assert_eq!(reopened.generation(), 2);
        assert_eq!(reopened.version(), 2);
        assert_eq!(reopened.text(), saved.text());
        for record in [opened, request, saved, read, contents, closed, reopened] {
            assert_eq!(record::decode(&record.encode()), Ok(record));
        }
    }

    #[test]
    fn record_fields_never_replace_the_separately_authenticated_task_generation() {
        let mut store = store();
        let request = record::open(0xd001, 1).unwrap();
        assert_rejected_unchanged(
            &mut store,
            client_identity::try_new(2, 7).unwrap(),
            request,
            service_error::wrong_authority,
        );
        assert_rejected_unchanged(
            &mut store,
            client_identity::try_new(1, 8).unwrap(),
            request,
            service_error::wrong_authority,
        );
        open(&mut store);
        let save = record::save(0xd001, 1, 1, 1, 2, b"replacement").unwrap();
        assert_rejected_unchanged(
            &mut store,
            client_identity::try_new(1, 8).unwrap(),
            save,
            service_error::wrong_authority,
        );
    }

    #[test]
    fn closed_and_reopened_handles_and_replacement_service_documents_are_revoked() {
        let mut store = store();
        open(&mut store);
        store
            .execute(authority(), &record::close(0xd001, 1, 1).unwrap())
            .unwrap();
        let old = record::save(0xd001, 1, 1, 1, 2, b"old").unwrap();
        assert_rejected_unchanged(&mut store, authority(), old, service_error::closed_handle);
        open(&mut store);
        assert_rejected_unchanged(&mut store, authority(), old, service_error::stale_handle);
        let mut replacement = document_store::try_new(authority(), 0xd002, b"new store").unwrap();
        assert_rejected_unchanged(
            &mut replacement,
            authority(),
            old,
            service_error::wrong_document,
        );
        assert_eq!(replacement.stored_text(), b"new store");
    }

    #[test]
    fn conflict_replay_and_already_open_fail_before_store_mutation() {
        let mut store = store();
        open(&mut store);
        assert_rejected_unchanged(
            &mut store,
            authority(),
            record::open(0xd001, 2).unwrap(),
            service_error::already_open,
        );
        let first = record::save(0xd001, 1, 1, 1, 2, b"first").unwrap();
        store.execute(authority(), &first).unwrap();
        assert_rejected_unchanged(&mut store, authority(), first, service_error::stale_request);
        let conflict = record::save(0xd001, 1, 2, 1, 3, b"second").unwrap();
        assert_rejected_unchanged(
            &mut store,
            authority(),
            conflict,
            service_error::version_conflict,
        );
        let corrected = record::save(0xd001, 1, 2, 2, 3, b"second").unwrap();
        assert_eq!(store.execute(authority(), &corrected).unwrap().version(), 3);
    }

    #[test]
    fn canonical_codec_rejects_each_reserved_byte_and_malformed_fields_without_store_changes() {
        let store = store();
        let before = snapshot(&store);
        let bytes = record::save(0xd001, 1, 1, 1, 2, b"hello").unwrap().encode();
        for offset in (10..16).chain(70..record_size) {
            let mut malformed = bytes;
            malformed[offset] = 1;
            assert_eq!(record::decode(&malformed), Err(codec_error::reserved));
            assert_eq!(store, before);
        }
        for offset in [16, 24, 32, 40, 56] {
            let mut malformed = bytes;
            malformed[offset..offset + 8].fill(0);
            assert_eq!(record::decode(&malformed), Err(codec_error::fields));
        }
        for (offset, byte, error) in [
            (0, 0, codec_error::magic),
            (4, 2, codec_error::version),
            (6, 0, codec_error::version),
            (8, 10, codec_error::operation),
            (9, 255, codec_error::status),
            (9, 1, codec_error::status),
            (48, 1, codec_error::fields),
            (64, 32, codec_error::text),
            (65, 0, codec_error::text),
            (65, 127, codec_error::text),
        ] {
            let mut malformed = bytes;
            malformed[offset] = byte;
            assert_eq!(record::decode(&malformed), Err(error));
        }
        assert_eq!(record::decode(&bytes[..127]), Err(codec_error::length));
        assert_eq!(record::decode(&[0; 129]), Err(codec_error::length));
    }

    #[test]
    fn empty_and_maximum_text_are_exact_and_invalid_text_cannot_enter_the_store() {
        let mut store = store();
        open(&mut store);
        for (request, text) in [(1, b"".as_slice()), (2, &[b'X'; text_capacity])] {
            let save = record::save(0xd001, 1, request, request, request + 1, text).unwrap();
            let reply = store.execute(authority(), &save).unwrap();
            assert_eq!(reply.text(), text);
            assert_eq!(store.stored_text(), text);
            assert_eq!(record::decode(&reply.encode()), Ok(reply));
        }
        for bad in [b"\n".as_slice(), &[0xff], &[b'x'; 32]] {
            assert_eq!(record::save(1, 1, 1, 1, 1, bad), Err(codec_error::text));
            assert_eq!(
                document_store::try_new(authority(), 1, bad),
                Err(codec_error::text)
            );
        }
        assert_eq!(client_identity::try_new(0, 1), Err(codec_error::fields));
        assert_eq!(client_identity::try_new(1, 0), Err(codec_error::fields));
        assert_eq!(record::open(0, 1), Err(codec_error::fields));
        assert_eq!(record::open(1, 0), Err(codec_error::fields));
    }

    #[test]
    fn counters_exhaust_without_wrapping_or_partially_committing() {
        let mut store = store();
        store.next_generation = u64::MAX;
        let opened = open(&mut store);
        assert_eq!(opened.generation(), u64::MAX);
        store
            .execute(authority(), &record::close(0xd001, u64::MAX, 1).unwrap())
            .unwrap();
        assert_rejected_unchanged(
            &mut store,
            authority(),
            record::open(0xd001, 2).unwrap(),
            service_error::generation_exhausted,
        );

        let mut store = document_store::try_new(authority(), 0xd001, b"unchanged").unwrap();
        open(&mut store);
        store.version = u64::MAX;
        assert_rejected_unchanged(
            &mut store,
            authority(),
            record::save(0xd001, 1, 1, u64::MAX, 2, b"new").unwrap(),
            service_error::version_exhausted,
        );
        store.version = 1;
        store
            .execute(
                authority(),
                &record::save(0xd001, 1, u64::MAX, 1, 2, b"last").unwrap(),
            )
            .unwrap();
        assert_rejected_unchanged(
            &mut store,
            authority(),
            record::save(0xd001, 1, 1, 2, 3, b"wrapped").unwrap(),
            service_error::stale_request,
        );
    }

    #[test]
    fn lost_reply_does_not_undo_a_committed_save_and_reopen_reports_actual_bytes() {
        let mut store = store();
        open(&mut store);
        let save = record::save(0xd001, 1, 1, 1, 2, b"committed snapshot").unwrap();
        let _lost_reply = store.execute(authority(), &save).unwrap();
        assert_eq!(store.stored_text(), save.text());
        store
            .execute(authority(), &record::close(0xd001, 1, 2).unwrap())
            .unwrap();
        let recovered = open(&mut store);
        assert_eq!(recovered.text(), save.text());
        assert_eq!(recovered.version(), 2);
        assert_ne!(recovered.generation(), save.generation());
    }

    #[test]
    fn replies_cannot_be_reinterpreted_as_requests() {
        let mut store = store();
        let reply = open(&mut store);
        let before = snapshot(&store);
        assert_eq!(
            store.execute(authority(), &reply),
            Err(service_error::not_request)
        );
        assert_eq!(store, before);
        assert_eq!(
            record::rejection_for(&reply, service_error::not_request),
            Err(codec_error::operation)
        );
    }
}

/// Canonical immutable request/reply data. No decoded field grants authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct record {
    operation: operation,
    status: Option<service_error>,
    document: u64,
    generation: u64,
    request: u64,
    base_version: u64,
    version: u64,
    revision: u64,
    text: [u8; text_capacity],
    length: usize,
}

impl record {
    pub fn open(document: u64, request: u64) -> Result<Self, codec_error> {
        Self::request_record(operation::open, document, 0, request)
    }

    pub fn read(document: u64, generation: u64, request: u64) -> Result<Self, codec_error> {
        Self::request_record(operation::read, document, generation, request)
    }

    pub fn close(document: u64, generation: u64, request: u64) -> Result<Self, codec_error> {
        Self::request_record(operation::close, document, generation, request)
    }

    pub fn save(
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
        revision: u64,
        text: &[u8],
    ) -> Result<Self, codec_error> {
        let mut value = Self::blank(operation::save, document, generation, request);
        value.base_version = base_version;
        value.revision = revision;
        value.set_text(text)?;
        value.validate()?;
        Ok(value)
    }

    /// Echoes the rejected request exactly. A client must still match its own
    /// pending tuple before changing any local state in response to rejection.
    pub fn rejection_for(request: &Self, error: service_error) -> Result<Self, codec_error> {
        if !request.is_request() {
            return Err(codec_error::operation);
        }
        let mut reply = *request;
        reply.operation = operation::rejected;
        reply.status = Some(error);
        reply.validate()?;
        Ok(reply)
    }

    fn request_record(
        operation: operation,
        document: u64,
        generation: u64,
        request: u64,
    ) -> Result<Self, codec_error> {
        let value = Self::blank(operation, document, generation, request);
        value.validate()?;
        Ok(value)
    }

    fn blank(operation: operation, document: u64, generation: u64, request: u64) -> Self {
        Self {
            operation,
            status: None,
            document,
            generation,
            request,
            base_version: 0,
            version: 0,
            revision: 0,
            text: [0; text_capacity],
            length: 0,
        }
    }

    fn set_text(&mut self, text: &[u8]) -> Result<(), codec_error> {
        validate_text(text)?;
        self.text = [0; text_capacity];
        self.text[..text.len()].copy_from_slice(text);
        self.length = text.len();
        Ok(())
    }

    #[must_use]
    pub fn kind(&self) -> operation {
        self.operation
    }
    #[must_use]
    pub fn status(&self) -> Option<service_error> {
        self.status
    }
    #[must_use]
    pub fn document(&self) -> u64 {
        self.document
    }
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
    #[must_use]
    pub fn request(&self) -> u64 {
        self.request
    }
    #[must_use]
    pub fn base_version(&self) -> u64 {
        self.base_version
    }
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }
    #[must_use]
    pub fn text(&self) -> &[u8] {
        &self.text[..self.length]
    }
    #[must_use]
    pub fn is_request(&self) -> bool {
        matches!(
            self.operation,
            operation::open | operation::read | operation::save | operation::close
        )
    }

    #[must_use]
    pub fn encode(&self) -> [u8; record_size] {
        let mut bytes = [0; record_size];
        bytes[..4].copy_from_slice(b"ANDC");
        bytes[4..8].copy_from_slice(&[1, 0, record_size as u8, 0]);
        bytes[8] = self.operation as u8;
        bytes[9] = self.status.map_or(0, |error| error as u8);
        for (offset, value) in [
            (16, self.document),
            (24, self.generation),
            (32, self.request),
            (40, self.base_version),
            (48, self.version),
            (56, self.revision),
        ] {
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[64] = self.length as u8;
        bytes[65..65 + self.length].copy_from_slice(self.text());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, codec_error> {
        if bytes.len() != record_size {
            return Err(codec_error::length);
        }
        if &bytes[..4] != b"ANDC" {
            return Err(codec_error::magic);
        }
        if bytes[4..8] != [1, 0, record_size as u8, 0] {
            return Err(codec_error::version);
        }
        let operation = match bytes[8] {
            1 => operation::open,
            2 => operation::opened,
            3 => operation::read,
            4 => operation::read_result,
            5 => operation::save,
            6 => operation::saved,
            7 => operation::close,
            8 => operation::closed,
            9 => operation::rejected,
            _ => return Err(codec_error::operation),
        };
        let status = match bytes[9] {
            0 => None,
            1 => Some(service_error::wrong_authority),
            2 => Some(service_error::wrong_document),
            3 => Some(service_error::already_open),
            4 => Some(service_error::stale_handle),
            5 => Some(service_error::closed_handle),
            6 => Some(service_error::version_conflict),
            7 => Some(service_error::stale_request),
            8 => Some(service_error::generation_exhausted),
            9 => Some(service_error::version_exhausted),
            10 => Some(service_error::not_request),
            _ => return Err(codec_error::status),
        };
        let length = usize::from(bytes[64]);
        if length > text_capacity {
            return Err(codec_error::text);
        }
        if bytes[10..16]
            .iter()
            .chain(bytes[65 + length..].iter())
            .any(|byte| *byte != 0)
        {
            return Err(codec_error::reserved);
        }
        let read = |offset| {
            let mut value = [0; 8];
            value.copy_from_slice(&bytes[offset..offset + 8]);
            u64::from_le_bytes(value)
        };
        let mut value = Self::blank(operation, read(16), read(24), read(32));
        value.status = status;
        value.base_version = read(40);
        value.version = read(48);
        value.revision = read(56);
        value.set_text(&bytes[65..65 + length])?;
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), codec_error> {
        if self.document == 0 || self.request == 0 {
            return Err(codec_error::fields);
        }
        if (self.operation == operation::rejected) != self.status.is_some() {
            return Err(codec_error::status);
        }
        let plain =
            self.base_version == 0 && self.version == 0 && self.revision == 0 && self.length == 0;
        let save_fields = self.generation != 0 && self.base_version != 0 && self.revision != 0;
        let valid = match self.operation {
            operation::open => self.generation == 0 && plain,
            operation::read | operation::close | operation::closed => self.generation != 0 && plain,
            operation::opened | operation::read_result => {
                self.generation != 0
                    && self.version != 0
                    && self.base_version == 0
                    && self.revision == 0
            }
            operation::save => save_fields && self.version == 0,
            operation::saved => {
                save_fields && self.base_version.checked_add(1) == Some(self.version)
            }
            operation::rejected => {
                (plain || (save_fields && self.version == 0)) && (self.generation != 0 || plain)
            }
        };
        if !valid {
            return Err(codec_error::fields);
        }
        validate_text(self.text())
    }
}

fn validate_text(text: &[u8]) -> Result<(), codec_error> {
    if text.len() > text_capacity || text.iter().any(|byte| !(b' '..=b'~').contains(byte)) {
        return Err(codec_error::text);
    }
    Ok(())
}

/// One private RAM document, with at most one open handle and no allocation.
///
/// `document` must be fresh for every replacement service instance, supplied by
/// trusted launch configuration. It is an incarnation identity, not a pathname.
/// Handle generations never wrap within that instance. A replacement service
/// does not recover bytes or authority from this value; it starts a new store.
#[derive(Debug, Eq, PartialEq)]
pub struct document_store {
    authority: client_identity,
    document: u64,
    next_generation: u64,
    generation: u64,
    open: bool,
    version: u64,
    last_save_request: u64,
    text: [u8; text_capacity],
    length: usize,
}

impl document_store {
    pub fn try_new(
        authority: client_identity,
        document: u64,
        initial: &[u8],
    ) -> Result<Self, codec_error> {
        if document == 0 {
            return Err(codec_error::fields);
        }
        validate_text(initial)?;
        let mut text = [0; text_capacity];
        text[..initial.len()].copy_from_slice(initial);
        Ok(Self {
            authority,
            document,
            next_generation: 1,
            generation: 0,
            open: false,
            version: 1,
            last_save_request: 0,
            text,
            length: initial.len(),
        })
    }

    /// The caller supplies an already authenticated route identity independently
    /// of the untrusted record. All validation and reply construction precede
    /// mutation. An error leaves every store byte/counter/handle unchanged.
    /// Successful save commits even if the caller subsequently loses its reply.
    pub fn execute(
        &mut self,
        authority: client_identity,
        request: &record,
    ) -> Result<record, service_error> {
        if authority != self.authority {
            return Err(service_error::wrong_authority);
        }
        if request.document != self.document {
            return Err(service_error::wrong_document);
        }
        if !request.is_request() {
            return Err(service_error::not_request);
        }
        if request.operation == operation::open {
            if self.open {
                return Err(service_error::already_open);
            }
            if self.next_generation == 0 {
                return Err(service_error::generation_exhausted);
            }
            let mut reply = self.content_reply(request, operation::opened);
            reply.generation = self.next_generation;
            self.generation = self.next_generation;
            self.next_generation = self.next_generation.checked_add(1).unwrap_or(0);
            self.open = true;
            self.last_save_request = 0;
            return Ok(reply);
        }
        if request.generation != self.generation {
            return Err(service_error::stale_handle);
        }
        if !self.open {
            return Err(service_error::closed_handle);
        }
        match request.operation {
            operation::read => Ok(self.content_reply(request, operation::read_result)),
            operation::close => {
                let mut reply = *request;
                reply.operation = operation::closed;
                self.open = false;
                Ok(reply)
            }
            operation::save => {
                if request.request <= self.last_save_request {
                    return Err(service_error::stale_request);
                }
                if request.base_version != self.version {
                    return Err(service_error::version_conflict);
                }
                let version = self
                    .version
                    .checked_add(1)
                    .ok_or(service_error::version_exhausted)?;
                let mut reply = *request;
                reply.operation = operation::saved;
                reply.version = version;
                self.text = request.text;
                self.length = request.length;
                self.version = version;
                self.last_save_request = request.request;
                Ok(reply)
            }
            _ => Err(service_error::not_request),
        }
    }

    fn content_reply(&self, request: &record, operation: operation) -> record {
        let mut reply = record::blank(operation, self.document, self.generation, request.request);
        reply.version = self.version;
        reply.text = self.text;
        reply.length = self.length;
        reply
    }

    #[must_use]
    pub fn stored_text(&self) -> &[u8] {
        &self.text[..self.length]
    }
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }
}

pub mod relaunch;

pub mod durable;
pub mod durable_failure_startup;
pub mod durable_startup;
pub mod interactive_startup;

#[cfg(feature = "remote_workspace")]
pub mod held_save_release;
#[cfg(feature = "remote_workspace")]
pub mod remote_startup;
