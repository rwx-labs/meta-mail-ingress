use std::process::Command;

use tempfile::TempPath;
use tracing::{debug, error};

use crate::Error;

pub trait PostProcessor: Send {
    /// Checks whether the post-processor is functional.
    fn check(&self) -> bool;

    /// Returns true if the postprocessor should run for the provided `mime_type`.
    fn applicable(&self, mime_type: &'static str) -> bool;

    fn apply(&self, path: TempPath) -> Result<TempPath, Error>;
}

use core::fmt::Debug;

impl Debug for dyn PostProcessor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PostProcessor{{}}")
    }
}

struct RotateImageExif;

impl PostProcessor for RotateImageExif {
    fn applicable(&self, mime_type: &str) -> bool {
        matches!(
            mime_type,
            "image/jpeg" | "image/png" | "image/heic" | "image/webp"
        )
    }

    fn check(&self) -> bool {
        match Command::new("exiftran").arg("-h").output() {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    fn apply(&self, path: TempPath) -> Result<TempPath, Error> {
        debug!("running exiftran on {path}", path = &path.display());

        match Command::new("exiftran")
            .args(["-i", "-a"])
            .arg(&path)
            .status()
        {
            Ok(status) if status.success() => Ok(path),
            _ => Err(Error::PostProcessFailed("exiftran failed".to_string())),
        }
    }
}

struct RemoveExif;

impl PostProcessor for RemoveExif {
    fn applicable(&self, mime_type: &str) -> bool {
        matches!(
            mime_type,
            "image/jpeg" | "image/png" | "image/heic" | "image/webp"
        )
    }

    fn check(&self) -> bool {
        match Command::new("exiv2").arg("--version").output() {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    fn apply(&self, path: TempPath) -> Result<TempPath, Error> {
        debug!("running exiv2 on {path}", path = &path.display());

        match Command::new("exiv2").args(["rm"]).arg(&path).status() {
            Ok(status) if status.success() => Ok(path),
            _ => Err(Error::PostProcessFailed("exiv2 failed".to_string())),
        }
    }
}

pub fn init() -> Vec<Box<dyn PostProcessor>> {
    debug!("initializing postprocessors");

    let processors: Vec<Box<dyn PostProcessor>> =
        vec![Box::new(RotateImageExif), Box::new(RemoveExif)];

    if processors.iter().any(|x| !x.check()) {
        let msg = "postprocessor failed sanity check";

        error!(msg);
        panic!("{}", msg);
    }

    processors
}
