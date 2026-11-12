#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Experimental userspace program-image admission, not execution or authority
//! creation. Admission recomputes every semantic claim from the same immutable
//! edition slice that the returned witness borrows. No NIL policy enters the
//! kernel. See ADR 0102 for the deliberately bounded derivation profile.

use sha2::{Digest, Sha256};

mod derive;
pub use derive::{derivation, derive};

pub const maximum_edition_bytes: usize = 64 * 1024;
pub const maximum_intent_bytes: usize = 512;
pub const image_header_bytes: usize = 64;
pub const manifest_header_bytes: usize = 96;
pub const maximum_image_bytes: usize =
    image_header_bytes + manifest_header_bytes + maximum_intent_bytes + maximum_edition_bytes;
pub const derivation_profile: u32 = 1;
const image_magic: &[u8; 8] = b"ANPROG\r\n";
const manifest_magic: &[u8; 8] = b"ANSMAN\r\n";
const version: u16 = 1;

/// Workload services understood by this derivation profile. These are distinct
/// from the bootstrap grant used by a loader to obtain its edition bytes.
pub mod authority {
    pub const console_write: u64 = 1;
    pub const known: u64 = console_write;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum error {
    edition,
    syntax,
    unsupported,
    intent,
    limit,
    image,
    encoding,
    version,
    profile,
    layout,
    identity,
    mismatch,
    ceiling,
    output,
}

impl error {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::edition => "manifest_edition",
            Self::syntax => "manifest_syntax",
            Self::unsupported => "manifest_unsupported",
            Self::intent => "manifest_intent",
            Self::limit => "manifest_limit",
            Self::image => "manifest_image",
            Self::encoding => "manifest_encoding",
            Self::version => "manifest_version",
            Self::profile => "manifest_profile",
            Self::layout => "manifest_layout",
            Self::identity => "manifest_identity",
            Self::mismatch => "manifest_mismatch",
            Self::ceiling => "manifest_ceiling",
            Self::output => "manifest_output",
        }
    }
}

/// Content identity only: this neither authenticates a publisher nor validates
/// an annotation's truth. The pinned software SHA-256 implementation needs no
/// allocator, operating-system service or CPU-specific hash instruction.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub fn encoded_length(edition: &[u8]) -> Result<usize, error> {
    let derived = derive(edition)?;
    Ok(image_header_bytes + manifest_header_bytes + derived.intent().len() + edition.len())
}

/// Derive and encode a canonical image. No caller-supplied surface, intent or
/// digest can be substituted. All fallible work precedes the first output write;
/// a short destination or invalid edition leaves the whole destination intact.
pub fn encode(edition: &[u8], output: &mut [u8]) -> Result<usize, error> {
    let derived = derive(edition)?;
    let intent = derived.intent().as_bytes();
    let manifest_length = manifest_header_bytes + intent.len();
    let edition_offset = image_header_bytes + manifest_length;
    let length = edition_offset + edition.len();
    if output.len() < length {
        return Err(error::output);
    }
    let identity = sha256(edition);
    let image = &mut output[..length];
    image.fill(0);
    image[..8].copy_from_slice(image_magic);
    image[8..10].copy_from_slice(&version.to_le_bytes());
    image[10..12].copy_from_slice(&(image_header_bytes as u16).to_le_bytes());
    image[12..16].copy_from_slice(&derivation_profile.to_le_bytes());
    put_u64(image, 16, image_header_bytes as u64);
    put_u64(image, 24, manifest_length as u64);
    put_u64(image, 32, edition_offset as u64);
    put_u64(image, 40, edition.len() as u64);
    put_u64(image, 48, length as u64);
    let manifest = &mut image[image_header_bytes..edition_offset];
    manifest[..8].copy_from_slice(manifest_magic);
    manifest[8..10].copy_from_slice(&version.to_le_bytes());
    manifest[10..12].copy_from_slice(&(manifest_header_bytes as u16).to_le_bytes());
    manifest[12..16].copy_from_slice(&derivation_profile.to_le_bytes());
    put_u64(manifest, 16, derived.authority());
    put_u64(manifest, 24, edition.len() as u64);
    manifest[32..64].copy_from_slice(&identity);
    manifest[64..68].copy_from_slice(&(intent.len() as u32).to_le_bytes());
    manifest[manifest_header_bytes..].copy_from_slice(intent);
    image[edition_offset..].copy_from_slice(edition);
    Ok(length)
}

/// A structural and workload-policy admission witness. Construction is private,
/// and its immutable edition borrow ties all validated claims to the exact held
/// image. It is not a kernel grant, signature, execution plan or interpreter.
#[derive(Debug)]
pub struct admitted_edition<'a> {
    edition: &'a [u8],
    intent: &'a str,
    identity: [u8; 32],
    required_services: u64,
    granted_services: u64,
}

