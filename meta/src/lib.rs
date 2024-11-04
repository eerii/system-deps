use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
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

/// Try to find the project root using locate-project
fn find_with_cargo(dir: &Path) -> Option<PathBuf> {
    let out = std::process::Command::new(env!("CARGO"))
        .current_dir(dir)
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .ok()?
        .stdout;
    if out.is_empty() {
        return None;
    }
    Some(PathBuf::from(std::str::from_utf8(&out).ok()?.trim()))
}

/// Try to find the project root finding the outmost Cargo.toml
fn find_by_path(mut dir: PathBuf) -> Option<PathBuf> {
    let mut best_match = None;
    loop {
        let Ok(read) = dir.read_dir() else {
            break;
        };
        for entry in read {
            let Ok(entry) = entry else { continue };
            if entry.file_name() == "Cargo.toml" {
                best_match = Some(entry.path());
            }
        }
        if !dir.pop() {
            break;
        }
    }
    best_match
}

/// Get the manifest from the project directory
/// This is **not** the directory where system-deps was cloned
/// If the target directory is not a subfolder of the project this will not work
pub fn root() -> PathBuf {
    if let Some(root) = env("SYSTEM_DEPS_MANIFEST") {
        return PathBuf::from(&root);
    }

    // This is a subdirectory of the target dir
    let mut dir = PathBuf::from(
        std::env::args()
            .next()
            .expect("There should be cargo arguments for determining the root"),
    );
    dir.pop();

    // Try to find the project first with cargo
    if let Some(dir) = find_with_cargo(&dir) {
        return dir;
    }

    // If it doesn't work, try to find a Cargo.toml
    find_by_path(dir).expect(
        "Error determining the cargo root manifest.\n\
                    Please set 'SYSTEM_DEPS_MANIFEST' to the path of your project's Cargo.toml",
    )
}

/// Get the metadata from the root Cargo.toml
pub fn metadata(manifest: &Path) -> Metadata {
    MetadataCommand::new()
        .manifest_path(manifest)
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

pub fn export_metadata(values: &Map<String, Value>) {
    let mut stack = values
        .iter()
        .map(|(k, v)| (k.clone(), v))
        .collect::<VecDeque<_>>();
    while let Some((key, value)) = stack.pop_front() {
        if let Some(value) = value.as_object() {
            stack.extend(
                value
                    .iter()
                    .filter(|(k, _)| *k != "name" && *k != "version")
                    .map(|(k, v)| (format!("{}_{}", key, k), v)),
            );
            continue;
        };
        let text = if let Some(value) = value.as_str() {
            value.into()
        } else if let Some(value) = value.as_array() {
            value
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",")
        } else if let Some(value) = value.as_number() {
            value.to_string()
        } else {
            continue;
        };
        // Metadata
        println!("cargo:{}={}", key.to_uppercase(), text);
        // Env vars
        println!("cargo:rustc-env={}={}", key.to_uppercase(), text);
    }
}
