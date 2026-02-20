use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn workspace_root(manifest_dir: &Path) -> PathBuf {
    manifest_dir
        .join("../..")
        .canonicalize()
        .expect("failed to resolve workspace root")
}

fn main() {
    println!("cargo:rerun-if-changed=../voting-oracle/Cargo.toml");
    println!("cargo:rerun-if-changed=../voting-oracle/src");

    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required"));
    let ws_root = workspace_root(&manifest_dir);

    let out_dir = manifest_dir.join("wasm");
    fs::create_dir_all(&out_dir).expect("failed to create wasm output dir");
    let out_wasm = out_dir.join("voting_oracle.wasm");

    // Build `voting-oracle` into a separate target dir to avoid Cargo target-dir
    // lock contention / recursion when invoked from this build script.
    let nested_target_dir = ws_root.join("target/contract-import");

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .current_dir(&ws_root)
        .args([
            "build",
            "-p",
            "voting-oracle",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--target-dir",
            nested_target_dir
                .to_str()
                .expect("nested target dir must be utf-8"),
        ])
        .status()
        .expect("failed to spawn nested cargo build");

    if !status.success() {
        panic!("nested build failed for voting-oracle");
    }

    let built_wasm = nested_target_dir
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("voting_oracle.wasm");
    fs::copy(&built_wasm, &out_wasm).expect("failed to copy voting-oracle wasm for contractimport");
}
