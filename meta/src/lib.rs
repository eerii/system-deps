use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
};

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand};
use serde_json::{Map, Value};

pub use serde_json::from_value;

pub type Values = Map<String, Value>;

// TODO: Cache results

/// Read an environment variable keeping track of its changes
fn env(name: &str) -> Option<String> {
    println!("cargo:rerun-if-env-changed={}", name);
    std::env::var(name).ok()
}

/// Get the directory where cargo is being called
/// This is **not** the directory where system-deps was cloned
fn root() -> PathBuf {
    if let Some(root) = env("SYSTEM_DEPS_ROOT") {
        return PathBuf::from(&root);
    }
    let mut dir = PathBuf::from(
        std::env::args()
            .next()
            .expect("There should be cargo arguments for determining the root"),
    );
    while !dir.ends_with("target") {
        if !dir.pop() {
            panic!("Error determining the cargo root, you may need to specify it manually with 'SYSTEM_DEPS_ROOT'");
        }
    }
    dir.pop();
    dir
}

/// Get the metadata from the root Cargo.toml
pub fn metadata() -> Metadata {
    MetadataCommand::new()
        .manifest_path(root().join("Cargo.toml"))
        .exec()
        .unwrap()
}

/// Recursively read dependency manifests to find system-deps metadata
pub fn read(metadata: &Metadata, key: &str) -> Map<String, Value> {
    let mut packages = if let Some(root) = metadata.root_package() {
        VecDeque::from([root])
    } else {
        metadata.workspace_packages().into()
    };
    let mut visited: HashSet<&str> = packages.iter().map(|p| p.name.as_str()).collect();
    let mut res = metadata
        .workspace_metadata
        .as_object()
        .and_then(|meta| meta.get(key))
        .and_then(|meta| meta.as_object())
        .cloned()
        .unwrap_or_default();

    while let Some(pkg) = packages.pop_front() {
        for dep in &pkg.dependencies {
            match dep.kind {
                DependencyKind::Normal | DependencyKind::Build => {}
                _ => continue,
            }
            if !visited.insert(&dep.name) {
                continue;
            }
            if let Some(dep_pkg) = metadata.packages.iter().find(|p| p.name == dep.name) {
                packages.push_back(dep_pkg);
            };
        }

        let Some(meta) = pkg
            .metadata
            .as_object()
            .and_then(|meta| meta.get(key))
            .and_then(|meta| meta.as_object())
        else {
            continue;
        };

        // Append the keys in this order to avoid overwriting the existing ones
        let mut meta = meta.clone();
        meta.append(&mut res);
        let _ = std::mem::replace(&mut res, meta);
    }

    res
}
