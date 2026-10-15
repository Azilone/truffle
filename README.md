# Truffle

**Find any file by name, content or meaning. Instantly, offline.**

One Rust binary, `truffle`, for macOS and Linux:

- **Names**: fuzzy and typo tolerant (`recete` finds `recette.md`), in about 0.1 ms.
- **Contents**: indexed grep (`grep:chocolat`, `regex:fn\s+\w+`, `sym:apply_dir`).
- **Meaning**: photos, screenshots, PDF pages, documents, audio and video moments, by what they show or say
  ("the clause about pets in the lease", "a dog on the beach", "where they talk about pricing"), with Google
  DeepMind's EmbeddingGemma 2 running on your machine. In any of 100+ languages, across languages.
- **Live**: a new file shows by name in ~1 ms and by meaning a few seconds later.

It runs as a small daemon with a JSON-lines socket, an MCP server for coding agents, and a Raycast extension.

```bash
cargo build --release && ./target/release/truffle install --login
truffle model download                                   # 865 MB, once
truffle meaning folders add ~/Documents ~/Downloads ~/Desktop
truffle 'facture'                                         # names
truffle 'grep:TODO in:~/Code/app'                         # contents
truffle meaning search 'relevé de compte bancaire'        # meaning
```

On macOS, give `~/.local/bin/truffle` Full Disk Access (System Settings › Privacy & Security), or it leaves Desktop,
Documents and Downloads alone rather than wait on a consent prompt.

## Measured (M1 Pro, macOS 27; 1.7 M files on disk, 1,924 files searched by meaning)

| | |
|---|---|
| Name search | 0.06–0.15 ms |
| Meaning search, warm | ~30 ms (18 ms to embed the query, 15 ms over 2,900 vectors); 23 ms at 100k vectors |
| First meaning search | ~1 s to load the text model (7 s the very first time: Metal compiles its shaders) |
| New file searchable by meaning | ~4 s |
| Indexing 1,924 files (photos, PDFs, docs, audio, video) | 7 min 48 s (DigUp, same files: 17 min) |
| Daemon memory | 49 MB idle; 131 MB footprint with the model loaded, released after 5 idle minutes |
| Disk | 33 MB binary; indexes 42 MB (names) + 17 MB (contents) + 14 MB (meaning); model 865 MB |
| Vectors vs DigUp's reference | cosine 1.00000 (Metal), ≥ 0.9997 (Linux CPU) |
| Ranking on DigUp's evals (R@1 / MRR) | synthetic 0.95 / 0.97, multilingual 0.85 / 0.90, code 1.00 / 1.00: DigUp's own scores |

## How it works

```
crates/
  truffle-index     names + contents: a fork of fsearch, made portable
                    macOS: getattrlistbulk crawl, FSEvents (replayed across restarts)
                    Linux: readdir + fstatat crawl (disk filesystems only), inotify on home, mtime relist at start
                    SIMD: NEON on arm64, SSE2 on x86_64 (parity-tested against byte-by-byte versions)
  truffle-embed     EmbeddingGemma 2 on llama.cpp b11461 (DigUp's pin and no-logits patch), built statically by
                    build.rs: Metal on macOS, AVX2/NEON CPU on Linux (--features vulkan|cuda for a GPU)
  truffle-semantic  the meaning index: SQLite + FTS5, float16 vectors, brute-force cosine (AVX2/NEON) with
                    DigUp's z-score gate fused with exact words
  truffle-extract   readers: PDF in pure Rust (pdf_oxide + tiny-skia), docx/odt/rtf/doc/html, EXIF, SVG;
                    macOS: ImageIO, AVFoundation, Vision OCR; Linux: image, symphonia, ffmpeg for video
  truffle           the binary: CLI, daemon, MCP server, indexer
raycast/            the Raycast extension (TypeScript, as Raycast requires): a client of the daemon's socket
```

One daemon, one watcher: the name index's file events also tell the meaning indexer what changed (ignoring hidden,
dependency, build and code-project folders). Searches by meaning run inside the daemon; indexing runs in a child
process that exits when it's done, so the image and audio encoders' memory goes back to the system.

## Status

- macOS (Apple Silicon) and Linux (x86_64, arm64): built and tested. Windows: not yet.
- Linux reads no text in pictures yet (no OCR), and indexes video frames only with `ffmpeg` installed.
- Releases (cargo-dist, Homebrew tap) and the Raycast Store listing are next.

## Credits

Truffle stands on [fsearch](https://github.com/noahdunnagan/fsearch) (names and contents),
[DigUp](https://github.com/ARahim3/DigUp) (the meaning index's design and rules),
[trovara](https://github.com/Subhankar-Patra1/trovara) (their Rust port), and llama.cpp. See [NOTICE](NOTICE).
