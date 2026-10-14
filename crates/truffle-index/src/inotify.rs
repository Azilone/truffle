//! Linux stand-in for fsevents.rs: the same events, from inotify (through the
//! notify crate), so the engine can't tell them apart.
//!
//! Two differences from FSEvents, both absorbed by the engine:
//! - inotify keeps no history. A watch "since" an older id starts with the
//!   event FSEvents sends when its history is gone (`/`, must scan subdirs),
//!   and the engine relists the folders whose mtime moved since its last sync.
//! - inotify watches folders one by one, so only home is watched live. The
//!   rest of the disk (system folders, other homes) is caught up by that same
//!   relist at every start.

use notify::event::{CreateKind, EventKind, ModifyKind, RenameMode};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub const MUST_SCAN_SUBDIRS: u32 = 0x1;
pub const USER_DROPPED: u32 = 0x2;
pub const KERNEL_DROPPED: u32 = 0x4;
pub const HISTORY_DONE: u32 = 0x10;

pub struct Event {
    pub path: Vec<u8>,
    pub flags: u32,
    pub id: u64,
    /// Which `watch` delivered it (its `stream` number).
    pub stream: u32,
}

/// A running watch; dropping it stops it.
pub struct Stream(Arc<Mutex<Option<RecommendedWatcher>>>);

impl Drop for Stream {
    fn drop(&mut self) {
        self.0.lock().unwrap().take();
    }
}

/// Event ids are microseconds since the epoch: increasing across restarts,
/// as FSEvents ids are, so a saved index's id still orders against new ones.
pub fn current_id() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_micros() as u64)
}

/// Watch home from `since`. Batches of directory-level events, stamped with
/// `stream`, arrive on `tx` until the returned stream is dropped.
pub fn watch(since: u64, _latency: f64, tx: Sender<Vec<Event>>, stream: u32) -> Stream {
    let slot: Arc<Mutex<Option<RecommendedWatcher>>> = Arc::new(Mutex::new(None));
    let now = current_id();
    // What happened while nobody watched can't be replayed: ask for the
    // relist FSEvents asks for when its history is gone.
    let mut first = Vec::new();
    if since < now.saturating_sub(1_000_000) {
        first.push(Event { path: b"/".to_vec(), flags: MUST_SCAN_SUBDIRS, id: now, stream });
    }
    first.push(Event { path: b"/".to_vec(), flags: HISTORY_DONE, id: now, stream });
    let _ = tx.send(first);
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty()).unwrap_or_else(|| "/".into());
    let held = slot.clone();
    // A recursive inotify watch opens every folder under home: seconds on a
    // big one, so the caller doesn't wait for it.
    let spawned = std::thread::Builder::new().name("truffle-inotify".into()).spawn(move || {
        let handler_tx = tx.clone();
        let handler = move |res: notify::Result<notify::Event>| match res {
            Ok(e) => {
                let batch = events(&e, stream);
                if !batch.is_empty() {
                    let _ = handler_tx.send(batch);
                }
            }
            Err(e) => crate::engine::log(format!("inotify: {e}")),
        };
        let mut w = match notify::recommended_watcher(handler) {
            Ok(w) => w,
            Err(e) => return crate::engine::log(format!("inotify unavailable: {e}")),
        };
        if let Err(e) = w.watch(Path::new(&home), RecursiveMode::Recursive) {
            // Usually fs.inotify.max_user_watches: what is watched still is.
            crate::engine::log(format!("inotify on {}: {e}", Path::new(&home).display()));
        }
        let mut slot = held.lock().unwrap();
        if Arc::strong_count(&held) > 1 {
            *slot = Some(w);
        }
    });
    if let Err(e) = spawned {
        crate::engine::log(format!("inotify thread: {e}"));
    }
    Stream(slot)
}

/// One notify event as directory-level events: the folder whose listing
/// changed, and a new folder's whole subtree.
fn events(e: &notify::Event, stream: u32) -> Vec<Event> {
    let id = current_id();
    if e.need_rescan() {
        // The kernel queue overflowed: changes were lost.
        return vec![Event { path: b"/".to_vec(), flags: MUST_SCAN_SUBDIRS | KERNEL_DROPPED, id, stream }];
    }
    if matches!(e.kind, EventKind::Access(_)) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for p in &e.paths {
        let tree = match e.kind {
            EventKind::Create(CreateKind::Folder) => true,
            EventKind::Modify(ModifyKind::Name(RenameMode::To | RenameMode::Both | RenameMode::Any)) => p.is_dir(),
            _ => false,
        };
        if tree {
            out.push(Event { path: p.as_os_str().as_bytes().to_vec(), flags: MUST_SCAN_SUBDIRS, id, stream });
        }
        if let Some(parent) = p.parent() {
            out.push(Event { path: parent.as_os_str().as_bytes().to_vec(), flags: 0, id, stream });
        }
    }
    out
}
