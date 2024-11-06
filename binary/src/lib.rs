use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

#[derive(Debug)]
enum Extension {
    #[cfg(feature = "gz")]
    TarGz,
    #[cfg(feature = "xz")]
    TarXz,
    #[cfg(feature = "zip")]
    Zip,
}

#[derive(Debug, Default, Deserialize)]
struct Binary {
    url: String,
    checksum: Option<String>,
    pkg_paths: Option<Vec<String>>,
}

/// Warn the user that one decompressing feature must be enabled
const _: () = {
    let enabled_features = {
        cfg!(feature = "gz") as u32 + cfg!(feature = "xz") as u32 + cfg!(feature = "zip") as u32
    };
    if enabled_features == 0 {
        panic!("You must enable at least one binary format feature ('gz', 'xz', 'zip')");
    }
};

// TODO: Reload on cargo.toml change
// TODO: Per package build config, adapt the env that we are passing

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
        if let Some(p) = bin.pkg_paths {
            paths.extend(p.iter().map(|p| dst.join(p)));
        }
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
    let ext = get_ext(url)?;

    // Local file
    if let Some(file_path) = url.strip_prefix("file://") {
        let file = fs::read(Path::new(file_path))?;
        decompress(&file, dst, ext)?;
    }
    // Download
    else {
        let file = reqwest::blocking::get(url)
            .and_then(|req| req.bytes())
            .map_err(|e| {
                io::Error::new(io::ErrorKind::Other, format!("Download error: {:?}", e))
            })?;
        decompress(&file, dst, ext)?;
    }

    Ok(())
}

fn get_ext(_url: &str) -> io::Result<Extension> {
    Ok(match _url {
        #[cfg(feature = "gz")]
        u if u.ends_with(".tar.gz") => Extension::TarGz,
        #[cfg(feature = "xz")]
        u if u.ends_with(".tar.xz") => Extension::TarXz,
        #[cfg(feature = "zip")]
        u if u.ends_with(".zip") => Extension::Zip,
        u => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Not suppported binary extension, {:?}", u.split(".").last()),
            ));
        }
    })
}

fn decompress(file: &[u8], dst: &Path, ext: Extension) -> io::Result<()> {
    match ext {
        #[cfg(feature = "gz")]
        Extension::TarGz => {
            let reader = flate2::read::GzDecoder::new(file);
            let mut archive = tar::Archive::new(reader);
            archive.unpack(dst)?;
        }
        #[cfg(feature = "xz")]
        Extension::TarXz => {
            let reader = xz::read::XzDecoder::new(file);
            let mut archive = tar::Archive::new(reader);
            archive.unpack(dst)?;
        }
        #[cfg(feature = "zip")]
        Extension::Zip => {
            let reader = io::Cursor::new(file);
            let mut archive = zip::ZipArchive::new(reader)?;
            archive.extract(dst)?;
        }
    };
    Ok(())
}
