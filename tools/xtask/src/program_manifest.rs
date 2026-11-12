//! Host-only packaging and policy inspection of a bounded NIL edition.
//! A receipt describes the captured bytes; it grants no live kernel authority
//! and neither command executes the edition or invokes a language toolchain.

use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anomalous_program_manifest::{
    admit, authority, encode, encoded_length, maximum_edition_bytes, maximum_image_bytes, sha256,
};

static next_temporary: AtomicU64 = AtomicU64::new(0);

pub(super) fn pack(input: &Path, output: &Path) -> Result<(), String> {
    let receipt = pack_report(input, output)?;
    write_report(&receipt)
        .map_err(|error| format!("program output published and retained; {error}"))
}

pub(super) fn inspect(input: &Path, ceiling: &str) -> Result<(), String> {
    let ceiling = parse_ceiling(ceiling)?;
    let image = read_bounded(input, maximum_image_bytes)?;
    write_report(&report(&image, ceiling)?)
}

fn parse_ceiling(value: &str) -> Result<u64, String> {
    match value {
        "none" => Ok(0),
        "console_write" => Ok(authority::console_write),
        _ => Err("program manifest ceiling must be exactly none or console_write".into()),
    }
}

fn pack_report(input: &Path, output: &Path) -> Result<String, String> {
    // Every derivation, hash, encoding and comparison uses this one snapshot.
    // A filesystem read is not a claim of an atomic historical file version.
    let edition = read_bounded(input, maximum_edition_bytes)?;
    let image = image_from_edition(&edition)?;
    let receipt = package_report(&image)?;
    publish_new(output, &image)?;
    // Detect a changed publication without deleting a path another writer may
    // now own. Only the final bytes, not the input pathname, are reread here.
    let published = read_bounded(output, maximum_image_bytes).map_err(|error| {
        format!("program output published and retained; verification read failed: {error}")
    })?;
    if published != image {
        return Err("published program image changed; output retained for inspection".into());
    }
    Ok(receipt)
}

fn image_from_edition(edition: &[u8]) -> Result<Vec<u8>, String> {
    let length = encoded_length(edition)
        .map_err(|error| format!("program edition rejected: {}", error.as_str()))?;
    if length > maximum_image_bytes {
        return Err("encoded program image exceeds the host bound".into());
    }
    let mut image = vec![0; length];
    let written = encode(edition, &mut image)
        .map_err(|error| format!("program encoding rejected: {}", error.as_str()))?;
    if written != length {
        return Err("program encoding length disagrees with its allocation".into());
    }
    let admitted = admit(&image, authority::known)
        .map_err(|error| format!("encoded program failed readback: {}", error.as_str()))?;
    if admitted.edition() != edition {
        return Err("program encoding changed the captured edition".into());
    }
    Ok(image)
}

// This is a byte bound, not an I/O deadline or a hostile-filesystem boundary.
// The caller trusts the filesystem and its namespace during the operation: a
// malicious regular-file-to-FIFO swap before File::open could otherwise block.
fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    // Reject obvious non-file inputs before open, then check the opened object
    // again. No path is reopened between the metadata check and bounded read.
    if !fs::metadata(path)
        .map_err(|error| format!("inspect {}: {error}", path.display()))?
        .is_file()
    {
        return Err(format!("input is not a regular file: {}", path.display()));
    }
    let file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("inspect opened {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(format!(
            "input exceeds its regular-file bound: {}",
            path.display()
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    if bytes.len() > maximum {
        return Err(format!(
            "input grew beyond its byte bound: {}",
            path.display()
        ));
    }
    Ok(bytes)
}

