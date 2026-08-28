//! The chart library.
//!
//! Charts on the phone live in one JSON file in the app's shared container.
//! A container rather than the app's own documents directory because the
//! keyboard extension is a separate process with its own sandbox: an App
//! Group is the only way a chart written on the keyboard reaches the app
//! (see `ios/README.md`).
//!
//! The store is deliberately a single file. A chart is a couple of
//! kilobytes of text, a working musician has tens to hundreds of them, and
//! the whole library is smaller than one album cover — a database would be
//! machinery without a problem to solve. When sync arrives it replaces this
//! wholesale rather than growing out of it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One saved chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Stable identifier, used in routes.
    pub id: String,
    /// Display name. Falls back to the chart's own title line.
    pub title: String,
    /// The Keyflow source.
    pub source: String,
    /// Last-modified, as seconds since the Unix epoch.
    ///
    /// A plain integer rather than a datetime type: it is only ever used to
    /// sort, it survives JSON round-trips without a format decision, and it
    /// keeps chrono out of an app-extension memory budget.
    pub modified: u64,
}

impl Entry {
    /// Create an entry, taking its title from the chart's first line when
    /// the chart has one.
    #[must_use]
    pub fn new(id: impl Into<String>, source: impl Into<String>, modified: u64) -> Self {
        let source = source.into();
        Self {
            id: id.into(),
            title: title_of(&source),
            source,
            modified,
        }
    }
}

/// The chart's own title, or a placeholder.
///
/// Keyflow's metadata line is `Title - Artist`, so the title is everything
/// before the first ` - ` on the first non-empty, non-fence line.
#[must_use]
pub fn title_of(source: &str) -> String {
    let first = source
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("---"));

    match first {
        Some(line) => line
            .split_once(" - ")
            .map_or(line, |(title, _artist)| title)
            .trim()
            .to_owned(),
        None => "Untitled".to_owned(),
    }
}

/// Every chart on the device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Library {
    /// Saved charts. Kept sorted most-recent-first by [`Library::upsert`].
    pub entries: Vec<Entry>,
}

impl Library {
    /// Insert or replace a chart, then re-sort so the list reads
    /// most-recent-first.
    pub fn upsert(&mut self, entry: Entry) {
        match self.entries.iter_mut().find(|e| e.id == entry.id) {
            Some(existing) => *existing = entry,
            None => self.entries.push(entry),
        }
        self.entries
            .sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.id.cmp(&b.id)));
    }

    /// Look up a chart.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Remove a chart. Returns whether anything was removed.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        before != self.entries.len()
    }

    /// Read a library from disk.
    ///
    /// A missing file is an empty library, not an error — that is first
    /// launch. A *corrupt* file is an error, because silently starting over
    /// would discard the user's charts.
    ///
    /// # Errors
    ///
    /// Returns the underlying error if the file exists but cannot be read
    /// or parsed.
    pub fn load(path: &Path) -> Result<Self, LoadError> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(LoadError::Corrupt),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(LoadError::Io(e)),
        }
    }

    /// Write the library to disk.
    ///
    /// Writes to a sibling temporary file and renames, so an interrupted
    /// save (the system killing a memory-hungry keyboard extension, say)
    /// cannot leave a half-written library behind.
    ///
    /// # Errors
    ///
    /// Returns any I/O error from writing or renaming.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = temp_path(path);
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(self).expect("a Library always serializes"),
        )?;
        std::fs::rename(&tmp, path)
    }
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Why a library could not be read.
#[derive(Debug)]
pub enum LoadError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file was read but is not a valid library.
    Corrupt(serde_json::Error),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "could not read the chart library: {e}"),
            Self::Corrupt(e) => write!(f, "the chart library is damaged: {e}"),
        }
    }
}

impl std::error::Error for LoadError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_title_from_the_chart() {
        assert_eq!(
            title_of("Build My Life - Housefires\n4/4\n"),
            "Build My Life"
        );
        assert_eq!(title_of("Thriller\n"), "Thriller");
    }

    #[test]
    fn skips_a_leading_fence_when_finding_the_title() {
        assert_eq!(title_of("--- keyflow ---\nThriller - MJ\n"), "Thriller");
    }

    #[test]
    fn an_empty_chart_still_has_a_name() {
        assert_eq!(title_of(""), "Untitled");
        assert_eq!(title_of("\n\n  \n"), "Untitled");
    }

    #[test]
    fn upsert_replaces_rather_than_duplicating() {
        let mut lib = Library::default();
        lib.upsert(Entry::new("a", "One - X", 1));
        lib.upsert(Entry::new("a", "Two - X", 2));
        assert_eq!(lib.entries.len(), 1);
        assert_eq!(lib.get("a").unwrap().title, "Two");
    }

    #[test]
    fn the_list_reads_most_recent_first() {
        let mut lib = Library::default();
        lib.upsert(Entry::new("old", "Old", 1));
        lib.upsert(Entry::new("new", "New", 3));
        lib.upsert(Entry::new("mid", "Mid", 2));
        let ids: Vec<&str> = lib.entries.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["new", "mid", "old"]);
    }

    #[test]
    fn remove_reports_whether_it_did_anything() {
        let mut lib = Library::default();
        lib.upsert(Entry::new("a", "A", 1));
        assert!(lib.remove("a"));
        assert!(!lib.remove("a"));
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("kf-lib-{}", std::process::id()));
        let path = dir.join("library.json");
        let _ = std::fs::remove_dir_all(&dir);

        let mut lib = Library::default();
        lib.upsert(Entry::new("a", "Build My Life - Housefires\n", 7));
        lib.save(&path).unwrap();

        assert_eq!(Library::load(&path).unwrap(), lib);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_library_is_first_launch_not_an_error() {
        let path = std::env::temp_dir().join("kf-does-not-exist/library.json");
        assert_eq!(Library::load(&path).unwrap(), Library::default());
    }

    #[test]
    fn a_damaged_library_is_an_error_rather_than_silent_data_loss() {
        let dir = std::env::temp_dir().join(format!("kf-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("library.json");
        std::fs::write(&path, "{ not json").unwrap();

        assert!(matches!(Library::load(&path), Err(LoadError::Corrupt(_))));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saving_leaves_no_temp_file_behind() {
        let dir = std::env::temp_dir().join(format!("kf-tmp-{}", std::process::id()));
        let path = dir.join("library.json");
        let _ = std::fs::remove_dir_all(&dir);

        Library::default().save(&path).unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .filter(|n| n.to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "left {leftovers:?} behind");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
