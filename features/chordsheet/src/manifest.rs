//! The `manifest.json` a chordsheet.com account backup ships.
//!
//! The source `.txt` files carry only chord data — the site keeps title,
//! artist and the song id in its database. The backup writes them out
//! alongside, one entry per song, keyed by `source_file`. Without it an
//! importer has nothing but the filename to go on.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::convert::ImportOptions;

/// One song in the backup manifest.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Entry {
    /// chordsheet.com's song id.
    pub id: String,
    pub slug: String,
    pub title: String,
    pub artist: String,
    /// Filename within the backup's `source/` directory.
    pub source_file: String,
    #[serde(default)]
    pub pdf_file: Option<String>,
    #[serde(default)]
    pub pdf_url: Option<String>,
}

impl Entry {
    /// The title and artist this entry carries, as import options.
    #[must_use]
    pub fn options(&self) -> ImportOptions {
        ImportOptions {
            title: (!self.title.trim().is_empty()).then(|| self.title.trim().to_string()),
            artist: (!self.artist.trim().is_empty()).then(|| self.artist.trim().to_string()),
            ..ImportOptions::default()
        }
    }
}

/// A parsed `manifest.json`, indexed by `source_file`.
#[derive(Debug, Clone, Default)]
pub struct Manifest {
    entries: Vec<Entry>,
    by_source: HashMap<String, usize>,
}

impl Manifest {
    /// Parse the JSON array a backup's `manifest.json` holds.
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        Ok(Self::from_entries(serde_json::from_str(json)?))
    }

    /// Read `manifest.json` from disk.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, crate::ImportError> {
        let text =
            std::fs::read_to_string(path.as_ref()).map_err(|source| crate::ImportError::Io {
                path: path.as_ref().to_path_buf(),
                source,
            })?;
        Self::parse(&text).map_err(|source| crate::ImportError::Manifest {
            path: path.as_ref().to_path_buf(),
            source,
        })
    }

    #[must_use]
    pub fn from_entries(entries: Vec<Entry>) -> Self {
        let by_source = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.source_file.clone(), i))
            .collect();
        Self { entries, by_source }
    }

    /// Look an entry up by its `source/` filename (with extension).
    #[must_use]
    pub fn get(&self, source_file: &str) -> Option<&Entry> {
        self.by_source.get(source_file).map(|i| &self.entries[*i])
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Import options for a source path: the manifest entry when the backup
    /// knows the file, and the filename stem otherwise.
    #[must_use]
    pub fn options_for(&self, source_path: &Path) -> ImportOptions {
        let name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if let Some(entry) = self.get(name) {
            return entry.options();
        }
        ImportOptions::from_stem(
            source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_backup_manifest_entry() {
        let manifest = Manifest::parse(
            r#"[{
                "id": "246597",
                "slug": "highway-to-hell-ac-dc",
                "title": "Highway to Hell",
                "artist": "AC/DC",
                "source_file": "AC-DC - Highway to Hell.txt",
                "pdf_file": "AC-DC - Highway to Hell.pdf"
            }]"#,
        )
        .expect("manifest should parse");

        let entry = manifest
            .get("AC-DC - Highway to Hell.txt")
            .expect("entry should be indexed by source_file");
        assert_eq!(entry.artist, "AC/DC");

        // The manifest is the authority — the filename says "AC-DC", which is
        // a filesystem-safe mangling of the real artist name.
        let options = manifest.options_for(Path::new("source/AC-DC - Highway to Hell.txt"));
        assert_eq!(options.artist.as_deref(), Some("AC/DC"));
        assert_eq!(options.title.as_deref(), Some("Highway to Hell"));
    }

    #[test]
    fn falls_back_to_the_filename_when_the_manifest_has_no_entry() {
        let manifest = Manifest::default();
        let options = manifest.options_for(Path::new("source/MUSE - UPRISING.txt"));
        assert_eq!(options.artist.as_deref(), Some("MUSE"));
        assert_eq!(options.title.as_deref(), Some("UPRISING"));
    }
}
