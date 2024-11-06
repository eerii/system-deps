use std::{
    env,
    path::{Path, PathBuf},
};

const MANIFEST_VAR: &str = "SYSTEM_DEPS_MANIFEST";
const TARGET_VAR: &str = "SYSTEM_DEPS_TARGET_DIR";

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
fn manifest() -> PathBuf {
    println!("cargo:rerun-if-env-changed={}", MANIFEST_VAR);
    if let Ok(root) = env::var(MANIFEST_VAR) {
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
/// Directory where system-deps related build products will be stored
/// Notably, binary outputs are located here
fn target_dir() -> String {
    println!("cargo:rerun-if-env-changed={}", TARGET_VAR);
    env::var(TARGET_VAR).or(env::var("OUT_DIR")).unwrap()
}

pub fn main() {
    let manifest = manifest();
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-env=BUILD_MANIFEST={}", manifest.display());

    let target_dir = target_dir();
    println!("cargo:rerun-if-changed={}", target_dir);
    println!("cargo:rustc-env=BUILD_TARGET_DIR={}", target_dir);
}
