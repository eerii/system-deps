use std::{
    collections::{HashSet, VecDeque},
    sync::OnceLock,
};

use cargo_metadata::{DependencyKind, MetadataCommand};
use serde_json::{Map, Value};

pub use cargo_metadata::Metadata;
pub use serde_json::from_value;
pub type Values = Map<String, Value>;

/// Path to the top level Cargo.toml
pub const BUILD_MANIFEST: &str = env!("BUILD_MANIFEST");

/// Directory where system-deps related build products will be stored
pub const BUILD_TARGET_DIR: &str = env!("BUILD_TARGET_DIR");

/// Get the metadata from the root Cargo.toml
fn metadata() -> &'static Metadata {
    static CACHED: OnceLock<Metadata> = OnceLock::new();
    CACHED.get_or_init(|| {
        MetadataCommand::new()
            .manifest_path(BUILD_MANIFEST)
            .exec()
            .unwrap()
    })
}

/// Inserts values from b into a only if they don't already exist
fn merge(a: &mut Value, b: Value) {
    match (a, b) {
        (a @ &mut Value::Object(_), Value::Object(b)) => {
            let a = a.as_object_mut().unwrap();
            for (k, v) in b {
                if k.starts_with("cfg") {}

                if let Some(e) = a.get_mut(&k) {
                    if e.is_object() {
                        merge(e, v);
                    }
                } else {
                    a.insert(k, v);
                }
            }
        }
        (a, b) => *a = b,
    }
}

/// Recursively read dependency manifests to find system-deps metadata
pub fn read_metadata(key: &str) -> Values {
    let metadata = metadata();

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

        let Some(meta) = pkg.metadata.as_object().and_then(|meta| meta.get(key)) else {
            continue;
        };

        println!("cargo:rerun-if-changed={}", pkg.manifest_path);
        merge(&mut res, meta.clone());
    }

    res.as_object().cloned().unwrap_or_default()
}
