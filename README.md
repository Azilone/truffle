# Truffle

**Find any file by name, content or meaning. Instantly, offline.**

One Rust binary, `truffle`, for macOS and Linux:

- **Names**: fuzzy and typo tolerant (`recete` finds `recette.md`), in about 0.1 ms.
- **Contents**: indexed grep (`grep:chocolat`, `regex:fn\s+\w+`, `sym:apply_dir`).
- **Live**: a new, renamed or deleted file shows in about 1 ms.
- **Meaning** *(in progress)*: photos, PDFs, documents, audio and video by what they show or say,
  with Google DeepMind's EmbeddingGemma 2 running on your machine.

It runs as a small daemon (about 10–20 MB of RAM) with a JSON-lines socket, an MCP server for coding
agents, and a Raycast extension on top.

```bash
cargo build --release && ./target/release/truffle install --login
truffle 'readme in:~/Code'
truffle 'ext:pdf facture'
truffle 'grep:TODO in:~/Code/app'
```

| | macOS | Linux |
|---|---|---|
| Crawl | `getattrlistbulk` | `readdir` + `fstatat`, disk filesystems only |
| Live updates | FSEvents (replayed across restarts) | inotify on home; relist by mtime at start |
| Login service | LaunchAgent | systemd user service |
| SIMD | NEON | NEON (arm64), SSE2 (x86_64) |

On macOS, give `~/.local/bin/truffle` Full Disk Access, or it skips Desktop, Documents and Downloads.

Truffle started as a fork of [fsearch](https://github.com/noahdunnagan/fsearch); see [NOTICE](NOTICE).