impl<'a> admitted_edition<'a> {
    pub const fn edition(&self) -> &'a [u8] {
        self.edition
    }

    pub const fn intent(&self) -> &'a str {
        self.intent
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub const fn required_services(&self) -> u64 {
        self.required_services
    }

    /// The intersection is returned only after refusing any request outside
    /// the ceiling. A denied requirement is never silently dropped.
    pub const fn granted_services(&self) -> u64 {
        self.granted_services
    }
}

/// Independently inspect the complete image, rehash and rederive its edition,
/// and refuse claims or authority that do not match. `ceiling` is explicit
/// trusted caller policy; nothing in this image may enlarge it.
pub fn admit(image: &[u8], ceiling: u64) -> Result<admitted_edition<'_>, error> {
    if ceiling & !authority::known != 0 {
        return Err(error::ceiling);
    }
    if image.len() < image_header_bytes + manifest_header_bytes || image.len() > maximum_image_bytes
    {
        return Err(error::image);
    }
    if &image[..8] != image_magic || read_u16(image, 10) != image_header_bytes as u16 {
        return Err(error::encoding);
    }
    if read_u16(image, 8) != version {
        return Err(error::version);
    }
    if read_u32(image, 12) != derivation_profile {
        return Err(error::profile);
    }
    if image[56..64].iter().any(|byte| *byte != 0) {
        return Err(error::encoding);
    }
    let manifest_length = read_u64(image, 24);
    let edition_offset = read_u64(image, 32);
    let edition_length = read_u64(image, 40);
    if read_u64(image, 16) != image_header_bytes as u64
        || !(manifest_header_bytes as u64 + 1
            ..=manifest_header_bytes as u64 + maximum_intent_bytes as u64)
            .contains(&manifest_length)
        || edition_offset != image_header_bytes as u64 + manifest_length
        || edition_length == 0
        || edition_length > maximum_edition_bytes as u64
        || edition_offset.checked_add(edition_length) != Some(image.len() as u64)
        || read_u64(image, 48) != image.len() as u64
    {
        return Err(error::layout);
    }
    // The comparisons above bound both extents within this held slice before
    // conversion to usize, independently of the width of the host pointer.
    let manifest = &image[image_header_bytes..edition_offset as usize];
    let edition = &image[edition_offset as usize..];
    if &manifest[..8] != manifest_magic
        || read_u16(manifest, 10) != manifest_header_bytes as u16
        || manifest[68..96].iter().any(|byte| *byte != 0)
    {
        return Err(error::encoding);
    }
    if read_u16(manifest, 8) != version {
        return Err(error::version);
    }
    if read_u32(manifest, 12) != derivation_profile {
        return Err(error::profile);
    }
    if read_u64(manifest, 24) != edition_length
        || read_u32(manifest, 64) as u64 != manifest_length - manifest_header_bytes as u64
    {
        return Err(error::layout);
    }
    let required_services = read_u64(manifest, 16);
    if required_services & !authority::known != 0 {
        return Err(error::unsupported);
    }
    let intent =
        core::str::from_utf8(&manifest[manifest_header_bytes..]).map_err(|_| error::intent)?;
    if intent.chars().any(char::is_control) {
        return Err(error::intent);
    }
    let identity = sha256(edition);
    if identity.as_slice() != &manifest[32..64] {
        return Err(error::identity);
    }
    let derived = derive(edition)?;
    if required_services != derived.authority() || intent != derived.intent() {
        return Err(error::mismatch);
    }
    if required_services & !ceiling != 0 {
        return Err(error::ceiling);
    }
    Ok(admitted_edition {
        edition,
        intent,
        identity,
        required_services,
        granted_services: required_services & ceiling,
    })
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::{format, string::String, vec, vec::Vec};

    const arithmetic: &[u8] = include_bytes!("../fixtures/arithmetic.nil");
    const console: &[u8] = include_bytes!("../fixtures/console.nil");

    fn image(edition: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; encoded_length(edition).unwrap()];
        assert_eq!(encode(edition, &mut bytes), Ok(bytes.len()));
        bytes
    }

    fn hexadecimal(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn standard_sha256_vectors_include_multiple_compression_blocks() {
        for (input, expected) in [
            (
                &b""[..],
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                &b"abc"[..],
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                &b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"[..],
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
        ] {
            assert_eq!(hexadecimal(&sha256(input)), expected);
        }
        assert_eq!(
            hexadecimal(&sha256(&vec![b'a'; 1_000_000])),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn round_trip_borrows_exact_edition_and_never_adds_ceiling_authority() {
        for (edition, required) in [(arithmetic, 0), (console, authority::console_write)] {
            let first = image(edition);
            assert_eq!(first, image(edition));
            let admitted = admit(&first, authority::known).unwrap();
            assert_eq!(admitted.edition(), edition);
            assert_eq!(admitted.required_services(), required);
            assert_eq!(admitted.granted_services(), required);
            assert_eq!(admitted.identity(), sha256(edition));
            assert_eq!(admitted.intent(), derive(edition).unwrap().intent());
            let offset = read_u64(&first, 32) as usize;
            assert_eq!(admitted.edition().as_ptr(), first[offset..].as_ptr());
        }
        assert!(admit(&image(arithmetic), 0).is_ok());
        assert_eq!(admit(&image(console), 0).unwrap_err(), error::ceiling);
        for ceiling in [2, 4, 8, u64::MAX] {
            assert_eq!(
                admit(&image(arithmetic), ceiling).unwrap_err(),
                error::ceiling
            );
        }
    }

    #[test]
    fn encoding_failures_leave_output_unchanged_and_success_preserves_tail() {
        let length = encoded_length(console).unwrap();
        let mut short = vec![0xa5; length - 1];
        assert_eq!(encode(console, &mut short), Err(error::output));
        assert!(short.iter().all(|byte| *byte == 0xa5));
        let mut bytes = vec![0xa5; length + 17];
        assert!(encode(b"not a NIL edition", &mut bytes).is_err());
        assert!(bytes.iter().all(|byte| *byte == 0xa5));
        assert_eq!(encode(console, &mut bytes), Ok(length));
        assert!(bytes[length..].iter().all(|byte| *byte == 0xa5));
        assert!(admit(&bytes[..length], authority::known).is_ok());
    }

    #[test]
    fn every_single_byte_mutation_and_truncation_is_refused() {
        let original = image(console);
        for length in 0..original.len() {
            assert!(
                admit(&original[..length], authority::known).is_err(),
                "{length}"
            );
        }
        for index in 0..original.len() {
            let mut mutated = original.clone();
            mutated[index] ^= 1;
            assert!(admit(&mutated, authority::known).is_err(), "byte {index}");
        }
        let mut trailing = original;
        trailing.push(0);
        assert_eq!(
            admit(&trailing, authority::known).unwrap_err(),
            error::layout
        );
    }

    #[test]
    fn offsets_lengths_and_reserved_fields_never_select_alternate_slices() {
        let original = image(console);
        for offset in [16, 24, 32, 40, 48, 64 + 24] {
            for value in [0, 1, 63, 64, 96, u32::MAX as u64, u64::MAX] {
                if read_u64(&original, offset) == value {
                    continue;
                }
                let mut mutated = original.clone();
                put_u64(&mut mutated, offset, value);
                assert!(
                    admit(&mutated, authority::known).is_err(),
                    "{offset}/{value}"
                );
            }
        }
        for index in (56..64).chain(64 + 68..64 + 96) {
            let mut mutated = original.clone();
            mutated[index] = 0xff;
            assert_eq!(
                admit(&mutated, authority::known).unwrap_err(),
                error::encoding
            );
        }
        for (index, expected) in [
            (8, error::version),
            (12, error::profile),
            (72, error::version),
            (76, error::profile),
        ] {
            let mut mutated = original.clone();
            mutated[index] = 2;
            assert_eq!(admit(&mutated, authority::known).unwrap_err(), expected);
        }
    }

    #[test]
    fn a_matching_digest_cannot_substitute_for_semantic_derivation() {
        let mut understated = image(console);
        put_u64(&mut understated, image_header_bytes + 16, 0);
        assert_eq!(
            admit(&understated, authority::known).unwrap_err(),
            error::mismatch
        );
        let mut overstated = image(arithmetic);
        put_u64(
            &mut overstated,
            image_header_bytes + 16,
            authority::console_write,
        );
        assert_eq!(
            admit(&overstated, authority::known).unwrap_err(),
            error::mismatch
        );
        let mut forged = image(console);
        forged[image_header_bytes + manifest_header_bytes] ^= 0x20;
        assert_eq!(
            admit(&forged, authority::known).unwrap_err(),
            error::mismatch
        );

        // Alter actual intent in the edition and recompute its real digest.
        // The old manifest intent must still be independently refused.
        let mut rehashed = image(console);
        let offset = read_u64(&rehashed, 32) as usize;
        let prefix = b"@ai_context(\"";
        let position = rehashed[offset..]
            .windows(prefix.len())
            .position(|bytes| bytes == prefix)
            .unwrap();
        rehashed[offset + position + prefix.len()] ^= 0x20;
        let digest = sha256(&rehashed[offset..]);
        rehashed[96..128].copy_from_slice(&digest);
        assert_eq!(
            admit(&rehashed, authority::known).unwrap_err(),
            error::mismatch
        );
    }

    #[test]
    fn oversized_and_arbitrary_images_are_bounded_and_refused_without_panics() {
        assert_eq!(
            admit(&vec![0; maximum_image_bytes + 1], 0).unwrap_err(),
            error::image
        );
        assert!(encoded_length(&vec![b' '; maximum_edition_bytes + 1]).is_err());
        let mut seed = 0x1c56_103du32;
        for length in 0..1024 {
            let mut bytes = vec![0; length];
            for byte in &mut bytes {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *byte = seed as u8;
            }
            assert!(admit(&bytes, authority::known).is_err());
        }
    }
}
