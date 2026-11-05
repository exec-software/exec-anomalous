use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=native-client.ld");
    println!("cargo:rerun-if-changed=elf-client.ld");
    let script_name = if env::var_os("CARGO_FEATURE_ELF_DATA_TEST").is_some() {
        "elf-client.ld"
    } else {
        "native-client.ld"
    };
    let script = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join(script_name);
    println!(
        "cargo:rustc-link-arg-bin=anomalous-native-client=-T{}",
        script.display()
    );
    println!("cargo:rustc-link-arg-bin=anomalous-native-client=--build-id=none");
    println!("cargo:rustc-link-arg-bin=anomalous-native-client=-z");
    println!("cargo:rustc-link-arg-bin=anomalous-native-client=max-page-size=0x1000");
}
