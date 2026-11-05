#![allow(non_upper_case_globals)]

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=renderer.ld");

    let linker_script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir")).join("renderer.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-semantic-surface-renderer=-T{}",
        linker_script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-semantic-surface-renderer=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-semantic-surface-renderer=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-semantic-surface-renderer=max-page-size=0x1000");
}
