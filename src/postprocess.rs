use std::process::Command;

use tempfile::TempPath;
use tracing::debug;

use crate::Error;

pub trait PostProcessor: Send {
    /// Checks whether the post-processor is functional.
    fn check(&self) -> Result<bool, Error>;

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

    fn check(&self) -> Result<bool, Error> {
        match Command::new("exiftran").arg("-h").output() {
            Ok(output) => Ok(output.status.success()),
            Err(_) => Err(Error::ToolCheckFailed("exiftran".to_string())),
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
            "image/jpeg"
                | "image/png"
                | "image/heic"
                | "image/webp"
                | "video/mp4"
                | "video/heic"
                | "video/mpeg"
                | "video/x-quicktime"
        )
    }

    fn check(&self) -> Result<bool, Error> {
        match Command::new("exiftool").arg("-ver").output() {
            Ok(output) => Ok(output.status.success()),
            Err(_) => Err(Error::ToolCheckFailed("exiftool".to_string())),
        }
    }

    fn apply(&self, path: TempPath) -> Result<TempPath, Error> {
        debug!("running exiftool on {path}", path = &path.display());

        match Command::new("exiftool").args(["-all="]).arg(&path).status() {
            Ok(status) if status.success() => Ok(path),
            _ => Err(Error::PostProcessFailed("exiftool failed".to_string())),
        }
    }
}

pub fn init() -> Result<Vec<Box<dyn PostProcessor>>, Error> {
    debug!("initializing postprocessors");

    let processors: Vec<Box<dyn PostProcessor>> =
        vec![Box::new(RotateImageExif), Box::new(RemoveExif)];

    for processor in &processors {
        let _ = processor.check()?;
    }

    Ok(processors)
}
