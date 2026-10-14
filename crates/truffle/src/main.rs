mod frecency;
mod git;
mod mcp;
mod meaning;
mod server;

use truffle_index::{index, live, query};

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::Instant;

/// See `truffle_index::alloc`.
#[global_allocator]
static GLOBAL: truffle_index::alloc::Alloc = truffle_index::alloc::Alloc;

const USAGE: &str = "usage:
  truffle <query...> [--json]   search (starts the daemon if needed)
  truffle stdio                 JSON lines on stdin/stdout
  truffle mcp                   MCP server on stdin/stdout, for coding agents
  truffle serve                 run the daemon in the foreground
  truffle status
  truffle install [--login]     copy to ~/.local/bin; --login also starts the daemon at login
                                (a LaunchAgent on macOS, which needs Full Disk Access granted
                                to ~/.local/bin/truffle; a systemd user service on Linux)
  truffle uninstall             remove the login service (keeps the index)
  truffle bench <query...>      time a query in-process against the saved index
  truffle model download        fetch EmbeddingGemma 2 (865 MB, once) for searching by meaning
  truffle meaning <command>     the meaning index: index, search, status, estimate… (truffle meaning help)";

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/".into())
}

fn data_dir() -> PathBuf {
    let d = truffle_index::default_dir(&home());
    std::fs::create_dir_all(&d).ok();
    d
}

fn main() {
    // Never let a search download iCloud placeholders: opening or listing a
    // dataless file/dir fails fast instead of materializing it.
    // (IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, OFF)
    truffle_index::sys::no_materialize_process();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => eprintln!("{USAGE}"),
        Some("serve") => {
            space_efficient();
            server::serve(data_dir(), home())
        }
        Some("stdio") => stdio(),
        Some("mcp") => mcp::run(data_dir()),
        Some("status") => print_one(&serde_json::json!({"op": "status"}), true),
        Some("bench") => bench(&args[1..].join(" ")),
        Some("install") => install(args.iter().any(|a| a == "--login")),
        Some("meaning") => exit_with(meaning::run(args[1..].to_vec())),
        Some("model") if args.get(1).map(String::as_str) == Some("download") => {
            exit_with(meaning::run([vec!["download-model".to_string()], args[2..].to_vec()].concat()))
        }
        Some("uninstall") => uninstall(),
        Some(_) => {
            let json = args.iter().any(|a| a == "--json");
            let q: Vec<&str> = args.iter().map(String::as_str).filter(|a| *a != "--json").collect();
            // The folder `git:` filters and globs are about, unless `in:` says.
            let cwd = std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
            print_one(&serde_json::json!({"q": q.join(" "), "cwd": cwd}), json);
        }
    }
}

/// macOS's malloc (27, xzone) keeps freed small blocks dirty and its
/// pressure relief does nothing, so after the first run or a compaction the
/// daemon held 150-290 MB it no longer used, until restarted. In
/// space-efficient mode, read only at launch, it hands them back: the daemon
/// idles at 5-15 MB on the M6, for searches ~8% slower. So `serve` restarts
/// itself in it.
fn space_efficient() {
    if std::env::var_os("MallocSpaceEfficient").is_none()
        && let Ok(exe) = std::env::current_exe()
    {
        use std::os::unix::process::CommandExt;
        let e = std::process::Command::new(exe).args(std::env::args_os().skip(1)).env("MallocSpaceEfficient", "1").exec();
        eprintln!("{} re-exec failed: {e}", query::now_secs());
    }
}

fn print_one(req: &serde_json::Value, raw: bool) {
    let mut s = server::connect(&data_dir()).unwrap_or_else(|e| die(&format!("cannot reach daemon: {e}")));
    s.write_all(format!("{req}\n").as_bytes()).unwrap();
    // One request only: the daemon needn't keep the cores warm for a next one.
    let _ = s.shutdown(std::net::Shutdown::Write);
    let mut line = String::new();
    BufReader::new(&s).read_line(&mut line).unwrap();
    if raw {
        print!("{line}");
        return;
    }
    let v: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
    if v["ok"] != true {
        die(v["error"].as_str().unwrap_or("error"));
    }
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    for h in v["hits"].as_array().into_iter().flatten() {
        let _ = writeln!(out, "{}", h["path"].as_str().unwrap_or(""));
    }
    for f in v["files"].as_array().into_iter().flatten() {
        for m in f["matches"].as_array().into_iter().flatten() {
            let _ = writeln!(out, "{}:{}: {}", f["path"].as_str().unwrap_or(""), m["line"], m["text"].as_str().unwrap_or("").trim());
        }
    }
    let _ = out.flush();
}