/// The temporary file and destination share a filesystem. A hard link exposes
/// only the complete synced inode and atomically refuses an existing name,
/// including a symlink. Plain rename would replace an existing output on Unix.
/// This assumes the parent directory is trusted against malicious entry
/// replacement. It is atomic no-replace publication between cooperating writers,
/// not protection against an attacker replacing the temporary pathname.
/// Directory fsync/power-loss durability are outside this publication claim.
fn publish_new(output: &Path, bytes: &[u8]) -> Result<(), String> {
    let name = output
        .file_name()
        .ok_or("program output needs a file name")?;
    let parent = output
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent =
        fs::canonicalize(parent).map_err(|error| format!("resolve output directory: {error}"))?;
    let destination = parent.join(name);
    let (temporary, mut file) = new_temporary(&parent)?;
    let result = (|| {
        file.write_all(bytes)
            .map_err(|error| format!("write program image: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync program image: {error}"))?;
        // The hard link is the publication point. A failed link never changes
        // an existing destination and leaves only our temporary to clean up.
        fs::hard_link(&temporary, &destination)
            .map_err(|error| format!("publish new {}: {error}", output.display()))
    })();
    drop(file);
    let cleanup = fs::remove_file(&temporary)
        .map_err(|error| format!("remove temporary program image: {error}"));
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(format!("program output published and retained; {error}")),
        (Err(error), Err(cleanup)) => Err(format!("{error}; {cleanup}")),
    }
}

fn new_temporary(parent: &Path) -> Result<(PathBuf, File), String> {
    for _ in 0..32 {
        let serial = next_temporary.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".anomalous-program-manifest-{}-{serial}.tmp",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("create temporary program image: {error}")),
        }
    }
    Err("program temporary-name budget exhausted".into())
}

fn report(image: &[u8], ceiling: u64) -> Result<String, String> {
    inspection_report(image, Some(ceiling))
}

fn package_report(image: &[u8]) -> Result<String, String> {
    inspection_report(image, None)
}

fn inspection_report(image: &[u8], policy: Option<u64>) -> Result<String, String> {
    // Packaging uses the known surface only to rederive encoding consistency.
    // It has no caller policy and must not report a ceiling or granted services.
    let admitted = admit(image, policy.unwrap_or(authority::known))
        .map_err(|error| format!("program image refused: {}", error.as_str()))?;
    let surface = if admitted.required_services() == 0 {
        "[]"
    } else if admitted.required_services() == authority::console_write {
        "[\"console_write\"]"
    } else {
        return Err("program admission returned an unknown service surface".into());
    };
    let (status, ceiling_field, granted_field) = match policy {
        Some(ceiling) => (
            "host_policy_admitted",
            format!("\"ceiling\":{ceiling},"),
            format!("\"granted_services\":{},", admitted.granted_services()),
        ),
        None => ("host_package_validated", String::new(), String::new()),
    };
    Ok(format!(
        "{{\"schema\":1,\"profile\":1,\"status\":\"{status}\",\"executed\":false,{ceiling_field}\"required_services\":{},{granted_field}\"surface\":{surface},\"intent\":{},\"edition_bytes\":{},\"edition_sha256\":\"{}\",\"image_bytes\":{},\"image_sha256\":\"{}\"}}",
        admitted.required_services(),
        quoted(admitted.intent()),
        admitted.edition().len(),
        hex(&admitted.identity()),
        image.len(),
        hex(&sha256(image)),
    ))
}

fn hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(result, "{byte:02x}").expect("writing to a String cannot fail");
    }
    result
}

fn quoted(value: &str) -> String {
    let mut result = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\u{0}'..='\u{1f}' => {
                write!(result, "\\u{:04x}", character as u32)
                    .expect("writing to a String cannot fail");
            }
            _ => result.push(character),
        }
    }
    result.push('"');
    result
}

fn write_report(value: &str) -> Result<(), String> {
    let mut output = std::io::stdout().lock();
    output
        .write_all(value.as_bytes())
        .and_then(|()| output.write_all(b"\n"))
        .map_err(|error| format!("write program inspection: {error}"))
}

const arithmetic_fixture: &[u8] = b"# NIL Textual Module\nmodule root\n\n@ai_context(\"Return zero without services\")\n# declaration_line 1\nfunction main -> i32:\n    PUSH_CONST 0 ; line 2\n    CAST i32 ; line 2\n    RET ; line 2\nend function\n";
const console_fixture: &[u8] = b"# NIL Textual Module\nmodule root\n\n# String constants\nconst S1 = \"hello\"\n\n@ai_context(\"Write hello then return zero\")\n# declaration_line 1\nfunction main -> i32:\n    PUSH_CONST_STR S1\n    CALL println 1\n    PUSH_CONST 0\n    CAST i32\n    RET\nend function\n";

