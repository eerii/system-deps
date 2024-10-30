pub use build::main;

#[cfg(not(feature = "binary"))]
mod build {
    pub fn main() {}
}

#[cfg(feature = "binary")]
mod build {
    use std::{
        env, fs, io,
        path::{Path, PathBuf},
    };

    #[derive(Debug)]
    enum Extension {
        #[cfg(feature = "gz")]
        TarGz,
        #[cfg(feature = "xz")]
        TarXz,
        #[cfg(feature = "zip")]
        Zip,
    }

    const _: () = {
        let enabled_features = {
            cfg!(feature = "gz") as u32 + cfg!(feature = "xz") as u32 + cfg!(feature = "zip") as u32
        };
        if enabled_features == 0 {
            panic!("You must enable at least one binary format feature ('gz', 'xz', 'zip')");
        }
    };

    fn metadata_or_env(name: &str) -> Option<String> {
        println!("cargo:warning=ENV {:?}", env::vars());
        env::var(format!("SYSTEM_DEPS_{}", name))
            .or(env::var(format!("DEP_SYSTEM_DEPS_ENV_{}", name)))
            .or(env::var(name))
            .ok()
    }

    pub fn main() {
        let out_dir = metadata_or_env("OUT_DIR").unwrap();
        let mut out_path = PathBuf::from(out_dir);
        out_path.push("binaries");

        let url = metadata_or_env("BINARY_URL").unwrap();
        let checksum = metadata_or_env("BINARY_CHECKSUM");

        // Only download the binaries if there isn't already a valid copy
        if !check_valid_dir(&out_path, checksum)
            .expect("Error when checking the download directory")
        {
            download(&url, &out_path).expect("Error when getting binaries");
        }
    }

    fn check_valid_dir(dir: &Path, checksum: Option<String>) -> io::Result<bool> {
        // If it doesn't exist yet everything is ok
        if !dir.try_exists()? {
            return Ok(false);
        }

        // Raise an error if it is a file
        if dir.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("The target directory is a file {:?}", dir),
            ));
        }

        // If the directory is empty, files need to be downloaded
        let mut contents = dir.read_dir()?.peekable();
        if contents.peek().is_none() {
            return Ok(false);
        }

        // If a checksum is not specified, assume the directory is valid
        let Some(checksum) = checksum else {
            return Ok(true);
        };

        // Check if the checksum is valid
        let valid = contents
            .find(|f| f.as_ref().is_ok_and(|f| f.file_name() == "checksum"))
            .and_then(|s| s.ok())
            .and_then(|s| fs::read_to_string(s.path()).ok())
            .and_then(|s| (checksum == s).then_some(()))
            .is_some();

        // Update the checksum
        let mut path = dir.to_path_buf();
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
        let reader;
        let mut archive = match ext {
            #[cfg(feature = "gz")]
            Extension::TarGz => {
                reader = flate2::read::GzDecoder::new(file);
                tar::Archive::new(reader)
            }
            #[cfg(feature = "xz")]
            Extension::TarXz => {
                reader = xz::read::XzDecoder::new(file);
                tar::Archive::new(reader)
            }
            #[cfg(feature = "zip")]
            Extension::Zip => todo!(),
        };
        archive.unpack(dst)?;
        Ok(())
    }
}
