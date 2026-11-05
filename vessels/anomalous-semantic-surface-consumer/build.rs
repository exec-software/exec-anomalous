#![allow(non_upper_case_globals)]

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=consumer.ld");

    let linker_script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir")).join("consumer.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-semantic-surface-consumer=-T{}",
        linker_script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-semantic-surface-consumer=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-semantic-surface-consumer=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-semantic-surface-consumer=max-page-size=0x1000");
}