/// Generated and retained fixture proofs, independent of the optional SDK and
/// of boot/image building. Filesystem publication is covered by host unit tests.
pub(super) fn proof(root: &Path, _build: &Path) -> Result<(), String> {
    let mutations = fixture_proof()?;
    retained_fixture_proof(root)?;
    write_report(&format!(
        "{{\"schema\":1,\"profile\":1,\"status\":\"host_manifest_proof_passed\",\"executed\":false,\"fixtures\":4,\"rejected_compiler_editions\":2,\"rejected_mutations\":{mutations}}}"
    ))
}

fn retained_fixture_proof(root: &Path) -> Result<(), String> {
    let directory = root.join("crates/anomalous-program-manifest/fixtures");
    for (name, expected_hash, services) in [
        (
            "arithmetic.nil",
            "71de5fee678f2721f6162dec8e0acae6d2978f9e7b6238557b29cade4d32e11a",
            Some(0),
        ),
        (
            "console.nil",
            "05486be07bb08311bc25f26f8566bf1d1d6e932b6d70cfef0d5b5525f40b45dd",
            Some(1),
        ),
        (
            "compiler-arithmetic.nil",
            "ba2e98aa1979bfd6ddecb620a320f3b7dc0097801f2b46f1fd9cae9f0409f79f",
            None,
        ),
        (
            "compiler-console.nil",
            "6df7742ffb25c7f89d3ac6ce432cc0c11eb8aac6f76d5252858cbd15c160e876",
            None,
        ),
    ] {
        let edition = read_bounded(&directory.join(name), maximum_edition_bytes)?;
        // Independent sha256sum pins preserve complete fixture provenance.
        if hex(&sha256(&edition)) != expected_hash {
            return Err(format!("retained fixture identity changed: {name}"));
        }
        if let Some(services) = services {
            let image = image_from_edition(&edition)?;
            let admitted = admit(&image, services)
                .map_err(|error| format!("retained fixture {name}: {}", error.as_str()))?;
            if admitted.required_services() != services || admitted.granted_services() != services {
                return Err(format!("retained fixture authority changed: {name}"));
            }
        } else if encoded_length(&edition).is_ok() {
            // Complete compiler outputs retain nilaix registration, which this
            // profile cannot admit. No helper is stripped or ignored here.
            return Err(format!(
                "unsupported complete compiler output admitted: {name}"
            ));
        }
    }
    Ok(())
}

