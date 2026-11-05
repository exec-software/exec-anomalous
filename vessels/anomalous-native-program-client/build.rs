use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=native-program-client.ld");
    let script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("native-program-client.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-native-program-client=-T{}",
        script.display()
    );
    // Bare, with no '-Wl,' prefix: rust-lld is invoked directly for
    // x86_64-unknown-none. See the note in AGENTS.md -- adding the prefix to
    // satisfy a host 'cargo build --workspace' is what breaks this.
    println!("cargo:rustc-link-arg-bin=anomalous-native-program-client=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-native-program-client=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-native-program-client=max-page-size=0x1000");
}
