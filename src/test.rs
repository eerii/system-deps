use pkg_config;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{probe_full, BuildFlags, EnvVariables, ErrorKind, Result};

lazy_static! {
    static ref LOCK: Mutex<()> = Mutex::new(());
}

fn toml(
    path: &str,
    env: Vec<(&'static str, &'static str)>,
) -> Result<(
    std::collections::HashMap<String, pkg_config::Library>,
    BuildFlags,
)> {
    {
        // PKG_CONFIG_PATH is read by pkg-config so we need to actually change the env
        let _l = LOCK.lock();
        env::set_var(
            "PKG_CONFIG_PATH",
            &env::current_dir().unwrap().join("src").join("tests"),
        );
    }

    let mut hash = HashMap::new();
    hash.insert(
        "CARGO_MANIFEST_DIR",
        env::current_dir()
            .unwrap()
            .join("src")
            .join("tests")
            .join(path)
            .to_string_lossy()
            .to_string(),
    );

    hash.insert("CARGO_FEATURE_TEST_FEATURE", "".to_string());
    env.iter().for_each(|(k, v)| {
        hash.insert(k, v.to_string());
    });

    probe_full(EnvVariables::Mock(hash))
}

#[test]
fn good() {
    let (libraries, flags) = toml("toml-good", vec![]).unwrap();
    let testlib = libraries.get("testlib").unwrap();
    assert_eq!(testlib.version, "1.2.3");
    let testdata = libraries.get("testdata").unwrap();
    assert_eq!(testdata.version, "4.5.6");
    assert!(libraries.get("testmore").is_none());

    assert_eq!(
        flags.to_string(),
        r#"cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu
cargo:rustc-link-search=framework=/usr/lib/x86_64-linux-gnu
cargo:rustc-link-lib=test
cargo:rustc-link-lib=framework=someframework
cargo:include=/usr/include/testlib
"#
    );
}

fn toml_err(path: &str, err_starts_with: &str) {
    let err = toml(path, vec![]).unwrap_err();
    if !err.description().starts_with(err_starts_with) {
        panic!(
            "Expected error to start with: {:?}\nGot error: {:?}",
            err_starts_with, err
        );
    }
}