fn fixture_proof() -> Result<usize, String> {
    let mut rejected = 0;
    // SHA-256 and wire-size pins were independently calculated with Python's
    // hashlib and struct encoders, not this crate's hash/encoding functions.
    for (edition, services, edition_hash, image_length, image_hash) in [
        (
            arithmetic_fixture,
            0,
            "9500ff98b49b0ee904413bc2c53cbdafb4d44679cee477f23a64c52159f04ccc",
            387,
            "12583f8667d4da880f567a40f4fc7ef626296568a8cdae0a9af02bae2c9735c7",
        ),
        (
            console_fixture,
            1,
            "fe41a275ae587accf56cf54a6de6c547f0d978f0dd513c5465b6672a9f0d82b3",
            440,
            "0f6a44526e13b48760e81e38163d4be007c8c7d51f24019f4a8e7de163c1de39",
        ),
    ] {
        let image = image_from_edition(edition)?;
        let admitted = admit(&image, services)
            .map_err(|error| format!("fixture rejected: {}", error.as_str()))?;
        if admitted.required_services() != services || admitted.granted_services() != services {
            return Err("fixture authority differs from its independent expectation".into());
        }
        if hex(&admitted.identity()) != edition_hash
            || image.len() != image_length
            || hex(&sha256(&image)) != image_hash
        {
            return Err("fixture differs from independent identity/encoding pins".into());
        }
        for index in 0..image.len() {
            let mut changed = image.clone();
            changed[index] ^= 1;
            if admit(&changed, authority::known).is_ok() {
                return Err(format!("changed program byte {index} was admitted"));
            }
            rejected += 1;
        }
        for length in 0..image.len() {
            if admit(&image[..length], authority::known).is_ok() {
                return Err(format!("truncated program length {length} was admitted"));
            }
            rejected += 1;
        }
        let mut extended = image.clone();
        extended.push(0);
        if admit(&extended, authority::known).is_ok() {
            return Err("trailing program byte was admitted".into());
        }
        rejected += 1;
        if services != 0 {
            if admit(&image, 0).is_ok() {
                return Err("console fixture escaped the empty ceiling".into());
            }
            rejected += 1;
        }
    }
    // Change a syntactically valid edition and refresh its identity, keeping
    // the old claims. Both refusals must reach semantic rederivation, rather
    // than merely fail a stale content hash or malformed framing check.
    for (edition, from, to) in [
        (
            arithmetic_fixture,
            b"Return zero".as_slice(),
            b"return zero".as_slice(),
        ),
        (
            console_fixture,
            b"CALL println 1".as_slice(),
            b"POP           ".as_slice(),
        ),
    ] {
        let mut changed = image_from_edition(edition)?;
        // ADR 0102 v1 offsets, stated independently of encoder constants.
        let offset = u64::from_le_bytes(changed[32..40].try_into().unwrap()) as usize;
        let matches = changed[offset..]
            .windows(from.len())
            .enumerate()
            .filter_map(|(index, bytes)| (bytes == from).then_some(index))
            .collect::<Vec<_>>();
        if matches.len() != 1 || from.len() != to.len() {
            return Err("semantic mutation fixture is ambiguous".into());
        }
        let position = offset + matches[0];
        changed[position..position + to.len()].copy_from_slice(to);
        let digest = sha256(&changed[offset..]);
        changed[96..128].copy_from_slice(&digest);
        match admit(&changed, authority::known) {
            Err(error) if error.as_str() == "manifest_mismatch" => rejected += 1,
            _ => return Err("rehashed false manifest was not refused by rederivation".into()),
        }
    }
    Ok(rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct test_directory(PathBuf);
    impl test_directory {
        fn new() -> Self {
            let serial = next_temporary.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "anomalous-manifest-test-{}-{serial}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
        fn entries(&self) -> usize {
            fs::read_dir(&self.0).unwrap().count()
        }
    }
    impl Drop for test_directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn fixture_authority_and_all_byte_mutations_are_checked() {
        assert_eq!(fixture_proof().unwrap(), 1659);
    }

    #[test]
    fn retained_hand_authored_editions_pass_and_complete_compiler_outputs_fail() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        retained_fixture_proof(&root).unwrap();
    }

    #[test]
    fn ceiling_is_explicit_and_has_no_numeric_or_prefix_fallback() {
        assert_eq!(parse_ceiling("none").unwrap(), 0);
        assert_eq!(parse_ceiling("console_write").unwrap(), 1);
        for value in [
            "",
            "0",
            "1",
            "all",
            "known",
            "None",
            "console",
            " console_write",
            "none ",
        ] {
            assert!(parse_ceiling(value).is_err(), "accepted {value:?}");
        }
        let console = image_from_edition(console_fixture).unwrap();
        assert!(report(&console, 0).is_err());
        assert!(report(&console, 2).is_err());
    }

    #[test]
    fn receipts_are_deterministic_and_contain_only_artifact_and_policy_facts() {
        let image = image_from_edition(arithmetic_fixture).unwrap();
        let actual = report(&image, 0).unwrap();
        assert_eq!(actual, report(&image, 0).unwrap());
        assert_eq!(image, image_from_edition(arithmetic_fixture).unwrap());
        let expected = format!(
            "{{\"schema\":1,\"profile\":1,\"status\":\"host_policy_admitted\",\"executed\":false,\"ceiling\":0,\"required_services\":0,\"granted_services\":0,\"surface\":[],\"intent\":\"Return zero without services\",\"edition_bytes\":{},\"edition_sha256\":\"{}\",\"image_bytes\":{},\"image_sha256\":\"{}\"}}",
            arithmetic_fixture.len(),
            hex(&sha256(arithmetic_fixture)),
            image.len(),
            hex(&sha256(&image))
        );
        assert_eq!(actual, expected);
        let package = package_report(&image).unwrap();
        let expected_package = expected
            .replace("host_policy_admitted", "host_package_validated")
            .replace("\"ceiling\":0,", "")
            .replace("\"granted_services\":0,", "");
        assert_eq!(package, expected_package);
        assert!(!package.contains("\"ceiling\""));
        assert!(!package.contains("\"granted_services\""));
        assert!(!actual.contains('\n'));
        assert_eq!(
            quoted("quote\" slash\\ π\n\r\t\0"),
            "\"quote\\\" slash\\\\ π\\u000a\\u000d\\u0009\\u0000\""
        );
    }

    #[test]
    fn bounded_snapshot_refuses_oversized_and_non_file_inputs() {
        let directory = test_directory::new();
        let input = directory.path("input.nil");
        fs::write(&input, b"abc").unwrap();
        assert_eq!(read_bounded(&input, 3).unwrap(), b"abc");
        assert!(read_bounded(&input, 2).is_err());
        assert!(read_bounded(&directory.0, 100).is_err());
        let snapshot = read_bounded(&input, 3).unwrap();
        fs::write(&input, b"xyz").unwrap();
        assert_eq!(snapshot, b"abc");
    }

    #[test]
    fn publication_preserves_existing_files_and_cleans_its_temporary() {
        let directory = test_directory::new();
        let output = directory.path("program.re");
        fs::write(&output, b"previous output").unwrap();
        assert!(publish_new(&output, b"replacement").is_err());
        assert_eq!(fs::read(&output).unwrap(), b"previous output");
        assert_eq!(directory.entries(), 1);
        let blocked = directory.path("directory.re");
        fs::create_dir(&blocked).unwrap();
        assert!(publish_new(&blocked, b"replacement").is_err());
        assert!(blocked.is_dir());
        assert_eq!(directory.entries(), 2);
        let fresh = directory.path("fresh.re");
        publish_new(&fresh, b"complete bytes").unwrap();
        assert_eq!(fs::read(&fresh).unwrap(), b"complete bytes");
        assert_eq!(directory.entries(), 3);
    }

    #[cfg(unix)]
    #[test]
    fn publication_never_follows_or_replaces_an_output_symlink() {
        use std::os::unix::fs::symlink;
        let directory = test_directory::new();
        let target = directory.path("target");
        fs::write(&target, b"keep").unwrap();
        let output = directory.path("program.re");
        symlink(&target, &output).unwrap();
        assert!(publish_new(&output, b"replace").is_err());
        assert_eq!(fs::read(&target).unwrap(), b"keep");
        assert!(
            fs::symlink_metadata(&output)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(directory.entries(), 2);
    }

    #[test]
    fn concurrent_publishers_have_one_complete_winner() {
        let directory = test_directory::new();
        let output = directory.path("program.re");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let writers = [vec![0x31; 8195], vec![0x72; 8195]];
        let mut threads = Vec::new();
        for bytes in writers.clone() {
            let output = output.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                publish_new(&output, &bytes)
            }));
        }
        let successes = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .filter(Result::is_ok)
            .count();
        assert_eq!(successes, 1);
        assert!(writers.contains(&fs::read(output).unwrap()));
        assert_eq!(directory.entries(), 1);
    }

    #[test]
    fn pack_readback_matches_structural_receipt_and_invalid_source_cannot_publish() {
        let directory = test_directory::new();
        let input = directory.path("source.nil");
        let output = directory.path("program.re");
        fs::write(&input, console_fixture).unwrap();
        let receipt = pack_report(&input, &output).unwrap();
        let image = read_bounded(&output, maximum_image_bytes).unwrap();
        assert_eq!(receipt, package_report(&image).unwrap());
        assert!(receipt.contains("\"status\":\"host_package_validated\""));
        assert!(!receipt.contains("\"ceiling\""));
        assert!(!receipt.contains("\"granted_services\""));
        assert!(report(&image, 0).is_err());
        let inspected = report(&image, 1).unwrap();
        assert!(inspected.contains("\"status\":\"host_policy_admitted\""));
        assert!(inspected.contains("\"ceiling\":1"));
        assert!(inspected.contains("\"granted_services\":1"));
        assert!(receipt.contains("\"surface\":[\"console_write\"]"));
        assert!(pack_report(&input, &output).is_err());
        assert_eq!(fs::read(&output).unwrap(), image);
        fs::write(&input, b"not NIL").unwrap();
        let rejected = directory.path("rejected.re");
        assert!(pack_report(&input, &rejected).is_err());
        assert!(!rejected.exists());
        assert_eq!(directory.entries(), 2);
    }
}