fn stdio() {
    let s = server::connect(&data_dir()).unwrap_or_else(|e| die(&format!("cannot reach daemon: {e}")));
    let mut up = s.try_clone().unwrap();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if writeln!(up, "{line}").is_err() {
                break;
            }
        }
        let _ = up.shutdown(std::net::Shutdown::Write);
    });
    let mut out = std::io::stdout().lock();
    for line in BufReader::new(s).lines() {
        let Ok(line) = line else { break };
        if writeln!(out, "{line}").and_then(|_| out.flush()).is_err() {
            break;
        }
    }
}

fn bench(qs: &str) {
    let dir = data_dir();
    let idx = index::Index::load(&dir.join("index.bin")).unwrap_or_else(|| die("no index yet; run truffle serve"));
    let live = live::Live::new(idx);
    let home = home();
    let mut q = query::Query::parse(qs, &home).unwrap_or_else(|e| die(&e));
    // Like the daemon: a content search when the query has a pattern, on
    // user-interactive threads, paths included.
    let grep = q.grep.take().map(|p| truffle_index::Grep::new(&p, q.grep_mode).unwrap_or_else(|e| die(&e)));
    let content = grep.as_ref().map(|_| truffle_index::content::Content::open_shared(dir.join("content")));
    let pool = rayon::ThreadPoolBuilder::new()
        .start_handler(|_| {
            truffle_index::sys::set_qos(truffle_index::sys::Qos::Interactive);
            truffle_index::alloc::keep_on_this_thread();
        })
        .build()
        .unwrap();
    let run = || -> Vec<String> {
        let (Some(g), Some(c)) = (&grep, &content) else {
            let mut p = Vec::new();
            let hits = pool.install(|| query::Searcher { live: &live }.search(&q));
            return hits
                .iter()
                .map(|h| {
                    live.base.path(h.idx as usize, &mut p);
                    format!("{:5} {}", h.score, String::from_utf8_lossy(&p))
                })
                .collect();
        };
        let r = if q.scope.as_ref().is_none_or(|s| truffle_index::content::in_scope(s, home.as_bytes())) {
            pool.install(|| c.search(g, &q))
        } else {
            let paths = pool.install(|| truffle_index::content::scan_paths(&live, q.clone_for_scan()));
            truffle_index::content::verify(g, &paths, q.limit)
        };
        let first = |f: &truffle_index::FileMatches| f.lines.first().map_or(String::new(), |(n, t)| format!("{n}: {}", t.trim()));
        r.files.iter().map(|f| format!("{}:{}", String::from_utf8_lossy(&f.path), first(f))).collect()
    };
    // Between runs, another name search: a repeat would be answered from
    // the names cache, which only a repeat of the last query hits.
    let other = query::Query::parse("zzzzzz", &home).unwrap();
    let mut times = Vec::new();
    let mut out = Vec::new();
    for _ in 0..20 {
        pool.install(|| query::Searcher { live: &live }.search(&other));
        let t = Instant::now();
        out = run();
        times.push(t.elapsed());
    }
    for l in out.iter().take(10) {
        println!("{l}");
    }
    let first = times[0];
    times.sort();
    eprintln!("first {first:.2?}  median {:.2?}  min {:.2?}", times[times.len() / 2], times[0]);
}

fn install(login: bool) {
    // A plain reinstall keeps an existing login agent.
    let login = login || service::installed();
    let bin = PathBuf::from(home()).join(".local/bin/truffle");
    // Stop the old daemon so the next one runs the new binary. Stop the login
    // service first, or its restart policy would bring it straight back.
    service::stop();
    server::stop(&data_dir()).unwrap_or_else(|e| die(&e));
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    // Replace, never overwrite in place: a rewritten signed binary at the same
    // path can be SIGKILLed by the code-signing cache. Copy then rename, so
    // reinstalling from the installed copy works too.
    let tmp = bin.with_extension("new");
    std::fs::copy(std::env::current_exe().unwrap(), &tmp).and_then(|_| std::fs::rename(&tmp, &bin)).unwrap_or_else(|e| die(&format!("copy: {e}")));
    let agents =
        format!("for coding agents: claude mcp add -s user truffle -- {0} mcp\n                   codex mcp add truffle -- {0} mcp", bin.display());
    if !login {
        println!("installed {}; the daemon starts on first use\n{agents}", bin.display());
        return;
    }
    service::start(&bin, &data_dir().join("daemon.log"));
    println!("installed {} ({})\n{agents}", bin.display(), service::NAME);
}

