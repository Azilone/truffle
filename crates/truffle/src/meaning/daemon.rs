//! The meaning index inside the daemon. Searches run in the daemon (`Live`); indexing runs in a child process
//! (`truffle meaning index-service`), started when the folders to search change and gone once it's done, so the
//! image and audio encoders' memory goes back to the system. The name index's watcher tells it what changed: one
//! watcher for both indexes.
//!
//! The folders to search are in `<data>/meaning-folders`, one per line (`truffle meaning folders add <dir>`).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
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
        let meaning = Arc::new(Meaning { roots: folders(), live: OnceLock::new(), indexer: Mutex::new(Indexer::default()) });
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
            .get_or_init(|| Live::open(&paths::index(), self.roots.clone(), Duration::from_secs(300)))
            .as_ref()
            .map_err(Clone::clone)
    }

    /// A request for the meaning index (`op` without its "meaning." prefix).
    pub fn handle(&self, op: &str, request: &Value) -> Value {
        if op == "folders" {
            return json!({"ok": true, "folders": self.roots});
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
            let d = String::from_utf8_lossy(d);
            self.roots.iter().any(|r| d == r.as_str() || d.starts_with(&format!("{}/", r.trim_end_matches('/'))))
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
        let mut child = Command::new(exe)
            .args(["meaning", "index-service", "--backfill", "--prune", "--linger", "60"])
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
        for line in out.lines().map_while(Result::ok) {
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

/// Where the daemon's meaning index lives, for `status`.
pub fn index_dir() -> PathBuf {
    paths::index()
}

#[allow(dead_code)]
fn _paths(_: &Path) {}
