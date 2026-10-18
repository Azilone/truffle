<div align="center">

<img src="assets/logo.png" alt="Truffle" width="180">

# Truffle

### Find any file by name, content or meaning.<br>Instantly. Offline.

One Rust binary for macOS and Linux. Free and open source.

[![Release](https://img.shields.io/github/v/release/Azilone/truffle?color=b06a3f&label=release)](https://github.com/Azilone/truffle/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-b06a3f)](LICENSE)
![macOS | Linux](https://img.shields.io/badge/macOS%20%7C%20Linux-333)
[![Stars](https://img.shields.io/github/stars/Azilone/truffle?style=flat&color=333)](https://github.com/Azilone/truffle/stargazers)

<table>
  <tr>
    <td align="center" width="200"><h2>33 MB</h2>one binary</td>
    <td align="center" width="200"><h2>50 MB</h2>RAM, idle</td>
    <td align="center" width="200"><h2>&lt; 1 ms</h2>across 3 M files</td>
    <td align="center" width="200"><h2>0</h2>bytes leave your machine</td>
  </tr>
</table>

</div>

## Install

```bash
brew install azilone/tap/truffle
truffle setup
```

`setup` walks you through everything: login service, Full Disk Access, the model (865 MB, once), your folders.
Prefer a binary? Grab it from [Releases](https://github.com/Azilone/truffle/releases).

## Use

```bash
truffle recete                                  # name, typos welcome → recette.md
truffle 'grep:TODO in:~/Code/app'               # inside files
truffle 'regex:fn\s+\w+'                        # regex over contents
truffle meaning search 'a dog on the beach'     # photos, by what they show
truffle meaning search 'the clause about pets in the lease'   # PDFs, by what they say
```

Search by meaning works on photos, screenshots, PDFs, docs, audio and video, in 100+ languages.
New files show up by name in ~1 ms, by meaning a few seconds later.

## Why Truffle

|                          | Spotlight | `fd` / `find` | `ripgrep` | **Truffle** |
|--------------------------|:---------:|:-------------:|:---------:|:-----------:|
| Names, typo tolerant     |     –     |       –       |     –     |    **✓**    |
| Inside files, indexed    |     ✓     |       –       |     –     |    **✓**    |
| By meaning (images, audio, video) |  ~  |     –       |     –     |    **✓**    |
| Linux                    |     –     |       ✓       |     ✓     |    **✓**    |
| 100% offline             |     ✓     |       ✓       |     ✓     |    **✓**    |

## Plug it in

- **Coding agents (MCP):** `claude mcp add truffle -- truffle mcp`
- **Raycast:** the extension lives in [`raycast/`](raycast)
- **Your own tools:** `truffle stdio` speaks JSON lines, `--json` for scripts

<details>
<summary><b>Benchmarks</b> (M1 Pro, 1.7 M files on disk)</summary>

| | |
|---|---|
| Name search | 0.06–0.15 ms |
| Meaning search, warm | ~30 ms; 23 ms at 100k vectors |
| First meaning search | ~1 s to load the model (7 s the very first time: Metal compiles its shaders) |
| New file searchable by meaning | ~4 s |
| Indexing 1,924 files (photos, PDFs, docs, audio, video) | 7 min 48 s (DigUp, same files: 17 min) |
| Daemon memory | 49 MB idle; 131 MB with the model loaded, released after 5 idle minutes |
| Disk | 33 MB binary; indexes 42 MB (names) + 17 MB (contents) + 14 MB (meaning); model 865 MB |
| Ranking on DigUp's evals (R@1 / MRR) | synthetic 0.95 / 0.97, multilingual 0.85 / 0.90, code 1.00 / 1.00 |

</details>

<details>
<summary><b>How it works</b></summary>

One daemon, one file watcher, three indexes. The model is Google DeepMind's EmbeddingGemma 2 on a statically linked
llama.cpp (Metal on macOS, AVX2/NEON on Linux).

```
crates/
  truffle-index     names + contents (fork of fsearch): FSEvents / inotify, NEON + SSE2 SIMD
  truffle-embed     EmbeddingGemma 2 on llama.cpp, built by build.rs (--features vulkan|cuda for a GPU on Linux)
  truffle-semantic  meaning index: SQLite + FTS5, float16 vectors, brute-force cosine
  truffle-extract   readers: PDF (pure Rust), docx/odt/rtf/doc/html, EXIF, SVG, OCR, audio, video
  truffle           the binary: CLI, daemon, MCP server, indexer
raycast/            Raycast extension, a client of the daemon's socket
```

Indexing runs in a child process that exits when done, so the encoders' memory goes back to the system.

</details>

<details>
<summary><b>Status</b></summary>

- macOS (Apple Silicon, Intel) and Linux (x86_64, arm64). Windows: not yet.
- Linux: no OCR yet; video frames need `ffmpeg`.
- macOS without Full Disk Access: Desktop, Documents and Downloads are skipped instead of prompting.

</details>

## Credits

Built on [fsearch](https://github.com/noahdunnagan/fsearch), [DigUp](https://github.com/ARahim3/DigUp),
[trovara](https://github.com/Subhankar-Patra1/trovara) and [llama.cpp](https://github.com/ggml-org/llama.cpp).
See [NOTICE](NOTICE).

<div align="center">

**If Truffle found something for you, a ⭐ helps others find it.**

[![Star History](https://api.star-history.com/svg?repos=Azilone/truffle&type=Date)](https://star-history.com/#Azilone/truffle&Date)

</div>
