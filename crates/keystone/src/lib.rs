//! # keystone
//!
//! A projector's 4-corner keystone, as a file. The player only *reads* it; a
//! remote *writes* it. No dependencies — not even vybe — so anything that
//! projects onto a flat face can use it.
//!
//! The file (`keystone.json`) is small and hand-editable:
//!
//! ```json
//! {
//!   "version": 1,
//!   "output": [1920, 1200],
//!   "corners": [[0, 0], [1, 0], [1, 1], [0, 1]],
//!   "feather": 0
//! }
//! ```
//!
//! `corners` are TL, TR, BR, BL, each a fraction of the output, y downward. A
//! 4-corner homography is exact for a flat face, so that is the whole model:
//! no mesh. Saving is atomic (write a temp file, rename over) and keeps the
//! previous calibration as `.bak` — a power cut mid-save on a wall-mounted
//! player must never cost the calibration.

mod json;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use json::Json;

/// The file format's version this crate writes.
pub const VERSION: u32 = 1;

/// One face's calibration.
#[derive(Clone, Debug, PartialEq)]
pub struct Keystone {
    /// The output it was calibrated on, in pixels. Informative: corners are
    /// fractions, so a calibration survives a change of resolution.
    pub output: [u32; 2],
    /// TL, TR, BR, BL — fractions of the output, x rightward, y downward.
    pub corners: [[f32; 2]; 4],
    /// Soft edge, as a fraction of the picture.
    pub feather: f32,
}

impl Default for Keystone {
    /// Uncalibrated: the picture is the whole output.
    fn default() -> Self {
        Self {
            output: [1920, 1200],
            corners: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            feather: 0.0,
        }
    }
}

/// Why a calibration could not be read or written.
#[derive(Debug)]
pub enum Error {
    Io(PathBuf, std::io::Error),
    /// The file is not a keystone: what is wrong, in words.
    Format(PathBuf, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(path, e) => write!(f, "{}: {e}", path.display()),
            Error::Format(path, why) => write!(f, "{}: {why}", path.display()),
        }
    }
}

impl std::error::Error for Error {}

impl Keystone {
    /// Reads a calibration. A missing file is not an error here — see
    /// [`Keystone::load_or_default`] for the player's "uncalibrated is fine".
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = fs::read_to_string(path).map_err(|e| Error::Io(path.to_owned(), e))?;
        Self::parse(&text).map_err(|why| Error::Format(path.to_owned(), why))
    }

    /// Reads a calibration, or starts uncalibrated when there is no file yet.
    /// A file that exists but is broken is still an error: silently projecting
    /// uncalibrated over a damaged calibration would hide the damage.
    pub fn load_or_default(path: &Path) -> Result<Self, Error> {
        Self::load_or(path, Self::default())
    }

    /// Like [`Keystone::load_or_default`], but "uncalibrated" is `rest` — for a
    /// picture that doesn't rest on the whole output (a square face, centered
    /// in a 16:10 projector).
    pub fn load_or(path: &Path, rest: Self) -> Result<Self, Error> {
        match Self::load(path) {
            Err(Error::Io(_, e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(rest),
            other => other,
        }
    }

    /// Writes the calibration atomically, keeping the previous one as `.bak`.
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        let io = |e| Error::Io(path.to_owned(), e);
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir).map_err(io)?;
        }
        let tmp = with_suffix(path, ".tmp");
        fs::write(&tmp, self.to_json()).map_err(io)?;
        if path.exists() {
            fs::copy(path, with_suffix(path, ".bak")).map_err(io)?;
        }
        // The rename is the commit: readers see the old file or the new one,
        // never half of either.
        fs::rename(&tmp, path).map_err(io)
    }

    pub fn to_json(&self) -> String {
        let corners: Vec<String> = self
            .corners
            .iter()
            .map(|[x, y]| format!("[{x}, {y}]"))
            .collect();
        format!(
            "{{\n  \"version\": {VERSION},\n  \"output\": [{}, {}],\n  \"corners\": [{}],\n  \"feather\": {}\n}}\n",
            self.output[0],
            self.output[1],
            corners.join(", "),
            self.feather,
        )
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let root = json::parse(text)?;
        let version = root.get("version").and_then(Json::number).unwrap_or(1.0);
        if version as u32 > VERSION {
            return Err(format!(
                "written by a newer keystone (version {version}); this one reads up to {VERSION}"
            ));
        }
        let mut keystone = Self::default();
        if let Some(output) = root.get("output").and_then(Json::array) {
            match output {
                [w, h] => {
                    keystone.output = [
                        w.number().ok_or("`output` must be two numbers")? as u32,
                        h.number().ok_or("`output` must be two numbers")? as u32,
                    ];
                }
                _ => return Err("`output` must be [width, height]".into()),
            }
        }
        let corners = root
            .get("corners")
            .and_then(Json::array)
            .ok_or("missing `corners`: four [x, y] pairs, TL TR BR BL")?;
        if corners.len() != 4 {
            return Err(format!(
                "`corners` has {} entries; a keystone has four (TL TR BR BL)",
                corners.len()
            ));
        }
        for (corner, value) in keystone.corners.iter_mut().zip(corners) {
            match value.array() {
                Some([x, y]) => {
                    *corner = [
                        x.number().ok_or("a corner must be two numbers")? as f32,
                        y.number().ok_or("a corner must be two numbers")? as f32,
                    ];
                }
                _ => return Err("each corner must be [x, y]".into()),
            }
        }
        if let Some(feather) = root.get("feather").and_then(Json::number) {
            keystone.feather = feather as f32;
        }
        Ok(keystone)
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_its_own_json() {
        let keystone = Keystone {
            output: [1920, 1200],
            corners: [[0.02, 0.01], [0.97, 0.0], [1.0, 0.98], [-0.01, 1.0]],
            feather: 0.05,
        };
        assert_eq!(Keystone::parse(&keystone.to_json()).unwrap(), keystone);
    }

    #[test]
    fn a_missing_file_is_uncalibrated_but_a_broken_one_is_an_error() {
        let dir = std::env::temp_dir().join(format!("keystone-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keystone.json");
        assert_eq!(
            Keystone::load_or_default(&path).unwrap(),
            Keystone::default()
        );
        fs::write(&path, "{ \"corners\": [[0,0]] }").unwrap();
        assert!(Keystone::load_or_default(&path).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saving_keeps_the_previous_calibration_as_bak() {
        let dir = std::env::temp_dir().join(format!("keystone-bak-{}", std::process::id()));
        let path = dir.join("config").join("keystone.json");
        let first = Keystone::default();
        let second = Keystone {
            feather: 0.1,
            ..Keystone::default()
        };
        first.save(&path).unwrap();
        second.save(&path).unwrap();
        assert_eq!(Keystone::load(&path).unwrap(), second);
        assert_eq!(Keystone::load(&with_suffix(&path, ".bak")).unwrap(), first);
        assert!(!with_suffix(&path, ".tmp").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn errors_say_what_is_wrong() {
        let err = Keystone::parse("{ \"corners\": [[0,0],[1,0]] }").unwrap_err();
        assert!(err.contains("four"), "{err}");
    }
}
