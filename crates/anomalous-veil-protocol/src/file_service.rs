//! Portable, allocation-free read-only file-service state for userspace.
//!
//! Trusted configuration supplies identities, a complete namespace scope and
//! immutable file bytes. Neither their numeric identities nor a copied request
//! authenticate a caller: the embedding must authenticate that caller first.
//! These types are not a wire format, native capability or Linux syscall ABI.
//!
//! Publication is synchronous. A real adapter must atomically publish the
//! result and any read bytes, retain this service until the outcome is known,
//! and contain the caller on veil failure/lost acknowledgement. This portable
//! state machine does not establish those native scheduling/copy guarantees.
//! A rejected publication made no externally visible change and can be retried;
//! an unknown outcome permanently fences this instance and revokes its handles.
//! Callback unwinding also fences the instance. Local validation errors publish
//! nothing; this model does not publish their corresponding Linux errno values.
//! The embedding binds pending retries to its retained authenticated request and
//! accounts separately for invocations completed outside these file operations.
//! Snapshots borrow their immutable provider through every descriptor. Bounded
//! reads can use per-invocation scratch and an embedding-authenticated context;
//! the provider and its context do not themselves establish native authority.

/// A provider failure before publication. Failed reads may dirty their entire
/// supplied destination; none of those bytes are eligible for publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum source_error {
    unavailable,
    changed,
    invalid_extent,
    failed,
}

/// An immutable, borrowed source with statically dispatched bounded reads.
///
/// `Eq` must compare the actual retained snapshot binding, not only a filename
/// or caller-selected object/version numbers. Equal providers must denote the
/// same immutable bytes and length. The embedding retains the backing source
/// for this borrow, authenticates each read context, and rejects revoked or
/// changed backing storage. This trait cannot establish those external facts.
///
/// A successful read reports its exact delivered length. The service rejects
/// both short and oversized replies instead of publishing a possibly dirty
/// prefix. Source length is u64; no whole-source usize conversion is required.
pub trait immutable_file_source: Eq {
    type read_context: ?Sized;

    fn byte_length(&self) -> u64;

    fn read_at(
        &self,
        context: &mut Self::read_context,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, source_error>;
}

impl immutable_file_source for [u8] {
    type read_context = ();

    fn byte_length(&self) -> u64 {
        self.len() as u64
    }

    fn read_at(
        &self,
        _context: &mut (),
        offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, source_error> {
        let start = usize::try_from(offset).map_err(|_| source_error::invalid_extent)?;
        let end = start
            .checked_add(destination.len())
            .ok_or(source_error::invalid_extent)?;
        let bytes = self.get(start..end).ok_or(source_error::invalid_extent)?;
        destination.copy_from_slice(bytes);
        Ok(bytes.len())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum file_error {
    invalid_identity,
    invalid_file,
    invalid_prefix,
    invalid_entry,
    duplicate_path,
    conflicting_paths,
    inconsistent_file,
    invalid_descriptor_range,
    wrong_owner,
    stale_invocation,
    fenced,
    out_of_scope,
    unsupported_path,
    not_found,
    table_full,
    generation_exhausted,
    bad_descriptor,
    stale_handle,
    invalid_budget,
    offset_overflow,
    invalid_scratch,
    source_failure(source_error),
    source_length_mismatch,
    publication_rejected,
    publication_unknown,
}

/// Inert identity supplied by an authenticated route or trusted lifecycle gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct client_identity {
    task: u64,
    generation: u64,
}

impl client_identity {
    pub fn new(task: u64, generation: u64) -> Result<Self, file_error> {
        if task == 0 || generation == 0 {
            return Err(file_error::invalid_identity);
        }
        Ok(Self { task, generation })
    }

    pub const fn task(self) -> u64 {
        self.task
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// A borrowed immutable version, not a claim that matching identifiers or a
/// filename can fetch bytes. The borrow retains its provider for every handle.
/// Object/version identifiers must retain their meaning across namespace views;
/// the trusted provider is responsible for that cross-view identity contract.
#[derive(Debug)]
pub struct file_snapshot<'a, source: immutable_file_source + ?Sized = [u8]> {
    object: u64,
    version: u64,
    source: &'a source,
    length: u64,
}

impl<'a> file_snapshot<'a> {
    pub fn new(object: u64, version: u64, bytes: &'a [u8]) -> Result<Self, file_error> {
        Self::from_source(object, version, bytes)
    }

    pub const fn bytes(self) -> &'a [u8] {
        self.source
    }

    pub const fn len(self) -> usize {
        self.source.len()
    }
}

impl<source: immutable_file_source + ?Sized> Copy for file_snapshot<'_, source> {}

impl<source: immutable_file_source + ?Sized> Clone for file_snapshot<'_, source> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<source: immutable_file_source + ?Sized> PartialEq for file_snapshot<'_, source> {
    fn eq(&self, other: &Self) -> bool {
        self.object == other.object
            && self.version == other.version
            && self.length == other.length
            && self.source == other.source
    }
}

impl<source: immutable_file_source + ?Sized> Eq for file_snapshot<'_, source> {}

impl<'a, source: immutable_file_source + ?Sized> file_snapshot<'a, source> {
    pub fn from_source(object: u64, version: u64, source: &'a source) -> Result<Self, file_error> {
        if object == 0 || version == 0 {
            return Err(file_error::invalid_file);
        }
        Ok(Self {
            object,
            version,
            source,
            length: source.byte_length(),
        })
    }

    pub const fn object(self) -> u64 {
        self.object
    }

    pub const fn version(self) -> u64 {
        self.version
    }

    pub const fn source(self) -> &'a source {
        self.source
    }

    pub const fn byte_length(self) -> u64 {
        self.length
    }

    pub const fn is_empty(self) -> bool {
        self.length == 0
    }
}

/// Inert metadata of an already retained immutable snapshot. These unsigned
/// native values are neither a foreign stat layout nor authority to read its
/// bytes. Foreign identity, permissions, timestamps and representation limits
/// remain the userspace adapter's explicit policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct file_metadata {
    object: u64,
    version: u64,
    length: u64,
}

impl file_metadata {
    pub const fn object(self) -> u64 {
        self.object
    }

    pub const fn version(self) -> u64 {
        self.version
    }

    pub const fn byte_length(self) -> u64 {
        self.length
    }
}

fn canonical_path(path: &[u8]) -> bool {
    if path.first() != Some(&b'/') || path.contains(&0) {
        return false;
    }
    if path == b"/" {
        return true;
    }
    path[1..]
        .split(|byte| *byte == b'/')
        .all(|part| !part.is_empty() && part != b"." && part != b"..")
}

fn descendant(path: &[u8], directory: &[u8]) -> bool {
    if directory == b"/" {
        return path.len() > 1 && path.starts_with(directory);
    }
    path.starts_with(directory) && path.get(directory.len()) == Some(&b'/')
}

#[derive(Debug, Eq, PartialEq)]
pub struct namespace_entry<'a, source: immutable_file_source + ?Sized = [u8]> {
    path: &'a [u8],
    file: file_snapshot<'a, source>,
}

impl<source: immutable_file_source + ?Sized> Copy for namespace_entry<'_, source> {}

impl<source: immutable_file_source + ?Sized> Clone for namespace_entry<'_, source> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, source: immutable_file_source + ?Sized> namespace_entry<'a, source> {
    pub fn new(path: &'a [u8], file: file_snapshot<'a, source>) -> Result<Self, file_error> {
        if !canonical_path(path) || path == b"/" {
            return Err(file_error::invalid_entry);
        }
        Ok(Self { path, file })
    }

    pub const fn path(self) -> &'a [u8] {
        self.path
    }

    pub const fn file(self) -> file_snapshot<'a, source> {
        self.file
    }
}

/// An explicitly complete immutable subtree of canonical absolute byte names.
/// The prefix denotes a directory; entries denote regular files. Intermediate
/// directories are implicit. Missing canonical names in this declared subtree
/// are absent. Other scopes and unsupported syntax do not become absence.
///
/// No normalization, symlink or directory-open semantics are supplied. Paths
/// are borrowed slices: this model imposes no fixed name length/depth buffer.
/// An embedding still accounts for its input/traversal work independently.
pub struct namespace<'a, source: immutable_file_source + ?Sized = [u8]> {
    prefix: &'a [u8],
    entries: &'a [namespace_entry<'a, source>],
}

impl<'a, source: immutable_file_source + ?Sized> namespace<'a, source> {
    pub fn new(
        prefix: &'a [u8],
        entries: &'a [namespace_entry<'a, source>],
    ) -> Result<Self, file_error> {
        if !canonical_path(prefix) {
            return Err(file_error::invalid_prefix);
        }
        for (index, entry) in entries.iter().enumerate() {
            if !descendant(entry.path, prefix) {
                return Err(file_error::invalid_entry);
            }
            for previous in &entries[..index] {
                if previous.path == entry.path {
                    return Err(file_error::duplicate_path);
                }
                if descendant(previous.path, entry.path) || descendant(entry.path, previous.path) {
                    return Err(file_error::conflicting_paths);
                }
                if previous.file.object == entry.file.object
                    && previous.file.version == entry.file.version
                    && previous.file != entry.file
                {
                    return Err(file_error::inconsistent_file);
                }
            }
        }
        Ok(Self { prefix, entries })
    }

    pub const fn prefix(&self) -> &'a [u8] {
        self.prefix
    }

    pub fn lookup(&self, path: &[u8]) -> Result<file_snapshot<'a, source>, file_error> {
        if !canonical_path(path) {
            return Err(file_error::unsupported_path);
        }
        if path == self.prefix {
            return Err(file_error::unsupported_path);
        }
        if !descendant(path, self.prefix) {
            return Err(file_error::out_of_scope);
        }
        for entry in self.entries {
            if entry.path == path {
                return Ok(entry.file);
            }
            // A file cannot be traversed as a directory; a known implicit
            // directory cannot be opened as a regular file by this model.
            if descendant(path, entry.path) || descendant(entry.path, path) {
                return Err(file_error::unsupported_path);
            }
        }
        Err(file_error::not_found)
    }
}

/// Internal handle returned only after open publication commits. Linux integer
/// fd reuse does not reject stale intent: an adapter resolves the current fd to
/// this handle. The generation prevents retained internal operations/handles
/// from accidentally targeting a later opening of that same integer slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct descriptor_handle {
    owner: client_identity,
    service_incarnation: u64,
    descriptor: u32,
    generation: u64,
}

impl descriptor_handle {
    pub const fn fd(self) -> u32 {
        self.descriptor
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn service_incarnation(self) -> u64 {
        self.service_incarnation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum publication<'a> {
    opened {
        descriptor: u32,
    },
    read {
        descriptor: u32,
        offset: u64,
        bytes: &'a [u8],
    },
    closed {
        descriptor: u32,
    },
}

/// This is an embedding's synchronous report, not an authority receipt.
/// `committed` means all bytes and the result were atomically published.
/// `rejected` means neither was published. An indeterminate effect is `unknown`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum publication_result {
    committed,
    rejected,
    unknown,
}

#[derive(Debug, Eq, PartialEq)]
struct descriptor<'a, source: immutable_file_source + ?Sized = [u8]> {
    handle: descriptor_handle,
    file: file_snapshot<'a, source>,
    offset: u64,
    close_on_exec: bool,
}

impl<source: immutable_file_source + ?Sized> Copy for descriptor<'_, source> {}