// Assert a PkgConfig error because requested lib version cannot be found
fn toml_pkg_config_err_version(
    path: &str,
    expected_version: &str,
    env_vars: Vec<(&'static str, &'static str)>,
) {
    let err = toml(path, env_vars).unwrap_err();
    match err.kind() {
        ErrorKind::PkgConfig(e) => match e {
            pkg_config::Error::Failure {
                command: cmd,
                output: _,
            } => {
                let s = format!(">= {}\"", expected_version);
                assert!(cmd.ends_with(&s));
            }
            _ => panic!("Wrong pkg-config error type"),
        },
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn missing_file() {
    toml_err("toml-missing-file", "Error opening");
}

#[test]
fn missing_key() {
    toml_err("toml-missing-key", "No package.metadata.pkg-config in");
}

#[test]
fn not_table() {
    toml_err(
        "toml-not-table",
        "package.metadata.pkg-config not a table in",
    );
}

#[test]
fn version_missing() {
    toml_err(
        "toml-version-missing",
        "No version in package.metadata.pkg-config.testlib",
    );
}

#[test]
fn version_not_string() {
    toml_err(
        "toml-version-not-string",
        "package.metadata.pkg-config.testlib not a string or table",
    );
}

#[test]
fn version_in_table_not_string() {
    toml_err(
        "toml-version-in-table-not-string",
        "Unexpected key package.metadata.pkg-config.testlib.version type integer",
    );
}

#[test]
fn feature_not_string() {
    toml_err(
        "toml-feature-not-string",
        "Unexpected key package.metadata.pkg-config.testlib.feature type integer",
    );
}

#[test]
fn unexpected_key() {
    toml_err(
        "toml-unexpected-key",
        "Unexpected key package.metadata.pkg-config.testlib.color type string",
    );
}

#[test]
fn override_name() {
    let (libraries, _) = toml("toml-override-name", vec![]).unwrap();
    let testlib = libraries.get("testlib").unwrap();
    assert_eq!(testlib.version, "2.0.0");
}

#[test]
fn feature_versions() {
    let (libraries, _) = toml("toml-feature-versions", vec![]).unwrap();
    let testdata = libraries.get("testdata").unwrap();
    assert_eq!(testdata.version, "4.5.6");

    // version 5 is not available
    env::set_var("CARGO_FEATURE_V5", "");
    toml_pkg_config_err_version("toml-feature-versions", "5", vec![("CARGO_FEATURE_V5", "")]);

    // We check the highest version enabled by features
    env::set_var("CARGO_FEATURE_V6", "");
    toml_pkg_config_err_version("toml-feature-versions", "6", vec![("CARGO_FEATURE_V6", "")]);
}

#[test]
fn override_search_native() {
    let (libraries, flags) = toml(
        "toml-good",
        vec![("METADEPS_TESTLIB_SEARCH_NATIVE", "/custom/path:/other/path")],
    )
    .unwrap();
    let testlib = libraries.get("testlib").unwrap();
    assert_eq!(
        testlib.link_paths,
        vec![Path::new("/custom/path"), Path::new("/other/path")]
    );

    assert_eq!(
        flags.to_string(),
        r#"cargo:rustc-link-search=native=/custom/path
cargo:rustc-link-search=native=/other/path
cargo:rustc-link-search=framework=/usr/lib/x86_64-linux-gnu
cargo:rustc-link-lib=test
cargo:rustc-link-lib=framework=someframework
cargo:include=/usr/include/testlib
"#
    );
}

#[test]
fn override_search_framework() {
    let (libraries, flags) = toml(
        "toml-good",
        vec![("METADEPS_TESTLIB_SEARCH_FRAMEWORK", "/custom/path")],
    )
    .unwrap();
    let testlib = libraries.get("testlib").unwrap();
    assert_eq!(testlib.framework_paths, vec![Path::new("/custom/path")]);

    assert_eq!(
        flags.to_string(),
        r#"cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu
cargo:rustc-link-search=framework=/custom/path
cargo:rustc-link-lib=test
cargo:rustc-link-lib=framework=someframework
cargo:include=/usr/include/testlib
"#
    );
}

#[test]
fn override_lib() {
    let (libraries, flags) = toml(
        "toml-good",
        vec![("METADEPS_TESTLIB_LIB", "overrided-test other-test")],
    )
    .unwrap();
    let testlib = libraries.get("testlib").unwrap();
    assert_eq!(testlib.libs, vec!["overrided-test", "other-test"]);

    assert_eq!(
        flags.to_string(),
        r#"cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu
cargo:rustc-link-search=framework=/usr/lib/x86_64-linux-gnu
cargo:rustc-link-lib=overrided-test
cargo:rustc-link-lib=other-test
cargo:rustc-link-lib=framework=someframework
cargo:include=/usr/include/testlib
"#
    );
}

#[test]
fn override_framework() {
    let (libraries, flags) = toml(
        "toml-good",
        vec![("METADEPS_TESTLIB_LIB_FRAMEWORK", "overrided-framework")],
    )
    .unwrap();
    let testlib = libraries.get("testlib").unwrap();
    assert_eq!(testlib.frameworks, vec!["overrided-framework"]);

    assert_eq!(
        flags.to_string(),
        r#"cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu
cargo:rustc-link-search=framework=/usr/lib/x86_64-linux-gnu
cargo:rustc-link-lib=test
cargo:rustc-link-lib=framework=overrided-framework
cargo:include=/usr/include/testlib
"#
    );
}

#[test]
fn override_include() {
    let (libraries, flags) = toml(
        "toml-good",
        vec![("METADEPS_TESTLIB_INCLUDE", "/other/include")],
    )
    .unwrap();
    let testlib = libraries.get("testlib").unwrap();
    assert_eq!(testlib.include_paths, vec![Path::new("/other/include")]);

    assert_eq!(
        flags.to_string(),
        r#"cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu
cargo:rustc-link-search=framework=/usr/lib/x86_64-linux-gnu
cargo:rustc-link-lib=test
cargo:rustc-link-lib=framework=someframework
cargo:include=/other/include
"#
    );
}

#[test]
fn override_unset() {
    let (libraries, flags) = toml(
        "toml-good",
        vec![
            ("METADEPS_TESTLIB_SEARCH_NATIVE", ""),
            ("METADEPS_TESTLIB_SEARCH_FRAMEWORK", ""),
            ("METADEPS_TESTLIB_LIB", ""),
            ("METADEPS_TESTLIB_LIB_FRAMEWORK", ""),
            ("METADEPS_TESTLIB_INCLUDE", ""),
        ],
    )
    .unwrap();
    let testlib = libraries.get("testlib").unwrap();
    assert_eq!(testlib.link_paths, Vec::<PathBuf>::new());
    assert_eq!(testlib.framework_paths, Vec::<PathBuf>::new());
    assert_eq!(testlib.libs, Vec::<String>::new());
    assert_eq!(testlib.frameworks, Vec::<String>::new());
    assert_eq!(testlib.include_paths, Vec::<PathBuf>::new());

    assert_eq!(flags.to_string(), "");
}
