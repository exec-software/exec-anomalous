//! Bounded editor replacement within one surviving RAM document service.
//!
//! These operations are not document wire requests. The service must obtain the
//! successor identity and routes from its authenticated lifecycle control gate
//! after the previous task is terminal. Decoding numbers does not confer that
//! authority. The store and its committed bytes remain owned by this service.

use crate::{client_identity, document_store, startup_descriptor};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum replacement_error {
    stale_authority,
    invalid_successor,
    handle_exhausted,
    descriptor,
}

impl document_store {
    /// Revoke the old editor's handle and install its authenticated successor.
    ///
    /// The current bounded lifecycle reuses the exact task slot with a newer
    /// generation. It does not replace the service or reset document/version/
    /// handle-generation counters. All checks precede mutation, and a failed
    /// operation leaves the entire store unchanged. Only a trusted lifecycle
    /// receipt may supply `successor`; a document record must never select it.
    pub fn reassign_editor(
        &mut self,
        expected: client_identity,
        successor: client_identity,
    ) -> Result<(), replacement_error> {
        if expected != self.authority {
            return Err(replacement_error::stale_authority);
        }
        if successor.task() != expected.task() || successor.generation() <= expected.generation() {
            return Err(replacement_error::invalid_successor);
        }
        if self.next_generation == 0 {
            return Err(replacement_error::handle_exhausted);
        }
        self.authority = successor;
        self.open = false;
        Ok(())
    }
}

