use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

/// The extension of the binary archive.
/// Support for different extensions is enabled using features.
#[derive(Debug)]
enum Extension {
    /// A `.tar.gz` archive.
    #[cfg(feature = "gz")]
    TarGz,
    /// A `.tar.xz` archive.
    #[cfg(feature = "xz")]
    TarXz,
    /// A `.zip` archive.
    #[cfg(feature = "zip")]
    Zip,
    /// Untested.
    #[cfg(feature = "pkg")]
    Pkg,
}

/// Represents one location from where to download library binaries.
#[derive(Debug, Default, Deserialize)]
struct Binary {
    /// The url from which to download the archived binaries. It suppports:
    ///
    /// - Web urls, in the form `http[s]://website/archive.ext`.
    ///   This must directly download an archive with a known `Extension`.
    /// - Local files, in the form `file:///path/to/archive.ext`.
    ///   Note that this is made of the url descriptor `file://`, and then an absolute path, that
    ///   starts with `/`, so three total slashes are needed.
    ///   The path can point at an archive with a known `Extension`, or to a folder containing the
    ///   uncompressed binaries.
    url: String,
    /// Optionally, a checksum of the downloaded archive. When set, it is used to correctly cache
    /// the result. If this is not specified, it will still be cached by cargo, but redownloads
    /// might happen more often. It has no effect if `url` is a local folder.
    checksum: Option<String>,
    /// A list of relative paths inside the binary archive that point to a folder containing
    /// package config files. These directories will be prepended to the `PKG_CONFIG_PATH` when
    /// compiling the affected libraries.
    pkg_paths: Vec<String>,
}

// TODO: Reload on cargo.toml change
// TODO: Per package build config, adapt the env that we are passing

/// Reads metadata from the cargo manifests and the environment to build a list of urls from where
pub fn build() -> Vec<PathBuf> {
    let values = system_deps_meta::read_metadata("system-deps");

    // Read metadata from the crate graph
    let binaries = values
        .into_iter()
        .filter_map(|(n, v)| Some((n, system_deps_meta::from_value(v).ok()?)))
        .collect::<HashMap<String, Binary>>();
    let mut paths = vec![];

    println!("cargo:warning=BINARIES {:?}", binaries);
    println!(
        "cargo:warning=TARGET DIR {}",
        system_deps_meta::BUILD_TARGET_DIR
    );

    for (name, bin) in binaries {
        let mut dst = PathBuf::from(&system_deps_meta::BUILD_TARGET_DIR);
        if !name.is_empty() {
            dst.push(name);
        };

        // Only download the binaries if there isn't already a valid copy
        if !check_valid_dir(&dst, bin.checksum).expect("Error when checking the download directory")
        {
            download(&bin.url, &dst).expect("Error when getting binaries");
        }

        // Add pkg config paths to the overrides
        paths.extend(bin.pkg_paths.iter().map(|p| dst.join(p)));
    }

    paths
}

fn check_valid_dir(dst: &Path, checksum: Option<String>) -> io::Result<bool> {
    // If it doesn't exist yet everything is ok
    if !dst.try_exists()? {
        return Ok(false);
    }

    // Raise an error if it is a file
    if dst.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("The target directory is a file {:?}", dst),
        ));
    }

    // If a checksum is not specified, assume the directory is invalid
    let Some(checksum) = checksum else {
        return Ok(false);
    };

    // Check if the checksum is valid
    let valid = dst
        .read_dir()?
        .find(|f| f.as_ref().is_ok_and(|f| f.file_name() == "checksum"))
        .and_then(|s| s.ok())
        .and_then(|s| fs::read_to_string(s.path()).ok())
        .and_then(|s| (checksum == s).then_some(()))
        .is_some();

    // Update the checksum
    let mut path = dst.to_path_buf();
    path.push("checksum");
    fs::write(path, checksum)?;

    Ok(valid)
}

fn download(url: &str, dst: &Path) -> io::Result<()> {
    let ext = match url {
        #[cfg(feature = "gz")]
        u if u.ends_with(".tar.gz") => Ok(Extension::TarGz),
        #[cfg(feature = "xz")]
        u if u.ends_with(".tar.xz") => Ok(Extension::TarXz),
        #[cfg(feature = "zip")]
        u if u.ends_with(".zip") => Ok(Extension::Zip),
        #[cfg(feature = "pkg")]
        u if u.ends_with(".pkg") => Ok(Extension::Pkg),
        u => Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Unsuppported binary extension, {:?}", u.split(".").last()),
        )),
    };

    // Local file
    if let Some(file_path) = url.strip_prefix("file://") {
        let path = Path::new(file_path);
        match ext {
            Ok(ext) => {
                let file = fs::read(path)?;
                decompress(&file, dst, ext)?;
            }
            Err(e) => {
                if path.is_dir() {
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(file_path, dst)?;
                    #[cfg(windows)]
                    std::os::windows::fs::symlink_dir(file_path, dst)?;
                } else {
                    return Err(e);
                };
            }
        };
    }
    // Download
    else {
        let ext = ext?;
        let file = reqwest::blocking::get(url)
            .and_then(|req| req.bytes())
            .map_err(|e| {
                io::Error::new(io::ErrorKind::Other, format!("Download error: {:?}", e))
            })?;
        decompress(&file, dst, ext)?;
    }

    Ok(())
}

fn decompress(file: &[u8], dst: &Path, ext: Extension) -> io::Result<()> {
    match ext {
        #[cfg(feature = "gz")]
        Extension::TarGz => {
            let reader = flate2::read::GzDecoder::new(file);
            let mut archive = tar::Archive::new(reader);
            archive.unpack(dst)?;
            Ok(())
        }
        #[cfg(feature = "xz")]
        Extension::TarXz => {
            let reader = xz::read::XzDecoder::new(file);
            let mut archive = tar::Archive::new(reader);
            archive.unpack(dst)?;
            Ok(())
        }
        #[cfg(feature = "zip")]
        Extension::Zip => {
            let reader = io::Cursor::new(file);
            let mut archive = zip::ZipArchive::new(reader)?;
            archive.extract(dst)?;
            Ok(())
        }
        #[cfg(feature = "pkg")]
        Extension::Pkg => {
            // TODO: Test this with actual pkg files, do they have pc files inside?
            // TODO: Error handling
            let reader = io::Cursor::new(file);
            let mut archive = apple_flat_package::PkgReader::new(reader).unwrap();
            let pkgs = archive.component_packages().unwrap();
            let mut cpio = pkgs.first().unwrap().payload_reader().unwrap().unwrap();
            while let Some(next) = cpio.next() {
                let entry = next.unwrap();
                let mut file = Vec::new();
                cpio.read_to_end(&mut file).unwrap();
                if entry.file_size() != 0 {
                    let dst = dst.join(entry.name());
                    fs::create_dir_all(dst.parent().unwrap())?;
                    fs::write(&dst, file)?;
                }
            }
            Ok(())
        }
    }
}