fn uninstall() {
    service::remove();
    println!("removed {}; index kept in {}", service::NAME, data_dir().display());
}

/// The daemon as a login item: a LaunchAgent on macOS, a systemd user
/// service on Linux. Both restart it if it dies.
#[cfg(target_os = "macos")]
mod service {
    use super::{die, home};
    use std::path::{Path, PathBuf};

    const LABEL: &str = "dev.truffle.daemon";
    pub const NAME: &str = "LaunchAgent dev.truffle.daemon";

    fn plist_path() -> PathBuf {
        PathBuf::from(home()).join(format!("Library/LaunchAgents/{LABEL}.plist"))
    }

    fn launchctl(args: &[&str]) -> bool {
        std::process::Command::new("launchctl").args(args).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
    }

    fn domain() -> String {
        format!("gui/{}", unsafe { libc::getuid() })
    }

    pub fn installed() -> bool {
        plist_path().exists()
    }

    pub fn stop() {
        launchctl(&["bootout", &format!("{}/{LABEL}", domain())]);
    }

    pub fn start(bin: &Path, log: &Path) {
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key><array><string>{}</string><string>serve</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>{}</string>
  <key>StandardErrorPath</key><string>{}</string>
</dict>
</plist>
"#,
            bin.display(),
            log.display(),
            log.display()
        );
        std::fs::write(plist_path(), plist).unwrap();
        // bootout returns before the old job is fully gone; bootstrap fails
        // until it is.
        let bootstrap = || launchctl(&["bootstrap", &domain(), plist_path().to_str().unwrap()]);
        let retry = || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            false
        };
        if !(0..50).any(|_| bootstrap() || retry()) {
            die("launchctl bootstrap failed");
        }
    }

    pub fn remove() {
        stop();
        let _ = std::fs::remove_file(plist_path());
    }
}

#[cfg(target_os = "linux")]
mod service {
    use super::{die, home};
    use std::path::{Path, PathBuf};

    pub const NAME: &str = "systemd user service truffle.service";

    fn unit_path() -> PathBuf {
        let config = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map_or_else(|| PathBuf::from(home()).join(".config"), PathBuf::from);
        config.join("systemd/user/truffle.service")
    }

    fn systemctl(args: &[&str]) -> bool {
        std::process::Command::new("systemctl").arg("--user").args(args).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
    }

    pub fn installed() -> bool {
        unit_path().exists()
    }

    pub fn stop() {
        if installed() {
            systemctl(&["stop", "truffle.service"]);
        }
    }

    pub fn start(bin: &Path, log: &Path) {
        let unit = format!(
            "[Unit]\nDescription=Truffle file index\n\n[Service]\nExecStart={} serve\nRestart=on-failure\nNice=5\nStandardOutput=append:{}\nStandardError=append:{}\n\n[Install]\nWantedBy=default.target\n",
            bin.display(),
            log.display(),
            log.display()
        );
        let path = unit_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, unit).unwrap();
        if !(systemctl(&["daemon-reload"]) && systemctl(&["enable", "--now", "truffle.service"])) {
            die("systemctl --user enable failed (no user systemd?); run `truffle serve` from your session instead");
        }
    }

    pub fn remove() {
        systemctl(&["disable", "--now", "truffle.service"]);
        let _ = std::fs::remove_file(unit_path());
        systemctl(&["daemon-reload"]);
    }
}

/// Leave with a meaning command's status.
fn exit_with(code: std::process::ExitCode) {
    if code != std::process::ExitCode::SUCCESS {
        std::process::exit(1);
    }
}

fn die(msg: &str) -> ! {
    eprintln!("truffle: {msg}");
    std::process::exit(1)
}