/// Check the copied successor descriptor against the service's retained launch
/// data. The caller must first obtain it from the service-only lifecycle gate.
/// Its arena describes the successor editor, never memory the service may
/// borrow. All four route handles change, even though the document survives.
pub fn validate_successor(
    service: &startup_descriptor,
    successor: &startup_descriptor,
) -> Result<(), replacement_error> {
    if service.role() != 1
        || successor.role() != 0
        || successor.scenario() != service.scenario()
        || successor.document() != service.document()
        || !successor.seed().is_empty()
        || successor.authority().task() != service.authority().task()
        || successor.authority().generation() <= service.authority().generation()
        || successor
            .handles()
            .iter()
            .any(|handle| service.handles().contains(handle))
    {
        return Err(replacement_error::descriptor);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{operation, record, service_error};

    fn old() -> client_identity {
        client_identity::try_new(3, 7).unwrap()
    }

    fn new() -> client_identity {
        client_identity::try_new(3, 8).unwrap()
    }

    fn store() -> document_store {
        document_store::try_new(old(), 91, b"const before = 1;").unwrap()
    }

    fn opened(store: &mut document_store) {
        let reply = store.execute(old(), &record::open(91, 1).unwrap()).unwrap();
        assert_eq!(reply.generation(), 1);
    }

    fn save() -> record {
        record::save(91, 1, 1, 1, 2, b"const after = 1;").unwrap()
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

    #[test]
    fn replacement_preserves_committed_bytes_and_advances_handle_namespace() {
        let mut store = store();
        opened(&mut store);
        store.execute(old(), &save()).unwrap();
        store.reassign_editor(old(), new()).unwrap();
        assert_eq!(store.stored_text(), b"const after = 1;");
        assert_eq!(store.version(), 2);
        assert!(!store.is_open());
        let reply = store.execute(new(), &record::open(91, 1).unwrap()).unwrap();
        assert_eq!(reply.kind(), operation::opened);
        assert_eq!(reply.generation(), 2);
        assert_eq!(reply.version(), 2);
        assert_eq!(reply.text(), b"const after = 1;");
        let next = record::save(91, 2, 1, 2, 2, b"const newer = 1;").unwrap();
        assert_eq!(store.execute(new(), &next).unwrap().version(), 3);
        store
            .execute(new(), &record::close(91, 2, 2).unwrap())
            .unwrap();
        let reopened = store.execute(new(), &record::open(91, 3).unwrap()).unwrap();
        assert_eq!(reopened.generation(), 3);
        assert_eq!(reopened.text(), next.text());
    }

    #[test]
    fn old_authority_and_old_handle_cannot_retry_into_successor() {
        let mut store = store();
        opened(&mut store);
        let saved = store.execute(old(), &save()).unwrap();
        store.reassign_editor(old(), new()).unwrap();
        store.execute(new(), &record::open(91, 1).unwrap()).unwrap();
        let before = snapshot(&store);
        for request in [save(), record::read(91, 1, 3).unwrap(), saved] {
            assert_eq!(
                store.execute(old(), &request),
                Err(service_error::wrong_authority)
            );
            assert_eq!(store, before);
        }
        assert_eq!(
            store.execute(new(), &save()),
            Err(service_error::stale_handle)
        );
        assert_eq!(store, before);
    }

    #[test]
    fn before_and_after_commit_reply_loss_reopen_actual_store() {
        for committed in [false, true] {
            let mut store = store();
            opened(&mut store);
            if committed {
                let _lost_reply = store.execute(old(), &save()).unwrap();
            }
            store.reassign_editor(old(), new()).unwrap();
            let reply = store.execute(new(), &record::open(91, 1).unwrap()).unwrap();
            assert_eq!(reply.generation(), 2);
            assert_eq!(reply.version(), if committed { 2 } else { 1 });
            assert_eq!(
                reply.text(),
                if committed {
                    b"const after = 1;".as_slice()
                } else {
                    b"const before = 1;".as_slice()
                }
            );
        }
    }

    #[test]
    fn invalid_or_replayed_reassignment_is_atomic() {
        let mut store = store();
        opened(&mut store);
        let before = snapshot(&store);
        for (expected, successor, error) in [
            (new(), old(), replacement_error::stale_authority),
            (old(), old(), replacement_error::invalid_successor),
            (
                old(),
                client_identity::try_new(4, 8).unwrap(),
                replacement_error::invalid_successor,
            ),
            (
                old(),
                client_identity::try_new(3, 6).unwrap(),
                replacement_error::invalid_successor,
            ),
        ] {
            assert_eq!(store.reassign_editor(expected, successor), Err(error));
            assert_eq!(store, before);
        }
        store.reassign_editor(old(), new()).unwrap();
        let after = snapshot(&store);
        assert_eq!(
            store.reassign_editor(old(), new()),
            Err(replacement_error::stale_authority)
        );
        assert_eq!(store, after);
    }

    #[test]
    fn failed_admissions_may_consume_intermediate_task_generations() {
        let mut store = store();
        opened(&mut store);
        let successor = client_identity::try_new(old().task(), old().generation() + 3).unwrap();
        store.reassign_editor(old(), successor).unwrap();
        let reply = store
            .execute(successor, &record::open(91, 1).unwrap())
            .unwrap();
        assert_eq!(reply.generation(), 2);
        let service = descriptor(1, old(), [1, 2, 3, 4]);
        assert_eq!(
            validate_successor(&service, &descriptor(0, successor, [11, 12, 13, 14])),
            Ok(())
        );
    }

    #[test]
    fn exhaustion_rejects_without_revoking_live_authority() {
        let mut store = store();
        opened(&mut store);
        store.next_generation = 0;
        let before = snapshot(&store);
        assert_eq!(
            store.reassign_editor(old(), new()),
            Err(replacement_error::handle_exhausted)
        );
        assert_eq!(store, before);
        store.authority = client_identity::try_new(3, u64::MAX).unwrap();
        let before = snapshot(&store);
        assert_eq!(
            store.reassign_editor(store.authority, new()),
            Err(replacement_error::invalid_successor)
        );
        assert_eq!(store, before);
    }

    fn descriptor(role: u64, authority: client_identity, handles: [u64; 4]) -> startup_descriptor {
        startup_descriptor::try_new(role, 2, [0x804000, 32768], handles, 91, authority, b"")
            .unwrap()
    }

    #[test]
    fn successor_receipt_requires_exact_incarnation_and_new_routes() {
        let service = descriptor(1, old(), [1, 2, 3, 4]);
        let valid = descriptor(0, new(), [11, 12, 13, 14]);
        assert_eq!(validate_successor(&service, &valid), Ok(()));
        for invalid in [
            descriptor(1, new(), [11, 12, 13, 14]),
            descriptor(0, old(), [11, 12, 13, 14]),
            descriptor(0, client_identity::try_new(4, 8).unwrap(), [11, 12, 13, 14]),
            descriptor(0, new(), [11, 12, 13, 4]),
            startup_descriptor::try_new(0, 1, [0x804000, 32768], [11, 12, 13, 14], 91, new(), b"")
                .unwrap(),
            startup_descriptor::try_new(0, 2, [0x804000, 32768], [11, 12, 13, 14], 92, new(), b"")
                .unwrap(),
        ] {
            assert_eq!(
                validate_successor(&service, &invalid),
                Err(replacement_error::descriptor)
            );
        }
    }
}
