#![allow(non_upper_case_globals)]

use std::env;
use std::fs;
use std::path::PathBuf;

use anomalous_desktop_renderer::scene_conformance::build_fixture_capsule;

const capsule_file_name: &str = "supervised_renderer_capsule.bin";
const source_variant_environment: &str = "ANOMALOUS_RENDERER_PROTECTED_SOURCE_VARIANT";

fn main() {
    println!("cargo:rerun-if-changed=renderer.ld");
    println!("cargo:rerun-if-env-changed={source_variant_environment}");

    let first_source = protected_source(7, 19);
    let second_source = protected_source(11, 23);
    let first = build_fixture_capsule(&first_source)
        .unwrap_or_else(|error| panic!("renderer capsule conformance failed: {}", error.code()));
    let second = build_fixture_capsule(&second_source)
        .unwrap_or_else(|error| panic!("renderer capsule conformance failed: {}", error.code()));
    assert!(
        first.as_bytes() == second.as_bytes(),
        "protected source changed public renderer capsule"
    );

    let output_directory = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is required"));
    let selected = match env::var(source_variant_environment).as_deref() {
        Ok("second") => &second,
        Ok("first") | Err(env::VarError::NotPresent) => &first,
        Ok(_) | Err(env::VarError::NotUnicode(_)) => {
            panic!("renderer protected-source variant rejected")
        }
    };
    fs::write(
        output_directory.join(capsule_file_name),
        selected.as_bytes(),
    )
    .expect("renderer capsule write failed");

    let linker_script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir")).join("renderer.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-supervised-renderer=-T{}",
        linker_script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-supervised-renderer=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-supervised-renderer=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-supervised-renderer=max-page-size=0x1000");
}

fn protected_source(multiplier: u32, offset: u32) -> String {
    (0..37)
        .map(|index| {
            let ordinal = (index * multiplier + offset) % 26;
            char::from_u32(u32::from(b'a') + ordinal).expect("lowercase ASCII scalar")
        })
        .collect()
}
