use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=native-document-client.ld");
    let script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("native-document-client.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-native-document-client=-T{}",
        script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-native-document-client=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-native-document-client=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-native-document-client=max-page-size=0x1000");
}
