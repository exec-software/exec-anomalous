use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=native-durable-client.ld");
    let script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("native-durable-client.ld");
    println!(
        "cargo:rustc-link-arg-bin=anomalous-native-durable-client=-T{}",
        script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-native-durable-client=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-native-durable-client=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-native-durable-client=max-page-size=0x1000");
}
