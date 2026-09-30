use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let lock = manifest.join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    let text = std::fs::read_to_string(&lock).expect("read workspace Cargo.lock");
    let value: toml::Value = toml::from_str(&text).expect("parse workspace Cargo.lock");
    let versions = value
        .get("package")
        .and_then(toml::Value::as_array)
        .expect("Cargo.lock packages")
        .iter()
        .filter(|package| package.get("name").and_then(toml::Value::as_str) == Some("plumb"))
        .filter_map(|package| package.get("version").and_then(toml::Value::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(versions.len(), 1, "expected one resolved Plumb version");
    println!(
        "cargo:rustc-env=CONCORD_PLUMB_VERSION=v{}",
        versions.into_iter().next().expect("Plumb version")
    );
    let mut digest = Sha256::new();
    for name in ["gate.rs", "process.rs"] {
        let file = manifest.join("src/estate/work/delivery").join(name);
        println!("cargo:rerun-if-changed={}", file.display());
        let bytes = std::fs::read(&file).expect("read native gate implementation");
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    println!("cargo:rustc-env=CONCORD_WHARF_GATE={:x}", digest.finalize());
}
