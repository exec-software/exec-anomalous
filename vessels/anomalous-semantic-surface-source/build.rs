#![allow(non_camel_case_types, non_upper_case_globals)]

use std::env;
use std::fs;
use std::path::PathBuf;

use anomalous_desktop_transport::{
    surface_snapshot_v1, surface_snapshot_v1_byte_count, try_encode_fixed_protected_fixture,
};

const snapshot_file_name: &str = "semantic_surface_snapshot.bin";
const source_variant_environment: &str = "ANOMALOUS_SEMANTIC_SURFACE_SOURCE_VARIANT";
const snapshot_path_environment: &str = "ANOMALOUS_SEMANTIC_SURFACE_SNAPSHOT_PATH";

struct ephemeral_protected_text(Vec<u8>);

impl ephemeral_protected_text {
    fn derive(environment: &[u8], domain: u8, length: usize) -> Self {
        assert!(!environment.is_empty(), "protected fixture seed is empty");
        let mut bytes = Vec::with_capacity(length);
        for index in 0..length {
            let seed = environment[index % environment.len()];
            let value = seed.wrapping_add(domain).wrapping_add(index as u8) % 26;
            bytes.push(b'a' + value);
        }
        Self(bytes)
    }

    fn expose_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("protected fixture is valid UTF-8")
    }
}

impl Drop for ephemeral_protected_text {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

fn main() {
    println!("cargo:rerun-if-changed=source.ld");
    println!("cargo:rerun-if-env-changed={source_variant_environment}");
    println!("cargo:rerun-if-env-changed={snapshot_path_environment}");

    // Derive both protected inputs from runtime-only Cargo state so neither
    // plaintext is present in this build script's executable or generated
    // files. The unequal lengths and domains make the two inputs distinct;
    // `Drop` clears their heap storage before the build script exits.
    let output_directory = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is required"));
    let selected = if let Some(path) = env::var_os(snapshot_path_environment) {
        println!("cargo:rerun-if-changed={}", PathBuf::from(&path).display());
        let bytes = fs::read(PathBuf::from(path))
            .expect("read coordinator-provided semantic-surface snapshot");
        assert_eq!(
            bytes.len(),
            surface_snapshot_v1_byte_count,
            "coordinator snapshot has the wrong byte length"
        );
        surface_snapshot_v1::try_from_slice(&bytes)
            .unwrap_or_else(|error| panic!("coordinator snapshot rejected: {}", error.code()))
    } else {
        let environment = output_directory.as_os_str().as_encoded_bytes();
        let first_source = ephemeral_protected_text::derive(environment, 0x35, 29);
        let second_source = ephemeral_protected_text::derive(environment, 0xa7, 71);
        assert!(
            first_source.0 != second_source.0,
            "protected conformance inputs must differ"
        );

        let first = try_encode_fixed_protected_fixture(first_source.expose_str())
            .unwrap_or_else(|error| panic!("source snapshot conformance failed: {}", error.code()));
        let second = try_encode_fixed_protected_fixture(second_source.expose_str())
            .unwrap_or_else(|error| panic!("source snapshot conformance failed: {}", error.code()));
        assert_eq!(
            first.as_bytes(),
            second.as_bytes(),
            "protected source changed the redacted semantic-surface snapshot"
        );

        match env::var(source_variant_environment).as_deref() {
            Ok("second") => second,
            Ok("first") | Err(env::VarError::NotPresent) => first,
            Ok(_) | Err(env::VarError::NotUnicode(_)) => {
                panic!("semantic-surface source variant rejected")
            }
        }
    };
    fs::write(
        output_directory.join(snapshot_file_name),
        selected.as_bytes(),
    )
    .expect("semantic-surface snapshot write failed");

    let linker_script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir")).join("source.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-semantic-surface-source=-T{}",
        linker_script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-semantic-surface-source=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-semantic-surface-source=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-semantic-surface-source=max-page-size=0x1000");
}
