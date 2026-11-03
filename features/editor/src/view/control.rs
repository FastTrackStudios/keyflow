//! The host control API: read, set and focus a [`KeyflowEditor`](super::KeyflowEditor)
//! by its `id`, from any thread.
//!
//! A mounted editor with an `id` registers here. [`set_text`] and [`focus`]
//! queue a command and wake the editor, which applies it on the UI thread,
//! where the signals and element handles live. [`state`] reads a snapshot the
//! editor keeps up to date, so it never touches the live tree. The snapshot is
//! as of the editor's last render: a call right after a queued command can
//! still read the text from before it.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

/// What a host sees of an editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorState {
    /// The whole document.
    pub text: String,
    /// The primary cursor's head, as a byte offset into `text`.
    pub cursor: usize,
    /// Whether the editor's input has keyboard focus.
    pub focused: bool,
}

/// A request from outside the UI thread, applied by the editor on it.
pub(super) enum Command {
    /// Replace the whole document, cursor to its end.
    SetText(String),
    /// Move keyboard focus into the editor.
    Focus,
}

/// The snapshot the editor writes and [`state`] reads.
pub(super) type Snapshot = Arc<Mutex<EditorState>>;

struct Entry {
    /// Tells this editor's entry from a later one given the same id, so an
    /// unmount removes only its own.
    token: u64,
    commands: UnboundedSender<Command>,
    snapshot: Snapshot,
}

static REGISTRY: Mutex<BTreeMap<String, Entry>> = Mutex::new(BTreeMap::new());
static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

fn registry() -> MutexGuard<'static, BTreeMap<String, Entry>> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The UI end of a registered editor. Dropping it unregisters, so the
/// registration lasts exactly as long as the editor is mounted.
pub(super) struct Control {
    id: String,
    token: u64,
    pub(super) snapshot: Snapshot,
}

impl Control {
    /// Registers `id`. Returns the UI end, and the queue the editor drains on
    /// the UI thread.
    pub(super) fn register(id: &str) -> (Self, UnboundedReceiver<Command>) {
        let (commands, queue) = unbounded();
        let snapshot = Snapshot::new(Mutex::new(EditorState {
            text: String::new(),
            cursor: 0,
            focused: false,
        }));
        let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
        registry().insert(
            id.to_string(),
            Entry {
                token,
                commands,
                snapshot: snapshot.clone(),
            },
        );
        let control = Self {
            id: id.to_string(),
            token,
            snapshot,
        };
        (control, queue)
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        let mut map = registry();
        if map.get(&self.id).is_some_and(|e| e.token == self.token) {
            map.remove(&self.id);
        }
    }
}

/// Replaces the whole document with `text` and puts the cursor at its end.
/// `on_change` fires, as it does for typing. Queued: the editor applies it on
/// the UI thread. `Err` when no mounted editor has `id`.
pub fn set_text(id: &str, text: &str) -> Result<(), String> {
    send(id, Command::SetText(text.to_string()))
}

/// Gives keyboard focus to the editor's input. Queued like [`set_text`], and
/// applied even when the editor is not focused. `Err` when no mounted editor
/// has `id`.
pub fn focus(id: &str) -> Result<(), String> {
    send(id, Command::Focus)
}

/// The editor's text, cursor and focus, from its snapshot. `None` when no
/// mounted editor has `id`.
pub fn state(id: &str) -> Option<EditorState> {
    let map = registry();
    let entry = map.get(id)?;
    let snapshot = entry
        .snapshot
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    Some(snapshot.clone())
}

fn send(id: &str, command: Command) -> Result<(), String> {
    let map = registry();
    let entry = map.get(id).ok_or_else(|| format!("no editor {id}"))?;
    entry
        .commands
        .unbounded_send(command)
        .map_err(|_| format!("editor {id} is gone"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_round_trips_and_rejects_unknown_ids() {
        let unknown = "registry-test-unknown";
        assert_eq!(set_text(unknown, "x"), Err(format!("no editor {unknown}")));
        assert_eq!(focus(unknown), Err(format!("no editor {unknown}")));
        assert_eq!(state(unknown), None);

        // A plain handle with no UI: the test plays the editor's part, which
        // is to drain the queue and publish the snapshot.
        let id = "registry-test";
        let (control, mut queue) = Control::register(id);
        set_text(id, "C: | 1 |").unwrap();
        assert!(matches!(
            queue.try_recv(),
            Ok(Command::SetText(text)) if text == "C: | 1 |"
        ));
        focus(id).unwrap();
        assert!(matches!(queue.try_recv(), Ok(Command::Focus)));

        *control.snapshot.lock().unwrap() = EditorState {
            text: "C: | 1 |".to_string(),
            cursor: 9,
            focused: true,
        };
        assert_eq!(
            state(id),
            Some(EditorState {
                text: "C: | 1 |".to_string(),
                cursor: 9,
                focused: true,
            })
        );

        drop(control);
        assert_eq!(state(id), None);
        assert!(set_text(id, "gone").is_err());
    }
}