impl<source: immutable_file_source + ?Sized> Clone for descriptor<'_, source> {
    fn clone(&self) -> Self {
        *self
    }
}

struct read_plan<'a, source: immutable_file_source + ?Sized> {
    index: usize,
    descriptor: descriptor<'a, source>,
    source_offset: u64,
    length: usize,
    next_offset: u64,
}

// This safe core guarantees logical zeroing, including unwinding. It does not
// guarantee volatile stores, physical remanence removal or zeroing after abort.
// A native embedding separately clears its retained buffers at trust boundaries.
struct scratch_guard<'a> {
    bytes: &'a mut [u8],
}

impl Drop for scratch_guard<'_> {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

/// One authenticated caller incarnation and one service lifetime. The trusted
/// embedding allocates a fresh service incarnation on replacement; recreating
/// the same numbers can accidentally admit old handles and is not safe recovery.
/// The constructor cannot establish global identity freshness.
///
/// Slots below `first_descriptor` are explicitly reserved outside this table;
/// this configuration does not itself create usable standard streams. Capacity
/// is a supplied per-instance resource budget, not a global descriptor limit.
pub struct file_service<'a, const capacity: usize, source: immutable_file_source + ?Sized = [u8]> {
    owner: client_identity,
    incarnation: u64,
    first_descriptor: u32,
    descriptors: [Option<descriptor<'a, source>>; capacity],
    next_generation: u64,
    last_sequence: u64,
    fenced: bool,
}

// Any path out of a publication without a known returned outcome revokes the
// whole service. In particular, host catch_unwind cannot turn a callback panic
// after external publication into permission to retry with stale local state.
struct publication_guard<
    'service,
    'a,
    const capacity: usize,
    source: immutable_file_source + ?Sized,
> {
    service: &'service mut file_service<'a, capacity, source>,
    resolved: bool,
}

impl<const capacity: usize, source: immutable_file_source + ?Sized> Drop
    for publication_guard<'_, '_, capacity, source>
{
    fn drop(&mut self) {
        if !self.resolved {
            self.service.fence();
        }
    }
}

