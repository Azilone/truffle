//! The meaning index inside the daemon. Searches run in the daemon (`Live`); indexing runs in a child process
//! (`truffle meaning index-service`), started when the folders to search change and gone once it's done, so the
//! image and audio encoders' memory goes back to the system. The name index's watcher tells it what changed: one
//! watcher for both indexes.
//!
//! The folders to search are in `<data>/meaning-folders`, one per line (`truffle meaning folders add <dir>`).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use truffle_semantic::paths;

use super::search_service::Live;

pub const FOLDERS_FILE: &str = "meaning-folders";

/// The folders the meaning index covers.
pub fn folders() -> Vec<String> {
    std::fs::read_to_string(paths::data_root().join(FOLDERS_FILE))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

pub fn set_folders(list: &[String]) -> Result<(), String> {
    std::fs::create_dir_all(paths::data_root()).map_err(|e| e.to_string())?;
    let mut text = String::from("# Folders Truffle searches by meaning, one per line.\n");
    for f in list {
        text.push_str(f);
        text.push('\n');
    }
    std::fs::write(paths::data_root().join(FOLDERS_FILE), text).map_err(|e| e.to_string())
}

/// `truffle meaning folders [add|remove <dir>…]`.
pub fn folders_command(args: &[String]) -> Result<(), String> {
    let mut list = folders();
    match args.first().map(String::as_str) {
        None => {}
        Some(verb @ ("add" | "remove")) => {
            for dir in &args[1..] {
                let path = std::fs::canonicalize(dir).map_err(|e| format!("{dir}: {e}"))?;
                let path = path.to_string_lossy().into_owned();
                list.retain(|f| f != &path);
                if verb == "add" {
                    list.push(path);
                }
            }
            set_folders(&list)?;
        }
        Some(other) => return Err(format!("folders: add or remove, not {other}")),
    }
    for f in &list {
        println!("{f}");
    }
    if list.is_empty() {
        println!("(none: truffle meaning folders add ~/Documents ~/Downloads)");
    }
    Ok(())
}

/// The meaning side of the daemon.
pub struct Meaning {
    roots: Vec<String>,
    /// Chosen folders macOS guards (Desktop, Documents, Downloads…) while truffle lacks Full Disk Access: opening one
    /// from a background process waits on a consent prompt nobody may see, so they're left alone until access is given.
    held: Vec<String>,
    live: OnceLock<Result<Live, String>>,
    indexer: Mutex<Indexer>,
}

#[derive(Default)]
struct Indexer {
    child: Option<(Child, ChildStdin)>,
    /// A sync asked while the child was starting or after it left.
    pending: bool,
    last_sync: Option<Instant>,
}

/// How long changes gather before the indexer is told: a download or an unzip is many events.
const SETTLE: Duration = Duration::from_secs(2);

impl Meaning {
    pub fn start() -> Arc<Meaning> {
        let (roots, held) = readable_roots(folders());
        let meaning = Arc::new(Meaning { roots, held, live: OnceLock::new(), indexer: Mutex::new(Indexer::default()) });
        if !meaning.roots.is_empty() {
            // Catch up with what changed while the daemon was off.
            meaning.ask_sync();
            let m = meaning.clone();
            std::thread::Builder::new().name("truffle-meaning-sync".into()).spawn(move || m.settle_loop()).ok();
        }
        meaning
    }

    fn live(&self) -> Result<&Live, String> {
        self.live
            // What's indexed in a held folder stays searchable: only the indexer keeps out of it.
            .get_or_init(|| Live::open(&paths::index(), [self.roots.clone(), self.held.clone()].concat(), Duration::from_secs(300)))
            .as_ref()
            .map_err(Clone::clone)
    }

    /// A request for the meaning index (`op` without its "meaning." prefix).
    pub fn handle(&self, op: &str, request: &Value) -> Value {
        if op == "folders" {
            return json!({"ok": true, "folders": self.roots, "held": self.held, "full_disk_access": self.held.is_empty()});
        }
        if op == "index" {
            self.ask_sync();
            return json!({"ok": true});
        }
        match self.live() {
            Ok(live) => live.handle(op, request),
            Err(e) => json!({"ok": false, "error": e}),
        }
    }

    /// Folders changed (from the name index's watcher): sync if any is in the meaning folders.
    pub fn changed(&self, dirs: &[(&[u8], bool)]) {
        if self.roots.is_empty() {
            return;
        }
        let hit = dirs.iter().any(|(d, _)| {
            let d = Path::new(std::ffi::OsStr::from_bytes(d));
            self.roots.iter().any(|r| truffle_semantic::rules::change_matters(d, Path::new(r)))
        });
        if hit {
            self.indexer.lock().unwrap().pending = true;
        }
    }

    fn ask_sync(&self) {
        self.indexer.lock().unwrap().pending = true;
    }

    /// Every second: a pending sync that has settled goes to the indexer, started if it isn't running.
    fn settle_loop(self: Arc<Self>) {
        let mut pending_since: Option<Instant> = None;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let mut ix = self.indexer.lock().unwrap();
            if let Some((child, _)) = &mut ix.child
                && !matches!(child.try_wait(), Ok(None))
            {
                ix.child = None;
            }
            if !ix.pending {
                pending_since = None;
                continue;
            }
            let since = *pending_since.get_or_insert_with(Instant::now);
            if since.elapsed() < SETTLE {
                continue;
            }
            pending_since = None;
            ix.pending = false;
            ix.last_sync = Some(Instant::now());
            if let Some((_, stdin)) = &mut ix.child
                && writeln!(stdin, "sync").and_then(|_| stdin.flush()).is_ok()
            {
                continue;
            }
            match self.spawn() {
                Ok(pair) => ix.child = Some(pair),
                Err(e) => eprintln!("{} meaning indexer: {e}", truffle_index::query::now_secs()),
            }
        }
    }

    fn spawn(self: &Arc<Self>) -> Result<(Child, ChildStdin), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut command = Command::new(exe);
        command.args(["meaning", "index-service", "--backfill", "--linger", "60"]);
        // Pruning drops files under none of the folders given: only when the indexer is given all of them.
        if self.held.is_empty() {
            command.arg("--prune");
        }
        let mut child = command
            .args(&self.roots)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let me = self.clone();
        std::thread::Builder::new()
            .name("truffle-meaning-events".into())
            .spawn(move || me.follow(BufReader::new(stdout)))
            .map_err(|e| e.to_string())?;
        Ok((child, stdin))
    }

    /// The indexer's events: what it indexed becomes searchable as it goes (a reload every few seconds of work).
    fn follow(&self, out: BufReader<std::process::ChildStdout>) {
        let mut last_reload = Instant::now();
        let mut unseen = false;
        // Every event, for `truffle meaning log` and for whoever wonders what the indexer is doing.
        let _ = std::fs::create_dir_all(paths::logs());
        let mut log = std::fs::OpenOptions::new().create(true).append(true).open(paths::logs().join("indexer.log")).ok();
        for line in out.lines().map_while(Result::ok) {
            if let Some(f) = &mut log {
                let _ = writeln!(f, "{} {line}", truffle_index::query::now_secs());
            }
            let Ok(event) = serde_json::from_str::<Value>(&line) else { continue };
            match event["event"].as_str() {
                Some("file" | "rest") => unseen = true,
                Some("error") => eprintln!("{} meaning indexer: {}", truffle_index::query::now_secs(), event["message"]),
                _ => {}
            }
            let done = matches!(event["event"].as_str(), Some("summary" | "idle" | "waiting" | "exit"));
            if unseen && (done || last_reload.elapsed() > Duration::from_secs(5)) {
                if let Some(Ok(live)) = self.live.get() {
                    live.handle("reload", &json!({}));
                }
                unseen = false;
                last_reload = Instant::now();
            }
        }
    }
}

/// The chosen folders split into those the indexer may open now and those it must not (consent-gated, without Full
/// Disk Access). Linux has no such gate.
fn readable_roots(all: Vec<String>) -> (Vec<String>, Vec<String>) {
    if truffle_index::sys::has_full_disk_access() {
        return (all, Vec::new());
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let gated: Vec<Vec<u8>> = truffle_index::gated(&home);
    all.into_iter().partition(|root| {
        let r = root.as_bytes();
        !gated.iter().any(|g| r == g.as_slice() || r.starts_with(g) && r.get(g.len()) == Some(&b'/') || g.starts_with(r) && g.get(r.len()) == Some(&b'/'))
    })
}