impl<'a, const capacity: usize, source: immutable_file_source + ?Sized>
    file_service<'a, capacity, source>
{
    pub fn new(
        owner: client_identity,
        service_incarnation: u64,
        first_descriptor: u32,
    ) -> Result<Self, file_error> {
        if service_incarnation == 0 {
            return Err(file_error::invalid_identity);
        }
        let last = u64::try_from(capacity.saturating_sub(1))
            .ok()
            .and_then(|index| u64::from(first_descriptor).checked_add(index));
        if first_descriptor > i32::MAX as u32 || last.is_none_or(|number| number > i32::MAX as u64)
        {
            return Err(file_error::invalid_descriptor_range);
        }
        Ok(Self {
            owner,
            incarnation: service_incarnation,
            first_descriptor,
            descriptors: [None; capacity],
            next_generation: 1,
            last_sequence: 0,
            fenced: false,
        })
    }

    fn authenticate(&self, owner: client_identity) -> Result<(), file_error> {
        if owner != self.owner {
            return Err(file_error::wrong_owner);
        }
        if self.fenced {
            return Err(file_error::fenced);
        }
        Ok(())
    }

    fn invocation(&self, owner: client_identity, sequence: u64) -> Result<(), file_error> {
        self.authenticate(owner)?;
        if sequence == 0 || sequence <= self.last_sequence {
            return Err(file_error::stale_invocation);
        }
        Ok(())
    }

    fn index(&self, fd: u32) -> Result<usize, file_error> {
        let index = fd
            .checked_sub(self.first_descriptor)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|index| *index < capacity)
            .ok_or(file_error::bad_descriptor)?;
        Ok(index)
    }

    fn retained(
        &self,
        owner: client_identity,
        handle: descriptor_handle,
    ) -> Result<(usize, descriptor<'a, source>), file_error> {
        self.authenticate(owner)?;
        if handle.owner != self.owner || handle.service_incarnation != self.incarnation {
            return Err(file_error::stale_handle);
        }
        let index = self.index(handle.descriptor)?;
        let descriptor = self.descriptors[index].ok_or(file_error::stale_handle)?;
        if descriptor.handle != handle {
            return Err(file_error::stale_handle);
        }
        Ok((index, descriptor))
    }

    fn publication<event>(
        &mut self,
        event: event,
        publish: impl FnOnce(event) -> publication_result,
    ) -> Result<(), file_error> {
        let mut guard = publication_guard {
            service: self,
            resolved: false,
        };
        let outcome = publish(event);
        guard.resolved = outcome != publication_result::unknown;
        match outcome {
            publication_result::committed => Ok(()),
            publication_result::rejected => Err(file_error::publication_rejected),
            publication_result::unknown => Err(file_error::publication_unknown),
        }
    }

    fn fence(&mut self) {
        self.descriptors.fill(None);
        self.fenced = true;
    }

    /// Local validation failures have no publication and consume no sequence.
    /// Rejected publication also leaves committed state unchanged. This watermark
    /// orders committed operations only; it does not bind a rejected request's
    /// identity or prevent the embedding from changing a pending request body.
    /// The embedding must retain/bind exact retries and account for completed
    /// errno returns. All fallible calculations precede the publication callback.
    pub fn open(
        &mut self,
        owner: client_identity,
        sequence: u64,
        namespace: &namespace<'a, source>,
        path: &[u8],
        close_on_exec: bool,
        publish: impl FnOnce(publication<'a>) -> publication_result,
    ) -> Result<descriptor_handle, file_error> {
        self.invocation(owner, sequence)?;
        let file = namespace.lookup(path)?;
        if self.next_generation == 0 {
            return Err(file_error::generation_exhausted);
        }
        let index = self
            .descriptors
            .iter()
            .position(Option::is_none)
            .ok_or(file_error::table_full)?;
        // Constructor validated the entire table's signed Linux-fd range.
        let fd = self.first_descriptor + index as u32;
        let handle = descriptor_handle {
            owner,
            service_incarnation: self.incarnation,
            descriptor: fd,
            generation: self.next_generation,
        };
        let next_generation = self.next_generation.checked_add(1).unwrap_or(0);
        self.publication(publication::opened { descriptor: fd }, publish)?;
        self.descriptors[index] = Some(descriptor {
            handle,
            file,
            offset: 0,
            close_on_exec,
        });
        self.next_generation = next_generation;
        self.last_sequence = sequence;
        Ok(handle)
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_read(
        &self,
        owner: client_identity,
        sequence: u64,
        handle: descriptor_handle,
        position: Option<u64>,
        count: u64,
        budget: usize,
    ) -> Result<read_plan<'a, source>, file_error> {
        self.invocation(owner, sequence)?;
        let (index, descriptor) = self.retained(owner, handle)?;
        let sequential_remaining = descriptor
            .file
            .length
            .checked_sub(descriptor.offset)
            .ok_or(file_error::offset_overflow)?;
        let source_offset = position.unwrap_or(descriptor.offset);
        let remaining = position.map_or(sequential_remaining, |offset| {
            descriptor.file.length.saturating_sub(offset)
        });
        if count != 0 && remaining != 0 && budget == 0 {
            return Err(file_error::invalid_budget);
        }
        // Only the bounded transfer is narrowed; a source and its offset can
        // exceed usize on a narrow target without truncation or whole mapping.
        let length = usize::try_from(remaining.min(count).min(budget as u64))
            .map_err(|_| file_error::offset_overflow)?;
        let next_offset = if position.is_some() {
            descriptor.offset
        } else {
            descriptor
                .offset
                .checked_add(length as u64)
                .ok_or(file_error::offset_overflow)?
        };
        Ok(read_plan {
            index,
            descriptor,
            source_offset,
            length,
            next_offset,
        })
    }

    fn commit_read(&mut self, sequence: u64, mut plan: read_plan<'a, source>) -> usize {
        plan.descriptor.offset = plan.next_offset;
        self.descriptors[plan.index] = Some(plan.descriptor);
        self.last_sequence = sequence;
        plan.length
    }

    /// Read min(count, budget, remaining bytes) through the retained provider,
    /// then publish only an exact successful delivery. Scratch must hold that
    /// complete planned transfer; a small scratch is not an implicit new budget.
    /// Zero count and EOF publish an empty slice without calling the provider.
    ///
    /// The callback's bytes borrow lasts only for this invocation. It may remain
    /// suspended while an embedding awaits acknowledgement, but cannot retain
    /// the slice after returning. No descriptor/offset/watermark change occurs
    /// until committed acknowledgement. Unknown outcomes and callback unwinding
    /// fence this service; provider unwinding also conservatively fences it.
    ///
    /// The complete supplied scratch is logically zeroed before source access
    /// and on every return or unwind, including validation errors. This safe
    /// no_std core does not promise volatile erasure or cleanup after abort.
    #[allow(clippy::too_many_arguments)]
    pub fn read_into(
        &mut self,
        owner: client_identity,
        sequence: u64,
        handle: descriptor_handle,
        count: u64,
        budget: usize,
        context: &mut source::read_context,
        scratch: &mut [u8],
        publish: impl FnOnce(publication<'_>) -> publication_result,
    ) -> Result<usize, file_error> {
        self.read_into_position(
            owner, sequence, handle, None, count, budget, context, scratch, publish,
        )
    }

    /// Read from an explicit unsigned source offset without changing the
    /// descriptor's sequential offset. An offset at or beyond EOF publishes an
    /// empty read without calling the provider. Committed publication still
    /// advances the request watermark; every other acknowledgement, scratch
    /// clearing and source-validation rule is identical to `read_into`.
    /// Foreign signed offsets must be validated by their userspace adapter.
    #[allow(clippy::too_many_arguments)]
    pub fn read_at_into(
        &mut self,
        owner: client_identity,
        sequence: u64,
        handle: descriptor_handle,
        offset: u64,
        count: u64,
        budget: usize,
        context: &mut source::read_context,
        scratch: &mut [u8],
        publish: impl FnOnce(publication<'_>) -> publication_result,
    ) -> Result<usize, file_error> {
        self.read_into_position(
            owner,
            sequence,
            handle,
            Some(offset),
            count,
            budget,
            context,
            scratch,
            publish,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn read_into_position(
        &mut self,
        owner: client_identity,
        sequence: u64,
        handle: descriptor_handle,
        position: Option<u64>,
        count: u64,
        budget: usize,
        context: &mut source::read_context,
        scratch: &mut [u8],
        publish: impl FnOnce(publication<'_>) -> publication_result,
    ) -> Result<usize, file_error> {
        let scratch = scratch_guard { bytes: scratch };
        scratch.bytes.fill(0);
        let plan = self.prepare_read(owner, sequence, handle, position, count, budget)?;
        if plan.length > scratch.bytes.len() {
            return Err(file_error::invalid_scratch);
        }
        if plan.length != 0 {
            // The guard fences only on unwind here. Ordinary backend errors
            // publish nothing and leave all committed service state unchanged.
            let mut guard = publication_guard {
                service: self,
                resolved: false,
            };
            let provider = plan.descriptor.file.source;
            let result = if provider.byte_length() != plan.descriptor.file.length {
                Err(file_error::source_failure(source_error::changed))
            } else {
                provider
                    .read_at(
                        context,
                        plan.source_offset,
                        &mut scratch.bytes[..plan.length],
                    )
                    .map_err(file_error::source_failure)
                    .and_then(|length| {
                        if provider.byte_length() != plan.descriptor.file.length {
                            Err(file_error::source_failure(source_error::changed))
                        } else if length == plan.length {
                            Ok(())
                        } else {
                            Err(file_error::source_length_mismatch)
                        }
                    })
            };
            guard.resolved = true;
            result?;
        }
        self.publication(
            publication::read {
                descriptor: handle.fd(),
                offset: plan.source_offset,
                bytes: &scratch.bytes[..plan.length],
            },
            publish,
        )?;
        Ok(self.commit_read(sequence, plan))
    }

    pub fn close(
        &mut self,
        owner: client_identity,
        sequence: u64,
        handle: descriptor_handle,
        publish: impl FnOnce(publication<'a>) -> publication_result,
    ) -> Result<(), file_error> {
        self.invocation(owner, sequence)?;
        let (index, _) = self.retained(owner, handle)?;
        self.publication(
            publication::closed {
                descriptor: handle.fd(),
            },
            publish,
        )?;
        self.descriptors[index] = None;
        self.last_sequence = sequence;
        Ok(())
    }

    /// Publish metadata from this exact opened snapshot without consulting a
    /// replacement namespace or invoking its byte provider. Snapshot retention
    /// and byte-source availability are separate embedding obligations.
    ///
    /// Confirmed publication advances only the request watermark; the handle,
    /// snapshot and sequential offset remain unchanged. Known rejection permits
    /// an authenticated retry. Unknown publication and callback unwinding fence
    /// the whole service through the same guard used by open, read and close.
    pub fn metadata(
        &mut self,
        owner: client_identity,
        sequence: u64,
        handle: descriptor_handle,
        publish: impl FnOnce(file_metadata) -> publication_result,
    ) -> Result<(), file_error> {
        self.invocation(owner, sequence)?;
        let (_, descriptor) = self.retained(owner, handle)?;
        let metadata = file_metadata {
            object: descriptor.file.object,
            version: descriptor.file.version,
            length: descriptor.file.length,
        };
        self.publication(metadata, publish)?;
        self.last_sequence = sequence;
        Ok(())
    }

    pub fn handle_for_fd(
        &self,
        owner: client_identity,
        fd: u32,
    ) -> Result<descriptor_handle, file_error> {
        self.authenticate(owner)?;
        self.descriptors[self.index(fd)?]
            .map(|descriptor| descriptor.handle)
            .ok_or(file_error::bad_descriptor)
    }

    pub fn offset(
        &self,
        owner: client_identity,
        handle: descriptor_handle,
    ) -> Result<u64, file_error> {
        self.retained(owner, handle)
            .map(|(_, descriptor)| descriptor.offset)
    }

    pub fn file(
        &self,
        owner: client_identity,
        handle: descriptor_handle,
    ) -> Result<file_snapshot<'a, source>, file_error> {
        self.retained(owner, handle)
            .map(|(_, descriptor)| descriptor.file)
    }

    /// Stored descriptor policy only; no exec lifecycle is implemented here.
    pub fn close_on_exec(
        &self,
        owner: client_identity,
        handle: descriptor_handle,
    ) -> Result<bool, file_error> {
        self.retained(owner, handle)
            .map(|(_, descriptor)| descriptor.close_on_exec)
    }

    /// Trusted lifecycle action, never a caller-selected request. Idempotently
    /// revoke this whole instance, including handles from uncertain publication.
    pub fn retire(&mut self, owner: client_identity) -> Result<(), file_error> {
        if owner != self.owner {
            return Err(file_error::wrong_owner);
        }
        self.fence();
        Ok(())
    }

    pub const fn is_fenced(&self) -> bool {
        self.fenced
    }

    pub fn open_count(&self) -> usize {
        self.descriptors
            .iter()
            .filter(|entry| entry.is_some())
            .count()
    }

    /// Last committed file operation; not a ledger of every Linux invocation.
    pub const fn last_sequence(&self) -> u64 {
        self.last_sequence
    }
}

impl<'a, const capacity: usize> file_service<'a, capacity> {
    /// Publish borrowed slice bytes using the same offset planning and commit
    /// as chunk-backed reads. A budget-induced short read is not EOF. Count
    /// zero and actual EOF publish an empty slice; zero budget with available
    /// requested data is an error. Existing slice-backed callers need no scratch.
    pub fn read(
        &mut self,
        owner: client_identity,
        sequence: u64,
        handle: descriptor_handle,
        count: u64,
        budget: usize,
        publish: impl FnOnce(publication<'a>) -> publication_result,
    ) -> Result<usize, file_error> {
        self.read_position(owner, sequence, handle, None, count, budget, publish)
    }

    /// Borrow bytes at an explicit source offset while preserving the
    /// sequential descriptor offset, using the same contract as `read_at_into`.
    #[allow(clippy::too_many_arguments)]
    pub fn read_at(
        &mut self,
        owner: client_identity,
        sequence: u64,
        handle: descriptor_handle,
        offset: u64,
        count: u64,
        budget: usize,
        publish: impl FnOnce(publication<'a>) -> publication_result,
    ) -> Result<usize, file_error> {
        self.read_position(
            owner,
            sequence,
            handle,
            Some(offset),
            count,
            budget,
            publish,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn read_position(
        &mut self,
        owner: client_identity,
        sequence: u64,
        handle: descriptor_handle,
        position: Option<u64>,
        count: u64,
        budget: usize,
        publish: impl FnOnce(publication<'a>) -> publication_result,
    ) -> Result<usize, file_error> {
        let plan = self.prepare_read(owner, sequence, handle, position, count, budget)?;
        // An empty positional read beyond EOF has no source range to narrow.
        // Preserve the original offset in its publication, including u64::MAX.
        let offset = usize::try_from(plan.source_offset.min(plan.descriptor.file.length))
            .map_err(|_| file_error::offset_overflow)?;
        let end = offset
            .checked_add(plan.length)
            .ok_or(file_error::offset_overflow)?;
        let bytes = plan
            .descriptor
            .file
            .source
            .get(offset..end)
            .ok_or(file_error::offset_overflow)?;
        self.publication(
            publication::read {
                descriptor: handle.fd(),
                offset: plan.source_offset,
                bytes,
            },
            publish,
        )?;
        Ok(self.commit_read(sequence, plan))
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::vec;

    fn owner() -> client_identity {
        client_identity::new(71, 1).unwrap()
    }

    fn entries() -> [namespace_entry<'static>; 2] {
        [
            namespace_entry::new(b"/lib/a", file_snapshot::new(5, 1, b"abcdef").unwrap()).unwrap(),
            namespace_entry::new(b"/lib/empty", file_snapshot::new(6, 1, b"").unwrap()).unwrap(),
        ]
    }

    fn commit(_: publication<'_>) -> publication_result {
        publication_result::committed
    }

    fn never(_: publication<'_>) -> publication_result {
        panic!("validation failure must not publish")
    }

    fn state<'a, const capacity: usize>(
        service: &file_service<'a, capacity>,
    ) -> ([Option<descriptor<'a>>; capacity], u64, u64, bool) {
        (
            service.descriptors,
            service.next_generation,
            service.last_sequence,
            service.fenced,
        )
    }

    #[test]
    fn namespace_distinguishes_absence_scope_and_unsupported_syntax() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        assert_eq!(view.prefix(), b"/lib");
        assert_eq!(view.lookup(b"/lib/a").unwrap().bytes(), b"abcdef");
        assert_eq!(view.lookup(b"/lib/missing"), Err(file_error::not_found));
        assert_eq!(
            view.lookup(b"/lib/missing/deep"),
            Err(file_error::not_found)
        );
        for path in [b"/library/a".as_slice(), b"/lib2/a", b"/other/a", b"/"] {
            assert_eq!(view.lookup(path), Err(file_error::out_of_scope));
        }
        for path in [
            b"".as_slice(),
            b"lib/a",
            b"/lib",
            b"/lib/",
            b"/lib//a",
            b"/lib/./a",
            b"/lib/../a",
            b"/lib/a\0",
            b"/lib/a/child",
        ] {
            assert_eq!(view.lookup(path), Err(file_error::unsupported_path));
        }
        let nested = [namespace_entry::new(b"/lib/nested/file", entries[0].file()).unwrap()];
        let nested = namespace::new(b"/lib", &nested).unwrap();
        assert_eq!(
            nested.lookup(b"/lib/nested"),
            Err(file_error::unsupported_path)
        );
        assert_eq!(
            nested.lookup(b"/lib/nested/missing"),
            Err(file_error::not_found)
        );
        let root = namespace::new(b"/", &entries).unwrap();
        assert_eq!(root.lookup(b"/unknown"), Err(file_error::not_found));
        assert_eq!(root.lookup(b"/"), Err(file_error::unsupported_path));
    }

    #[test]
    fn binary_long_and_deep_names_have_no_fixed_path_buffer() {
        let file = file_snapshot::new(9, 1, b"held").unwrap();
        let mut long_path = b"/lib/".to_vec();
        long_path.extend_from_slice(&vec![b'x'; 8192]);
        let mut deep_path = b"/lib".to_vec();
        for _ in 0..5000 {
            deep_path.extend_from_slice(b"/x");
        }
        let binary_path = b"/lib/\xff\x80\xfe";
        let entries = [
            namespace_entry::new(&long_path, file).unwrap(),
            namespace_entry::new(&deep_path, file).unwrap(),
            namespace_entry::new(binary_path, file).unwrap(),
        ];
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<3>::new(owner(), 90, 3).unwrap();
        for (index, path) in [&long_path[..], &deep_path[..], &binary_path[..]]
            .into_iter()
            .enumerate()
        {
            assert_eq!(view.lookup(path), Ok(file));
            let handle = service
                .open(owner(), index as u64 + 1, &view, path, false, commit)
                .unwrap();
            assert_eq!(handle.fd(), index as u32 + 3);
            assert_eq!(service.file(owner(), handle), Ok(file));
        }
        let binary_entries = [namespace_entry::new(b"/\xff/a", file).unwrap()];
        let binary_view = namespace::new(b"/\xff", &binary_entries).unwrap();
        assert_eq!(binary_view.lookup(b"/\xff/a"), Ok(file));
        assert_eq!(
            binary_view.lookup(b"/\xffx/a"),
            Err(file_error::out_of_scope)
        );
    }

    #[test]
    fn trusted_configuration_rejects_ambiguous_or_invalid_bindings() {
        assert_eq!(
            client_identity::new(0, 1),
            Err(file_error::invalid_identity)
        );
        assert_eq!(
            client_identity::new(1, 0),
            Err(file_error::invalid_identity)
        );
        assert_eq!(file_snapshot::new(0, 1, b""), Err(file_error::invalid_file));
        assert_eq!(file_snapshot::new(1, 0, b""), Err(file_error::invalid_file));
        let file = file_snapshot::new(1, 1, b"a").unwrap();
        assert_eq!(
            namespace_entry::new(b"/", file),
            Err(file_error::invalid_entry)
        );
        assert_eq!(
            namespace_entry::new(b"relative", file),
            Err(file_error::invalid_entry)
        );
        assert_eq!(
            namespace::<[u8]>::new(b"/lib/", &[]).err(),
            Some(file_error::invalid_prefix)
        );
        let entry = namespace_entry::new(b"/lib/a", file).unwrap();
        assert_eq!(
            namespace::new(b"/library", &[entry]).err(),
            Some(file_error::invalid_entry)
        );
        assert_eq!(
            namespace::new(b"/lib/a", &[entry]).err(),
            Some(file_error::invalid_entry)
        );
        assert_eq!(
            namespace::new(b"/lib", &[entry, entry]).err(),
            Some(file_error::duplicate_path)
        );
        let child = namespace_entry::new(b"/lib/a/child", file).unwrap();
        for entries in [[entry, child], [child, entry]] {
            assert_eq!(
                namespace::new(b"/lib", &entries).err(),
                Some(file_error::conflicting_paths)
            );
        }
        let conflicting =
            namespace_entry::new(b"/lib/b", file_snapshot::new(1, 1, b"b").unwrap()).unwrap();
        assert_eq!(
            namespace::new(b"/lib", &[entry, conflicting]).err(),
            Some(file_error::inconsistent_file)
        );
        let alias = namespace_entry::new(b"/lib/b", file).unwrap();
        assert!(namespace::new(b"/lib", &[entry, alias]).is_ok());
        let version =
            namespace_entry::new(b"/lib/b", file_snapshot::new(1, 2, b"b").unwrap()).unwrap();
        assert!(namespace::new(b"/lib", &[entry, version]).is_ok());
        assert_eq!(owner().task(), 71);
        assert_eq!(owner().generation(), 1);
        assert_eq!(entry.path(), b"/lib/a");
        assert_eq!(file.object(), 1);
        assert_eq!(file.version(), 1);
        assert_eq!(file.len(), 1);
        assert!(!file.is_empty());
        assert!(file_snapshot::new(2, 1, b"").unwrap().is_empty());
    }

    #[test]
    fn descriptor_range_reservations_and_capacity_are_explicit() {
        assert_eq!(
            file_service::<1>::new(owner(), 0, 3).err(),
            Some(file_error::invalid_identity)
        );
        assert_eq!(
            file_service::<2>::new(owner(), 1, i32::MAX as u32).err(),
            Some(file_error::invalid_descriptor_range)
        );
        assert_eq!(
            file_service::<0>::new(owner(), 1, u32::MAX).err(),
            Some(file_error::invalid_descriptor_range)
        );
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut empty = file_service::<0>::new(owner(), 1, 3).unwrap();
        assert_eq!(
            empty.open(owner(), 1, &view, b"/lib/a", false, never),
            Err(file_error::table_full)
        );
        assert_eq!(empty.last_sequence(), 0);
        for first in [0, 3, i32::MAX as u32] {
            let mut service = file_service::<1>::new(owner(), 2, first).unwrap();
            let handle = service
                .open(owner(), 1, &view, b"/lib/a", true, |event| {
                    assert_eq!(event, publication::opened { descriptor: first });
                    publication_result::committed
                })
                .unwrap();
            assert_eq!(handle.fd(), first);
            assert_eq!(service.close_on_exec(owner(), handle), Ok(true));
            let before = state(&service);
            assert_eq!(
                service.open(owner(), 2, &view, b"/lib/a", false, never),
                Err(file_error::table_full)
            );
            assert_eq!(state(&service), before);
        }
    }

    #[test]
    fn bounded_reads_have_independent_offsets_zero_eof_and_short_semantics() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<3>::new(owner(), 1, 3).unwrap();
        let a = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        let b = service
            .open(owner(), 2, &view, b"/lib/a", true, commit)
            .unwrap();
        let empty = service
            .open(owner(), 3, &view, b"/lib/empty", false, commit)
            .unwrap();
        assert_eq!(service.close_on_exec(owner(), a), Ok(false));
        assert_eq!(
            service.read(owner(), 4, a, 0, 0, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 3,
                        offset: 0,
                        bytes: b""
                    }
                );
                publication_result::committed
            }),
            Ok(0)
        );
        let before = state(&service);
        assert_eq!(
            service.read(owner(), 5, a, 1, 0, never),
            Err(file_error::invalid_budget)
        );
        assert_eq!(state(&service), before);
        assert_eq!(
            service.read(owner(), 5, a, u64::MAX, 4, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 3,
                        offset: 0,
                        bytes: b"abcd"
                    }
                );
                publication_result::committed
            }),
            Ok(4)
        );
        assert_eq!(service.offset(owner(), a), Ok(4));
        assert_eq!(service.offset(owner(), b), Ok(0));
        assert_eq!(
            service.read(owner(), 6, b, 2, usize::MAX, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 4,
                        offset: 0,
                        bytes: b"ab"
                    }
                );
                publication_result::committed
            }),
            Ok(2)
        );
        assert_eq!(
            service.read(owner(), 7, a, u64::MAX, usize::MAX, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 3,
                        offset: 4,
                        bytes: b"ef"
                    }
                );
                publication_result::committed
            }),
            Ok(2)
        );
        for (sequence, handle, fd, offset) in [(8, a, 3, 6), (9, empty, 5, 0)] {
            assert_eq!(
                service.read(owner(), sequence, handle, u64::MAX, 0, |event| {
                    assert_eq!(
                        event,
                        publication::read {
                            descriptor: fd,
                            offset,
                            bytes: b""
                        }
                    );
                    publication_result::committed
                }),
                Ok(0)
            );
        }
        assert_eq!(service.last_sequence(), 9);
        assert_eq!(service.offset(owner(), b), Ok(2));
    }

    #[test]
    fn rejected_publication_preserves_every_committed_field_and_can_retry() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<2>::new(owner(), 1, 3).unwrap();
        let before = state(&service);
        assert_eq!(
            service.open(owner(), 1, &view, b"/lib/a", true, |event| {
                assert_eq!(event, publication::opened { descriptor: 3 });
                publication_result::rejected
            }),
            Err(file_error::publication_rejected)
        );
        assert_eq!(state(&service), before);
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", true, commit)
            .unwrap();
        assert_eq!(handle.generation(), 1);
        let before = state(&service);
        assert_eq!(
            service.read(owner(), 2, handle, 4, 3, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 3,
                        offset: 0,
                        bytes: b"abc"
                    }
                );
                publication_result::rejected
            }),
            Err(file_error::publication_rejected)
        );
        assert_eq!(state(&service), before);
        assert_eq!(service.read(owner(), 2, handle, 4, 3, commit), Ok(3));
        let before = state(&service);
        assert_eq!(
            service.close(owner(), 3, handle, |event| {
                assert_eq!(event, publication::closed { descriptor: 3 });
                publication_result::rejected
            }),
            Err(file_error::publication_rejected)
        );
        assert_eq!(state(&service), before);
        assert_eq!(service.close(owner(), 3, handle, commit), Ok(()));
        assert_eq!(service.open_count(), 0);
        assert_eq!(service.last_sequence(), 3);
    }

    #[test]
    fn local_errors_do_not_publish_or_advance_the_committed_watermark() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1>::new(owner(), 1, 3).unwrap();
        let before = state(&service);
        for (path, error) in [
            (b"/else/a".as_slice(), file_error::out_of_scope),
            (b"/lib/no", file_error::not_found),
            (b"/lib/../a", file_error::unsupported_path),
        ] {
            assert_eq!(
                service.open(owner(), 1, &view, path, false, never),
                Err(error)
            );
            assert_eq!(state(&service), before);
        }
        // The core permits a different body at this uncommitted sequence. An
        // adapter, not this watermark, binds its pending authenticated request.
        service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        assert_eq!(service.last_sequence(), 1);
    }

    #[test]
    fn unknown_open_read_and_close_revoke_every_handle() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        for operation in 0..3 {
            let mut service = file_service::<3>::new(owner(), operation + 1, 3).unwrap();
            let a = service
                .open(owner(), 1, &view, b"/lib/a", false, commit)
                .unwrap();
            let b = service
                .open(owner(), 2, &view, b"/lib/empty", false, commit)
                .unwrap();
            let outcome = match operation {
                0 => service
                    .open(owner(), 3, &view, b"/lib/a", false, |_| {
                        publication_result::unknown
                    })
                    .map(|_| ()),
                1 => service
                    .read(owner(), 3, a, 4, 4, |_| publication_result::unknown)
                    .map(|_| ()),
                _ => service.close(owner(), 3, a, |_| publication_result::unknown),
            };
            assert_eq!(outcome, Err(file_error::publication_unknown));
            assert!(service.is_fenced());
            assert_eq!(service.open_count(), 0);
            assert_eq!(service.last_sequence(), 2);
            assert_eq!(service.offset(owner(), a), Err(file_error::fenced));
            assert_eq!(service.file(owner(), b), Err(file_error::fenced));
            assert_eq!(
                service.handle_for_fd(owner(), b.fd()),
                Err(file_error::fenced)
            );
            assert_eq!(
                service.open(owner(), 4, &view, b"/lib/a", false, never),
                Err(file_error::fenced)
            );
            assert_eq!(
                service.read(owner(), 4, a, 4, 4, never),
                Err(file_error::fenced)
            );
            assert_eq!(service.close(owner(), 4, a, never), Err(file_error::fenced));
            assert_eq!(service.retire(owner()), Ok(()));
            assert!(service.is_fenced());
        }
    }

    #[test]
    fn publication_unwinding_fences_open_read_and_close_before_host_reuse() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        for operation in 0..3 {
            let mut service = file_service::<2>::new(owner(), operation + 1, 3).unwrap();
            let handle = service
                .open(owner(), 1, &view, b"/lib/a", false, commit)
                .unwrap();
            let mut entered = false;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let publish = |_: publication<'_>| {
                    entered = true;
                    panic!("external effect may already have happened");
                };
                match operation {
                    0 => service
                        .open(owner(), 2, &view, b"/lib/a", false, publish)
                        .map(|_| ()),
                    1 => service.read(owner(), 2, handle, 4, 4, publish).map(|_| ()),
                    _ => service.close(owner(), 2, handle, publish),
                }
            }));
            assert!(result.is_err());
            assert!(entered);
            assert!(service.is_fenced());
            assert_eq!(service.open_count(), 0);
            assert_eq!(service.last_sequence(), 1);
            assert_eq!(
                service.read(owner(), 2, handle, 4, 4, never),
                Err(file_error::fenced)
            );
        }
    }

    #[test]
    fn owners_service_incarnations_and_internal_generations_are_checked() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let owner_b = client_identity::new(owner().task(), 2).unwrap();
        let stranger = client_identity::new(72, owner().generation()).unwrap();
        let mut service = file_service::<2>::new(owner(), 10, 3).unwrap();
        let a = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        let before = state(&service);
        for wrong in [owner_b, stranger] {
            assert_eq!(
                service.open(wrong, 2, &view, b"/lib/a", false, never),
                Err(file_error::wrong_owner)
            );
            assert_eq!(
                service.read(wrong, 2, a, 1, 1, never),
                Err(file_error::wrong_owner)
            );
            assert_eq!(
                service.close(wrong, 2, a, never),
                Err(file_error::wrong_owner)
            );
            assert_eq!(service.retire(wrong), Err(file_error::wrong_owner));
            assert_eq!(
                service.handle_for_fd(wrong, 3),
                Err(file_error::wrong_owner)
            );
            assert_eq!(state(&service), before);
        }
        let mut recreated = file_service::<1>::new(owner(), 11, 3).unwrap();
        let b = recreated
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        assert_eq!(a.fd(), b.fd());
        assert_eq!(a.generation(), b.generation());
        assert_ne!(a.service_incarnation(), b.service_incarnation());
        assert_eq!(
            recreated.read(owner(), 2, a, 1, 1, never),
            Err(file_error::stale_handle)
        );
        let mut other_owner = file_service::<1>::new(owner_b, 12, 3).unwrap();
        let c = other_owner
            .open(owner_b, 1, &view, b"/lib/a", false, commit)
            .unwrap();
        assert_eq!(c.fd(), a.fd());
        assert_eq!(
            service.read(owner(), 2, c, 1, 1, never),
            Err(file_error::stale_handle)
        );
        let forged_generation = descriptor_handle {
            generation: a.generation() + 1,
            ..a
        };
        assert_eq!(
            service.read(owner(), 2, forged_generation, 1, 1, never),
            Err(file_error::stale_handle)
        );
        assert_eq!(
            service.handle_for_fd(owner(), 2),
            Err(file_error::bad_descriptor)
        );
        assert_eq!(
            service.handle_for_fd(owner(), 4),
            Err(file_error::bad_descriptor)
        );
        assert_eq!(
            service.handle_for_fd(owner(), u32::MAX),
            Err(file_error::bad_descriptor)
        );
        assert_eq!(state(&service), before);
    }

    #[test]
    fn reused_integer_fd_has_a_fresh_internal_handle_but_current_fd_lookup_succeeds() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<2>::new(owner(), 1, 3).unwrap();
        let a = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        let b = service
            .open(owner(), 2, &view, b"/lib/a", false, commit)
            .unwrap();
        service.close(owner(), 3, a, commit).unwrap();
        assert_eq!(service.offset(owner(), a), Err(file_error::stale_handle));
        let replacement = service
            .open(owner(), 4, &view, b"/lib/empty", false, commit)
            .unwrap();
        assert_eq!(replacement.fd(), a.fd());
        assert!(replacement.generation() > b.generation());
        assert_eq!(service.handle_for_fd(owner(), a.fd()), Ok(replacement));
        let before = state(&service);
        assert_eq!(
            service.read(owner(), 5, a, 1, 1, never),
            Err(file_error::stale_handle)
        );
        assert_eq!(
            service.close(owner(), 5, a, never),
            Err(file_error::stale_handle)
        );
        assert_eq!(state(&service), before);
    }

    #[test]
    fn replacement_namespace_cannot_rebind_an_already_open_file() {
        let old_file = file_snapshot::new(5, 1, b"old").unwrap();
        let new_file = file_snapshot::new(5, 2, b"replacement").unwrap();
        let old_entries = [namespace_entry::new(b"/lib/a", old_file).unwrap()];
        let new_entries = [namespace_entry::new(b"/lib/a", new_file).unwrap()];
        let old_view = namespace::new(b"/lib", &old_entries).unwrap();
        let new_view = namespace::new(b"/lib", &new_entries).unwrap();
        let mut service = file_service::<2>::new(owner(), 1, 3).unwrap();
        let old = service
            .open(owner(), 1, &old_view, b"/lib/a", false, commit)
            .unwrap();
        let new = service
            .open(owner(), 2, &new_view, b"/lib/a", false, commit)
            .unwrap();
        assert_eq!(service.file(owner(), old), Ok(old_file));
        assert_eq!(service.file(owner(), new), Ok(new_file));
        assert_eq!(
            service.read(owner(), 3, old, 100, 100, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 3,
                        offset: 0,
                        bytes: b"old"
                    }
                );
                publication_result::committed
            }),
            Ok(3)
        );
        assert_eq!(service.offset(owner(), new), Ok(0));
        assert_eq!(
            service.read(owner(), 4, new, 100, 100, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 4,
                        offset: 0,
                        bytes: b"replacement"
                    }
                );
                publication_result::committed
            }),
            Ok(11)
        );
    }

    #[test]
    fn sequence_and_handle_generation_exhaustion_never_wrap() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1>::new(owner(), 1, 3).unwrap();
        assert_eq!(
            service.open(owner(), 0, &view, b"/lib/a", false, never),
            Err(file_error::stale_invocation)
        );
        service.next_generation = u64::MAX;
        let before = state(&service);
        assert_eq!(
            service.open(owner(), 1, &view, b"/lib/a", false, |_| {
                publication_result::rejected
            }),
            Err(file_error::publication_rejected)
        );
        assert_eq!(state(&service), before);
        let last = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        assert_eq!(last.generation(), u64::MAX);
        assert_eq!(service.next_generation, 0);
        service.close(owner(), 2, last, commit).unwrap();
        let before = state(&service);
        assert_eq!(
            service.open(owner(), 3, &view, b"/lib/a", false, never),
            Err(file_error::generation_exhausted)
        );
        assert_eq!(state(&service), before);
        let mut service = file_service::<1>::new(owner(), 2, 3).unwrap();
        let last = service
            .open(owner(), u64::MAX, &view, b"/lib/a", false, commit)
            .unwrap();
        let before = state(&service);
        for sequence in [0, 1, u64::MAX] {
            assert_eq!(
                service.read(owner(), sequence, last, 1, 1, never),
                Err(file_error::stale_invocation)
            );
            assert_eq!(
                service.close(owner(), sequence, last, never),
                Err(file_error::stale_invocation)
            );
        }
        assert_eq!(state(&service), before);
        assert_eq!(service.retire(owner()), Ok(()));
        assert_eq!(service.open_count(), 0);
    }

    #[test]
    fn impossible_offset_is_refused_before_slice_or_publication() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1>::new(owner(), 1, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        // Fault injection into otherwise private state tests the checked bound.
        service.descriptors[0].as_mut().unwrap().offset = u64::MAX;
        let before = state(&service);
        assert_eq!(
            service.read(owner(), 2, handle, u64::MAX, usize::MAX, never),
            Err(file_error::offset_overflow)
        );
        assert_eq!(state(&service), before);
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum source_mode {
        exact,
        short,
        oversized,
        unavailable,
        changed,
        dirty_failure,
        unwind,
    }

    #[derive(Debug)]
    struct read_context {
        binding: u64,
        mode: source_mode,
        calls: usize,
    }

    impl read_context {
        fn new(binding: u64) -> Self {
            Self {
                binding,
                mode: source_mode::exact,
                calls: 0,
            }
        }
    }

    // A deterministic sparse source can cover u64 extents without allocating
    // or narrowing the whole source. Context modes model failures in a trusted
    // adapter; its binding is test correlation, never real caller authority.
    #[derive(Debug, Eq, PartialEq)]
    struct chunk_source {
        binding: u64,
        length: u64,
        seed: u8,
    }

    impl immutable_file_source for chunk_source {
        type read_context = read_context;

        fn byte_length(&self) -> u64 {
            self.length
        }

        fn read_at(
            &self,
            context: &mut read_context,
            offset: u64,
            destination: &mut [u8],
        ) -> Result<usize, source_error> {
            context.calls += 1;
            assert!(!destination.is_empty());
            assert!(destination.iter().all(|byte| *byte == 0));
            destination.fill(0xd5);
            if context.binding != self.binding {
                return Err(source_error::changed);
            }
            if offset
                .checked_add(destination.len() as u64)
                .is_none_or(|end| end > self.length)
            {
                return Err(source_error::invalid_extent);
            }
            match context.mode {
                source_mode::exact => {
                    for (index, byte) in destination.iter_mut().enumerate() {
                        *byte = offset.wrapping_add(index as u64) as u8 ^ self.seed;
                    }
                    Ok(destination.len())
                }
                source_mode::short => Ok(destination.len() - 1),
                source_mode::oversized => Ok(destination.len() + 1),
                source_mode::unavailable => Err(source_error::unavailable),
                source_mode::changed => Err(source_error::changed),
                source_mode::dirty_failure => Err(source_error::failed),
                source_mode::unwind => panic!("provider failed after dirtying scratch"),
            }
        }
    }

    fn chunk_state<'a, const capacity: usize>(
        service: &file_service<'a, capacity, chunk_source>,
    ) -> (
        [Option<descriptor<'a, chunk_source>>; capacity],
        u64,
        u64,
        bool,
    ) {
        (
            service.descriptors,
            service.next_generation,
            service.last_sequence,
            service.fenced,
        )
    }

    #[test]
    fn chunk_reads_share_descriptor_offsets_and_preserve_original_snapshot() {
        let old = chunk_source {
            binding: 80,
            length: 9,
            seed: 0x20,
        };
        let new = chunk_source {
            binding: 81,
            length: 5,
            seed: 0x40,
        };
        let old_file = file_snapshot::from_source(5, 1, &old).unwrap();
        let new_file = file_snapshot::from_source(5, 2, &new).unwrap();
        let old_entries = [namespace_entry::new(b"/lib/a", old_file).unwrap()];
        let new_entries = [namespace_entry::new(b"/lib/a", new_file).unwrap()];
        let old_view = namespace::new(b"/lib", &old_entries).unwrap();
        let new_view = namespace::new(b"/lib", &new_entries).unwrap();
        let mut service = file_service::<3, chunk_source>::new(owner(), 10, 3).unwrap();
        let first = service
            .open(owner(), 1, &old_view, b"/lib/a", false, commit)
            .unwrap();
        let second = service
            .open(owner(), 2, &old_view, b"/lib/a", true, commit)
            .unwrap();
        let replacement = service
            .open(owner(), 3, &new_view, b"/lib/a", false, commit)
            .unwrap();
        let mut context = read_context::new(80);
        let mut scratch = [0xa5; 8];
        for (sequence, handle, offset, expected) in [
            (4, first, 0, [0x20, 0x21, 0x22]),
            (5, first, 3, [0x23, 0x24, 0x25]),
            (6, second, 0, [0x20, 0x21, 0x22]),
        ] {
            assert_eq!(
                service.read_into(
                    owner(),
                    sequence,
                    handle,
                    u64::MAX,
                    3,
                    &mut context,
                    &mut scratch,
                    |event| {
                        assert_eq!(
                            event,
                            publication::read {
                                descriptor: handle.fd(),
                                offset,
                                bytes: &expected,
                            }
                        );
                        publication_result::committed
                    }
                ),
                Ok(3)
            );
            assert_eq!(scratch, [0; 8]);
        }
        assert_eq!(service.offset(owner(), first), Ok(6));
        assert_eq!(service.offset(owner(), second), Ok(3));
        assert_eq!(service.offset(owner(), replacement), Ok(0));
        assert_eq!(service.file(owner(), first), Ok(old_file));
        assert_eq!(service.file(owner(), replacement), Ok(new_file));
        assert!(core::ptr::eq(old_file.source(), &old));
        assert_eq!(old_file.byte_length(), 9);
        context.binding = 81;
        assert_eq!(
            service.read_into(
                owner(),
                7,
                replacement,
                2,
                3,
                &mut context,
                &mut scratch,
                |event| {
                    assert_eq!(
                        event,
                        publication::read {
                            descriptor: replacement.fd(),
                            offset: 0,
                            bytes: &[0x40, 0x41],
                        }
                    );
                    publication_result::committed
                }
            ),
            Ok(2)
        );
        assert_eq!(service.offset(owner(), first), Ok(6));
        assert_eq!(context.calls, 4);
        service.close(owner(), 8, first, commit).unwrap();
        let before = chunk_state(&service);
        scratch.fill(0xa5);
        assert_eq!(
            service.read_into(owner(), 9, first, 1, 1, &mut context, &mut scratch, never),
            Err(file_error::stale_handle)
        );
        assert_eq!(chunk_state(&service), before);
        assert_eq!(context.calls, 4);
        assert_eq!(scratch, [0; 8]);
    }

    #[test]
    fn chunk_errors_never_publish_dirty_prefix_or_change_committed_state() {
        let source = chunk_source {
            binding: 80,
            length: 9,
            seed: 0,
        };
        let entries = [namespace_entry::new(
            b"/lib/a",
            file_snapshot::from_source(5, 1, &source).unwrap(),
        )
        .unwrap()];
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1, chunk_source>::new(owner(), 10, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        let before = chunk_state(&service);
        let mut context = read_context::new(80);
        let mut scratch = [0xa5; 8];
        for (mode, error) in [
            (source_mode::short, file_error::source_length_mismatch),
            (source_mode::oversized, file_error::source_length_mismatch),
            (
                source_mode::unavailable,
                file_error::source_failure(source_error::unavailable),
            ),
            (
                source_mode::changed,
                file_error::source_failure(source_error::changed),
            ),
            (
                source_mode::dirty_failure,
                file_error::source_failure(source_error::failed),
            ),
        ] {
            context.mode = mode;
            scratch.fill(0xa5);
            assert_eq!(
                service.read_into(owner(), 2, handle, 4, 4, &mut context, &mut scratch, never),
                Err(error)
            );
            assert_eq!(chunk_state(&service), before);
            assert_eq!(scratch, [0; 8]);
        }
        context.mode = source_mode::exact;
        context.binding = 81;
        assert_eq!(
            service.read_into(owner(), 2, handle, 4, 4, &mut context, &mut scratch, never),
            Err(file_error::source_failure(source_error::changed))
        );
        assert_eq!(chunk_state(&service), before);
        assert_eq!(scratch, [0; 8]);
        context.binding = 80;
        assert_eq!(
            service.read_into(
                owner(),
                2,
                handle,
                4,
                4,
                &mut context,
                &mut scratch,
                |event| {
                    assert_eq!(
                        event,
                        publication::read {
                            descriptor: 3,
                            offset: 0,
                            bytes: &[0, 1, 2, 3]
                        }
                    );
                    publication_result::committed
                }
            ),
            Ok(4)
        );
        assert_eq!(service.offset(owner(), handle), Ok(4));
        assert_eq!(context.calls, 7);
        assert_eq!(scratch, [0; 8]);
    }

    #[test]
    fn zero_eof_and_validation_errors_clear_scratch_without_backend_calls() {
        let source = chunk_source {
            binding: 80,
            length: 3,
            seed: 0,
        };
        let entries = [namespace_entry::new(
            b"/lib/a",
            file_snapshot::from_source(5, 1, &source).unwrap(),
        )
        .unwrap()];
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1, chunk_source>::new(owner(), 10, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        let before = chunk_state(&service);
        let mut context = read_context::new(80);
        context.mode = source_mode::unwind;
        let mut scratch = [0xa5; 2];
        for (client, sequence, count, budget, error) in [
            (
                client_identity::new(72, 1).unwrap(),
                2,
                1,
                1,
                file_error::wrong_owner,
            ),
            (owner(), 1, 1, 1, file_error::stale_invocation),
            (owner(), 2, 1, 0, file_error::invalid_budget),
            (owner(), 2, 3, 3, file_error::invalid_scratch),
        ] {
            scratch.fill(0xa5);
            assert_eq!(
                service.read_into(
                    client,
                    sequence,
                    handle,
                    count,
                    budget,
                    &mut context,
                    &mut scratch,
                    never
                ),
                Err(error)
            );
            assert_eq!(chunk_state(&service), before);
            assert_eq!(scratch, [0; 2]);
        }
        assert_eq!(
            service.read_into(
                owner(),
                2,
                handle,
                0,
                0,
                &mut context,
                &mut scratch,
                |event| {
                    assert_eq!(
                        event,
                        publication::read {
                            descriptor: 3,
                            offset: 0,
                            bytes: &[]
                        }
                    );
                    publication_result::committed
                }
            ),
            Ok(0)
        );
        assert_eq!(service.offset(owner(), handle), Ok(0));
        context.mode = source_mode::exact;
        assert_eq!(
            service.read_into(
                owner(),
                3,
                handle,
                3,
                3,
                &mut context,
                &mut [0xa5; 3],
                commit
            ),
            Ok(3)
        );
        context.mode = source_mode::unwind;
        scratch.fill(0xa5);
        assert_eq!(
            service.read_into(
                owner(),
                4,
                handle,
                u64::MAX,
                0,
                &mut context,
                &mut scratch,
                |event| {
                    assert_eq!(
                        event,
                        publication::read {
                            descriptor: 3,
                            offset: 3,
                            bytes: &[]
                        }
                    );
                    publication_result::committed
                }
            ),
            Ok(0)
        );
        assert_eq!(scratch, [0; 2]);
        assert_eq!(context.calls, 1);
        assert_eq!(service.last_sequence(), 4);
        assert_eq!(service.offset(owner(), handle), Ok(3));
    }

    #[test]
    fn u64_source_offsets_are_bounded_without_whole_source_usize_conversion() {
        let source = chunk_source {
            binding: 80,
            length: u64::MAX,
            seed: 0,
        };
        let file = file_snapshot::from_source(5, 1, &source).unwrap();
        let entries = [namespace_entry::new(b"/lib/a", file).unwrap()];
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1, chunk_source>::new(owner(), 10, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        assert_eq!(file.byte_length(), u64::MAX);
        // Private-state injection places the descriptor near the representable
        // end without iterating over its entire virtual contents. On a 32-bit
        // target this offset and source length both exceed usize::MAX.
        service.descriptors[0].as_mut().unwrap().offset = u64::MAX - 3;
        let mut context = read_context::new(80);
        let mut scratch = [0xa5; 8];
        assert_eq!(
            service.read_into(
                owner(),
                2,
                handle,
                u64::MAX,
                usize::MAX,
                &mut context,
                &mut scratch,
                |event| {
                    assert_eq!(
                        event,
                        publication::read {
                            descriptor: 3,
                            offset: u64::MAX - 3,
                            bytes: &[0xfc, 0xfd, 0xfe],
                        }
                    );
                    publication_result::committed
                }
            ),
            Ok(3)
        );
        assert_eq!(service.offset(owner(), handle), Ok(u64::MAX));
        assert_eq!(scratch, [0; 8]);
        assert_eq!(
            service.read_into(
                owner(),
                3,
                handle,
                u64::MAX,
                0,
                &mut context,
                &mut [],
                commit
            ),
            Ok(0)
        );
        assert_eq!(context.calls, 1);
    }

    #[test]
    fn rejected_chunk_publication_retries_exact_bytes_and_unknown_fences() {
        let source = chunk_source {
            binding: 80,
            length: 8,
            seed: 0,
        };
        let entries = [namespace_entry::new(
            b"/lib/a",
            file_snapshot::from_source(5, 1, &source).unwrap(),
        )
        .unwrap()];
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1, chunk_source>::new(owner(), 10, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        let before = chunk_state(&service);
        let mut context = read_context::new(80);
        let mut scratch = [0xa5; 8];
        let mut publications = 0;
        for outcome in [publication_result::rejected, publication_result::committed] {
            assert_eq!(
                service.read_into(
                    owner(),
                    2,
                    handle,
                    4,
                    4,
                    &mut context,
                    &mut scratch,
                    |event| {
                        publications += 1;
                        assert_eq!(
                            event,
                            publication::read {
                                descriptor: 3,
                                offset: 0,
                                bytes: &[0, 1, 2, 3],
                            }
                        );
                        outcome
                    }
                ),
                if outcome == publication_result::committed {
                    Ok(4)
                } else {
                    Err(file_error::publication_rejected)
                }
            );
            assert_eq!(scratch, [0; 8]);
            if outcome == publication_result::rejected {
                assert_eq!(chunk_state(&service), before);
            }
        }
        assert_eq!(publications, 2);
        assert_eq!(service.offset(owner(), handle), Ok(4));
        assert_eq!(service.last_sequence(), 2);
        assert_eq!(
            service.read_into(
                owner(),
                3,
                handle,
                4,
                4,
                &mut context,
                &mut scratch,
                |event| {
                    assert_eq!(
                        event,
                        publication::read {
                            descriptor: 3,
                            offset: 4,
                            bytes: &[4, 5, 6, 7],
                        }
                    );
                    publication_result::unknown
                }
            ),
            Err(file_error::publication_unknown)
        );
        assert_eq!(scratch, [0; 8]);
        assert!(service.is_fenced());
        assert_eq!(service.open_count(), 0);
        assert_eq!(service.last_sequence(), 2);
        assert_eq!(
            service.read_into(owner(), 3, handle, 1, 1, &mut context, &mut scratch, never),
            Err(file_error::fenced)
        );
        assert_eq!(context.calls, 3);
    }

    #[test]
    fn backend_and_publication_unwind_clear_all_scratch_and_fence_service() {
        let source = chunk_source {
            binding: 80,
            length: 8,
            seed: 0,
        };
        let entries = [namespace_entry::new(
            b"/lib/a",
            file_snapshot::from_source(5, 1, &source).unwrap(),
        )
        .unwrap()];
        let view = namespace::new(b"/lib", &entries).unwrap();
        for provider_panics in [true, false] {
            let mut service = file_service::<1, chunk_source>::new(owner(), 10, 3).unwrap();
            let handle = service
                .open(owner(), 1, &view, b"/lib/a", false, commit)
                .unwrap();
            let mut context = read_context::new(80);
            context.mode = if provider_panics {
                source_mode::unwind
            } else {
                source_mode::exact
            };
            let mut scratch = [0xa5; 8];
            let mut published = false;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let _ = service.read_into(
                    owner(),
                    2,
                    handle,
                    4,
                    4,
                    &mut context,
                    &mut scratch,
                    |event| {
                        published = true;
                        assert_eq!(
                            event,
                            publication::read {
                                descriptor: 3,
                                offset: 0,
                                bytes: &[0, 1, 2, 3],
                            }
                        );
                        panic!("lost publication acknowledgement");
                    },
                );
            }));
            assert!(result.is_err());
            assert_eq!(published, !provider_panics);
            assert_eq!(scratch, [0; 8]);
            assert!(service.is_fenced());
            assert_eq!(service.open_count(), 0);
            assert_eq!(service.last_sequence(), 1);
            assert_eq!(context.calls, 1);
        }
    }

    #[test]
    fn same_file_identity_requires_consistent_provider_and_snapshot_length() {
        use std::cell::Cell;

        #[derive(Debug, Eq, PartialEq)]
        struct changing_source {
            length: Cell<u64>,
            change_during_read: Cell<bool>,
        }
        impl immutable_file_source for changing_source {
            type read_context = usize;

            fn byte_length(&self) -> u64 {
                self.length.get()
            }

            fn read_at(
                &self,
                calls: &mut usize,
                _: u64,
                destination: &mut [u8],
            ) -> Result<usize, source_error> {
                *calls += 1;
                destination.fill(0xd5);
                if self.change_during_read.get() {
                    self.length.set(self.length.get() + 1);
                }
                Ok(destination.len())
            }
        }

        let a = chunk_source {
            binding: 80,
            length: 8,
            seed: 0,
        };
        let b = chunk_source {
            binding: 81,
            length: 8,
            seed: 0,
        };
        let first =
            namespace_entry::new(b"/lib/a", file_snapshot::from_source(5, 1, &a).unwrap()).unwrap();
        let changed =
            namespace_entry::new(b"/lib/b", file_snapshot::from_source(5, 1, &b).unwrap()).unwrap();
        assert_eq!(
            namespace::new(b"/lib", &[first, changed]).err(),
            Some(file_error::inconsistent_file)
        );
        let alias = namespace_entry::new(b"/lib/b", first.file()).unwrap();
        assert!(namespace::new(b"/lib", &[first, alias]).is_ok());

        for change_during_read in [false, true] {
            let source = changing_source {
                length: Cell::new(8),
                change_during_read: Cell::new(change_during_read),
            };
            let snapshot = file_snapshot::from_source(5, 1, &source).unwrap();
            let entries = [namespace_entry::new(b"/lib/a", snapshot).unwrap()];
            let view = namespace::new(b"/lib", &entries).unwrap();
            let mut service = file_service::<1, changing_source>::new(owner(), 10, 3).unwrap();
            let handle = service
                .open(owner(), 1, &view, b"/lib/a", false, commit)
                .unwrap();
            if !change_during_read {
                source.length.set(9);
                let changed = namespace_entry::new(
                    b"/lib/b",
                    file_snapshot::from_source(5, 1, &source).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    namespace::new(b"/lib", &[entries[0], changed]).err(),
                    Some(file_error::inconsistent_file)
                );
            }
            let mut calls = 0;
            let mut scratch = [0xa5; 8];
            assert_eq!(
                service.read_into(owner(), 2, handle, 4, 4, &mut calls, &mut scratch, never),
                Err(file_error::source_failure(source_error::changed))
            );
            assert_eq!(calls, usize::from(change_during_read));
            assert_eq!(scratch, [0; 8]);
            assert_eq!(service.offset(owner(), handle), Ok(0));
            assert_eq!(service.last_sequence(), 1);
            assert!(!service.is_fenced());
        }
    }

    #[test]
    fn default_slice_provider_supports_the_same_bounded_scratch_interface() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1>::new(owner(), 10, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        let mut scratch = [0xa5; 8];
        assert_eq!(
            service.read_into(owner(), 2, handle, 4, 3, &mut (), &mut scratch, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 3,
                        offset: 0,
                        bytes: b"abc"
                    }
                );
                publication_result::committed
            }),
            Ok(3)
        );
        assert_eq!(scratch, [0; 8]);
        assert_eq!(
            service.read(owner(), 3, handle, 4, 4, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 3,
                        offset: 3,
                        bytes: b"def"
                    }
                );
                publication_result::committed
            }),
            Ok(3)
        );
        assert_eq!(service.offset(owner(), handle), Ok(6));
    }

    #[test]
    fn positional_reads_preserve_offset_across_slice_and_scratch_apis() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1>::new(owner(), 10, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", true, commit)
            .unwrap();
        assert_eq!(service.read(owner(), 2, handle, 2, 2, commit), Ok(2));
        let retained = service.descriptors;
        assert_eq!(
            service.read_at(owner(), 3, handle, 4, u64::MAX, 3, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 3,
                        offset: 4,
                        bytes: b"ef"
                    }
                );
                publication_result::committed
            }),
            Ok(2)
        );
        assert_eq!(service.descriptors, retained);
        assert_eq!(service.last_sequence(), 3);
        let mut scratch = [0xa5; 8];
        assert_eq!(
            service.read_at_into(
                owner(),
                4,
                handle,
                0,
                u64::MAX,
                3,
                &mut (),
                &mut scratch,
                |event| {
                    assert_eq!(
                        event,
                        publication::read {
                            descriptor: 3,
                            offset: 0,
                            bytes: b"abc"
                        }
                    );
                    publication_result::committed
                }
            ),
            Ok(3)
        );
        assert_eq!(scratch, [0; 8]);
        assert_eq!(service.descriptors, retained);
        assert_eq!(service.last_sequence(), 4);
        assert_eq!(
            service.read(owner(), 5, handle, 2, 2, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 3,
                        offset: 2,
                        bytes: b"cd"
                    }
                );
                publication_result::committed
            }),
            Ok(2)
        );
        assert_eq!(service.offset(owner(), handle), Ok(4));
    }

    #[test]
    fn positional_zero_and_beyond_eof_reads_preserve_the_requested_offset() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1>::new(owner(), 10, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        for (index, (offset, count)) in [(0, 0), (6, u64::MAX), (7, 1), (u64::MAX, u64::MAX)]
            .into_iter()
            .enumerate()
        {
            let check = |event: publication<'_>| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 3,
                        offset,
                        bytes: &[]
                    }
                );
                publication_result::committed
            };
            let sequence = 2 + index as u64 * 2;
            assert_eq!(
                service.read_at(owner(), sequence, handle, offset, count, 0, check),
                Ok(0)
            );
            let mut scratch = [0xa5; 8];
            assert_eq!(
                service.read_at_into(
                    owner(),
                    sequence + 1,
                    handle,
                    offset,
                    count,
                    0,
                    &mut (),
                    &mut scratch,
                    check
                ),
                Ok(0)
            );
            assert_eq!(scratch, [0; 8]);
            assert_eq!(service.offset(owner(), handle), Ok(0));
            assert_eq!(service.last_sequence(), sequence + 1);
        }
    }

    #[test]
    fn positional_wide_source_reads_do_not_narrow_or_advance_the_descriptor() {
        let source = chunk_source {
            binding: 80,
            length: u64::MAX,
            seed: 0,
        };
        let entries = [namespace_entry::new(
            b"/lib/a",
            file_snapshot::from_source(5, 1, &source).unwrap(),
        )
        .unwrap()];
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<2, chunk_source>::new(owner(), 10, 3).unwrap();
        let first = service
            .open(owner(), 1, &view, b"/lib/a", true, commit)
            .unwrap();
        let second = service
            .open(owner(), 2, &view, b"/lib/a", false, commit)
            .unwrap();
        let mut context = read_context::new(80);
        let mut scratch = [0xa5; 8];
        assert_eq!(
            service.read_into(owner(), 3, first, 2, 2, &mut context, &mut scratch, commit),
            Ok(2)
        );
        let retained = service.descriptors;
        assert_eq!(
            service.read_at_into(
                owner(),
                4,
                first,
                u64::MAX - 3,
                u64::MAX,
                8,
                &mut context,
                &mut scratch,
                |event| {
                    assert_eq!(
                        event,
                        publication::read {
                            descriptor: 3,
                            offset: u64::MAX - 3,
                            bytes: &[252, 253, 254]
                        }
                    );
                    publication_result::committed
                }
            ),
            Ok(3)
        );
        assert_eq!(service.descriptors, retained);
        assert_eq!(service.last_sequence(), 4);
        assert_eq!(scratch, [0; 8]);
        // The invalid context would reject any provider access. EOF must avoid it.
        context.binding = 0;
        assert_eq!(
            service.read_at_into(
                owner(),
                5,
                second,
                u64::MAX,
                1,
                0,
                &mut context,
                &mut scratch,
                commit
            ),
            Ok(0)
        );
        assert_eq!(context.calls, 2);
        assert_eq!(service.descriptors, retained);
        assert_eq!(service.last_sequence(), 5);
    }

    #[test]
    fn positional_validation_rejects_before_source_or_publication_and_clears_scratch() {
        let source = chunk_source {
            binding: 80,
            length: 8,
            seed: 0,
        };
        let entries = [namespace_entry::new(
            b"/lib/a",
            file_snapshot::from_source(5, 1, &source).unwrap(),
        )
        .unwrap()];
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1, chunk_source>::new(owner(), 10, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        let mut stale = handle;
        stale.generation += 1;
        let before = chunk_state(&service);
        let wrong = client_identity::new(owner().task(), owner().generation() + 1).unwrap();
        for (identity, sequence, selected, budget, error) in [
            (wrong, 2, handle, 4, file_error::wrong_owner),
            (owner(), 0, handle, 4, file_error::stale_invocation),
            (owner(), 1, handle, 4, file_error::stale_invocation),
            (owner(), 2, stale, 4, file_error::stale_handle),
            (owner(), 2, handle, 0, file_error::invalid_budget),
            (owner(), 2, handle, 4, file_error::invalid_scratch),
        ] {
            let mut context = read_context::new(80);
            let mut scratch = [0xa5; 3];
            assert_eq!(
                service.read_at_into(
                    identity,
                    sequence,
                    selected,
                    2,
                    4,
                    budget,
                    &mut context,
                    &mut scratch,
                    never
                ),
                Err(error)
            );
            assert_eq!(context.calls, 0);
            assert_eq!(scratch, [0; 3]);
            assert_eq!(chunk_state(&service), before);
        }
    }

    #[test]
    fn positional_rejection_retries_without_offset_change_and_unknown_fences() {
        let source = chunk_source {
            binding: 80,
            length: 8,
            seed: 0,
        };
        let entries = [namespace_entry::new(
            b"/lib/a",
            file_snapshot::from_source(5, 1, &source).unwrap(),
        )
        .unwrap()];
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1, chunk_source>::new(owner(), 10, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        let mut context = read_context::new(80);
        let mut scratch = [0xa5; 8];
        assert_eq!(
            service.read_into(owner(), 2, handle, 2, 2, &mut context, &mut scratch, commit),
            Ok(2)
        );
        let before = chunk_state(&service);
        for (sequence, outcome, expected) in [
            (
                3,
                publication_result::rejected,
                Err(file_error::publication_rejected),
            ),
            (3, publication_result::committed, Ok(4)),
            (
                4,
                publication_result::unknown,
                Err(file_error::publication_unknown),
            ),
        ] {
            assert_eq!(
                service.read_at_into(
                    owner(),
                    sequence,
                    handle,
                    3,
                    4,
                    4,
                    &mut context,
                    &mut scratch,
                    |event| {
                        assert_eq!(
                            event,
                            publication::read {
                                descriptor: 3,
                                offset: 3,
                                bytes: &[3, 4, 5, 6]
                            }
                        );
                        outcome
                    }
                ),
                expected
            );
            assert_eq!(scratch, [0; 8]);
            match outcome {
                publication_result::rejected => assert_eq!(chunk_state(&service), before),
                publication_result::committed => {
                    assert_eq!(service.descriptors, before.0);
                    assert_eq!(service.last_sequence(), 3);
                }
                publication_result::unknown => {
                    assert!(service.is_fenced());
                    assert_eq!(service.open_count(), 0);
                    assert_eq!(service.last_sequence(), 3);
                }
            }
        }
        assert_eq!(
            service.read_at_into(
                owner(),
                4,
                handle,
                3,
                4,
                4,
                &mut context,
                &mut scratch,
                never
            ),
            Err(file_error::fenced)
        );
        assert_eq!(context.calls, 4);
    }

    #[test]
    fn positional_source_failures_and_unwinds_clear_scratch_and_contain_unknown_effects() {
        let source = chunk_source {
            binding: 80,
            length: 8,
            seed: 0,
        };
        let entries = [namespace_entry::new(
            b"/lib/a",
            file_snapshot::from_source(5, 1, &source).unwrap(),
        )
        .unwrap()];
        let view = namespace::new(b"/lib", &entries).unwrap();
        for (mode, expected) in [
            (source_mode::short, file_error::source_length_mismatch),
            (source_mode::oversized, file_error::source_length_mismatch),
            (
                source_mode::unavailable,
                file_error::source_failure(source_error::unavailable),
            ),
            (
                source_mode::changed,
                file_error::source_failure(source_error::changed),
            ),
            (
                source_mode::dirty_failure,
                file_error::source_failure(source_error::failed),
            ),
        ] {
            let mut service = file_service::<1, chunk_source>::new(owner(), 10, 3).unwrap();
            let handle = service
                .open(owner(), 1, &view, b"/lib/a", false, commit)
                .unwrap();
            let before = chunk_state(&service);
            let mut context = read_context::new(80);
            context.mode = mode;
            let mut scratch = [0xa5; 8];
            assert_eq!(
                service.read_at_into(
                    owner(),
                    2,
                    handle,
                    2,
                    4,
                    4,
                    &mut context,
                    &mut scratch,
                    never
                ),
                Err(expected)
            );
            assert_eq!(chunk_state(&service), before);
            assert_eq!(context.calls, 1);
            assert_eq!(scratch, [0; 8]);
        }
        for provider_panics in [false, true] {
            let mut service = file_service::<1, chunk_source>::new(owner(), 10, 3).unwrap();
            let handle = service
                .open(owner(), 1, &view, b"/lib/a", false, commit)
                .unwrap();
            let mut context = read_context::new(80);
            context.mode = if provider_panics {
                source_mode::unwind
            } else {
                source_mode::exact
            };
            let mut scratch = [0xa5; 8];
            let mut published = false;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let _ = service.read_at_into(
                    owner(),
                    2,
                    handle,
                    2,
                    4,
                    4,
                    &mut context,
                    &mut scratch,
                    |event| {
                        published = true;
                        assert_eq!(
                            event,
                            publication::read {
                                descriptor: 3,
                                offset: 2,
                                bytes: &[2, 3, 4, 5]
                            }
                        );
                        panic!("lost positional publication acknowledgement");
                    },
                );
            }));
            assert!(result.is_err());
            assert_eq!(published, !provider_panics);
            assert!(service.is_fenced());
            assert_eq!(service.open_count(), 0);
            assert_eq!(service.last_sequence(), 1);
            assert_eq!(context.calls, 1);
            assert_eq!(scratch, [0; 8]);
        }
    }

    #[test]
    fn metadata_retains_each_opened_snapshot_and_preserves_descriptor_state() {
        let old = [
            namespace_entry::new(b"/lib/a", file_snapshot::new(5, 1, b"abcdef").unwrap()).unwrap(),
        ];
        let replacement =
            [
                namespace_entry::new(b"/lib/a", file_snapshot::new(5, 2, b"replacement").unwrap())
                    .unwrap(),
            ];
        let old_view = namespace::new(b"/lib", &old).unwrap();
        let new_view = namespace::new(b"/lib", &replacement).unwrap();
        let mut service = file_service::<2>::new(owner(), 10, 3).unwrap();
        let first = service
            .open(owner(), 1, &old_view, b"/lib/a", true, commit)
            .unwrap();
        let second = service
            .open(owner(), 2, &new_view, b"/lib/a", false, commit)
            .unwrap();
        assert_eq!(service.read(owner(), 3, first, 2, 2, commit), Ok(2));
        let retained = service.descriptors;
        let next_generation = service.next_generation;
        for (sequence, handle, version, length) in [(4, first, 1, 6), (5, second, 2, 11)] {
            assert_eq!(
                service.metadata(owner(), sequence, handle, |metadata| {
                    assert_eq!(
                        (
                            metadata.object(),
                            metadata.version(),
                            metadata.byte_length()
                        ),
                        (5, version, length)
                    );
                    publication_result::committed
                }),
                Ok(())
            );
            assert_eq!(service.descriptors, retained);
            assert_eq!(service.next_generation, next_generation);
            assert_eq!(service.last_sequence(), sequence);
        }
        assert_eq!(
            service.read(owner(), 6, first, 2, 2, |event| {
                assert_eq!(
                    event,
                    publication::read {
                        descriptor: 3,
                        offset: 2,
                        bytes: b"cd"
                    }
                );
                publication_result::committed
            }),
            Ok(2)
        );
    }

    #[test]
    fn metadata_validation_rejects_wrong_or_retired_authority_without_publication() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1>::new(owner(), 10, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        let before = state(&service);
        let wrong = client_identity::new(owner().task(), owner().generation() + 1).unwrap();
        let mut stale_generation = handle;
        stale_generation.generation += 1;
        let mut stale_service = handle;
        stale_service.service_incarnation += 1;
        let mut bad_descriptor = handle;
        bad_descriptor.descriptor += 1;
        for (identity, sequence, selected, error) in [
            (wrong, 2, handle, file_error::wrong_owner),
            (owner(), 0, handle, file_error::stale_invocation),
            (owner(), 1, handle, file_error::stale_invocation),
            (owner(), 2, stale_generation, file_error::stale_handle),
            (owner(), 2, stale_service, file_error::stale_handle),
            (owner(), 2, bad_descriptor, file_error::bad_descriptor),
        ] {
            assert_eq!(
                service.metadata(identity, sequence, selected, |_| panic!(
                    "invalid metadata published"
                )),
                Err(error)
            );
            assert_eq!(state(&service), before);
        }
        assert_eq!(service.close(owner(), 2, handle, commit), Ok(()));
        assert_eq!(
            service.metadata(owner(), 3, handle, |_| panic!("closed metadata published")),
            Err(file_error::stale_handle)
        );
        let reopened = service
            .open(owner(), 3, &view, b"/lib/a", false, commit)
            .unwrap();
        assert_eq!(
            service.metadata(owner(), 4, handle, |_| panic!("reused metadata published")),
            Err(file_error::stale_handle)
        );
        service.retire(owner()).unwrap();
        assert_eq!(
            service.metadata(owner(), 4, reopened, |_| panic!(
                "retired metadata published"
            )),
            Err(file_error::fenced)
        );
    }

    #[test]
    fn metadata_rejection_retries_and_unknown_or_unwind_fences_all_handles() {
        let entries = entries();
        let view = namespace::new(b"/lib", &entries).unwrap();
        for unwind in [false, true] {
            let mut service = file_service::<2>::new(owner(), 10, 3).unwrap();
            let handle = service
                .open(owner(), 1, &view, b"/lib/a", true, commit)
                .unwrap();
            let peer = service
                .open(owner(), 2, &view, b"/lib/empty", false, commit)
                .unwrap();
            assert_eq!(service.read(owner(), 3, handle, 2, 2, commit), Ok(2));
            let before = state(&service);
            assert_eq!(
                service.metadata(owner(), 4, handle, |metadata| {
                    assert_eq!(metadata.byte_length(), 6);
                    publication_result::rejected
                }),
                Err(file_error::publication_rejected)
            );
            assert_eq!(state(&service), before);
            assert_eq!(
                service.metadata(owner(), 4, handle, |metadata| {
                    assert_eq!(
                        (
                            metadata.object(),
                            metadata.version(),
                            metadata.byte_length()
                        ),
                        (5, 1, 6)
                    );
                    publication_result::committed
                }),
                Ok(())
            );
            assert_eq!(service.descriptors, before.0);
            assert_eq!(service.last_sequence(), 4);
            let result = catch_unwind(AssertUnwindSafe(|| {
                service.metadata(owner(), 5, handle, |metadata| {
                    assert_eq!(metadata.byte_length(), 6);
                    if unwind {
                        panic!("lost metadata acknowledgement");
                    }
                    publication_result::unknown
                })
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Err(file_error::publication_unknown));
            }
            assert!(service.is_fenced());
            assert_eq!(service.open_count(), 0);
            assert_eq!(service.last_sequence(), 4);
            assert_eq!(
                service.metadata(owner(), 5, peer, |_| panic!("fenced metadata published")),
                Err(file_error::fenced)
            );
        }
    }

    #[test]
    fn metadata_uses_retained_unsigned_length_without_a_byte_provider_call() {
        use std::cell::Cell;
        #[derive(Eq, PartialEq)]
        struct metadata_source {
            forbid_access: Cell<bool>,
        }
        impl immutable_file_source for metadata_source {
            type read_context = ();
            fn byte_length(&self) -> u64 {
                assert!(
                    !self.forbid_access.get(),
                    "metadata reread the source length"
                );
                u64::MAX
            }
            fn read_at(&self, _: &mut (), _: u64, _: &mut [u8]) -> Result<usize, source_error> {
                panic!("metadata read source bytes")
            }
        }
        let source = metadata_source {
            forbid_access: Cell::new(false),
        };
        let snapshot = file_snapshot::from_source(u64::MAX, u64::MAX, &source).unwrap();
        let entries = [namespace_entry::new(b"/lib/a", snapshot).unwrap()];
        let view = namespace::new(b"/lib", &entries).unwrap();
        let mut service = file_service::<1, metadata_source>::new(owner(), 10, 3).unwrap();
        let handle = service
            .open(owner(), 1, &view, b"/lib/a", false, commit)
            .unwrap();
        source.forbid_access.set(true);
        assert_eq!(
            service.metadata(owner(), 2, handle, |metadata| {
                assert_eq!(
                    (
                        metadata.object(),
                        metadata.version(),
                        metadata.byte_length()
                    ),
                    (u64::MAX, u64::MAX, u64::MAX)
                );
                publication_result::committed
            }),
            Ok(())
        );
        assert_eq!(service.offset(owner(), handle), Ok(0));
        assert_eq!(service.last_sequence(), 2);
    }

    #[test]
    fn provider_length_unwind_before_and_after_read_fences_and_clears_scratch() {
        use std::cell::Cell;

        #[derive(Debug, Eq, PartialEq)]
        struct length_probe {
            calls: Cell<usize>,
            panic_at: usize,
        }
        impl immutable_file_source for length_probe {
            type read_context = usize;

            fn byte_length(&self) -> u64 {
                let calls = self.calls.get() + 1;
                self.calls.set(calls);
                assert_ne!(calls, self.panic_at, "length provider unwound");
                8
            }

            fn read_at(
                &self,
                calls: &mut usize,
                _: u64,
                destination: &mut [u8],
            ) -> Result<usize, source_error> {
                *calls += 1;
                destination.fill(0xd5);
                Ok(destination.len())
            }
        }

        for panic_at in [2, 3] {
            let source = length_probe {
                calls: Cell::new(0),
                panic_at,
            };
            let snapshot = file_snapshot::from_source(5, 1, &source).unwrap();
            let entries = [namespace_entry::new(b"/lib/a", snapshot).unwrap()];
            let view = namespace::new(b"/lib", &entries).unwrap();
            let mut service = file_service::<1, length_probe>::new(owner(), 10, 3).unwrap();
            let handle = service
                .open(owner(), 1, &view, b"/lib/a", false, commit)
                .unwrap();
            let mut read_calls = 0;
            let mut scratch = [0xa5; 8];
            let result = catch_unwind(AssertUnwindSafe(|| {
                let _ = service.read_into(
                    owner(),
                    2,
                    handle,
                    4,
                    4,
                    &mut read_calls,
                    &mut scratch,
                    never,
                );
            }));
            assert!(result.is_err());
            assert_eq!(source.calls.get(), panic_at);
            assert_eq!(read_calls, panic_at - 2);
            assert_eq!(scratch, [0; 8]);
            assert!(service.is_fenced());
            assert_eq!(service.open_count(), 0);
            assert_eq!(service.last_sequence(), 1);
        }
    }
}
