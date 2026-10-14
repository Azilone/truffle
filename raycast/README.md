# Dig — fsearch + DigUp in Raycast

One Raycast command, **Dig Files**, that searches your Mac three ways at once:

- **by name**, typo-tolerant, in ~1 ms ([fsearch](https://github.com/noahdunnagan/fsearch))
- **inside files**, indexed grep: `grep:word`, `regex:…`, `sym:name` (fsearch)
- **by meaning**: photos, screenshots, PDF pages, audio and video moments, documents, and code with `code: …`
  ([DigUp](https://github.com/ARahim3/DigUp), EmbeddingGemma 2, all on-device)

A file both engines find shows once, as DigUp's row, since DigUp knows the page or the moment.
A sentence ("a dog on the beach") puts meaning first; a word or two ("invoice.pdf") puts names first.
fsearch filters (`ext:`, `in:`, `type:`, `'exact`, globs…) send the query to fsearch alone.

| Key | Action |
| --- | --- |
| ↩ | Open: audio and video at the moment, in QuickTime Player |
| ⌘↩ | Show in Finder |
| ⌘Y | Quick Look |
| ⌘D | Preview pane: PDF page, video frame, passage or matching lines |
| ⌘F | Search in this file's folder |
| ⌘⇧C / ⌘. | Copy the file / its path |

## Setup

```bash
# fsearch (Rust)
git clone https://github.com/noahdunnagan/fsearch && cd fsearch && cargo build --release && ./target/release/fsearch install
# DigUp (app + model, then pick folders in the app)
brew install --cask arahim3/tap/digup
# this extension
npm install && npm run dev
```

**Full Disk Access:** without it fsearch skips Desktop, Documents and Downloads. Give `~/.local/bin/fsearch`
Full Disk Access in System Settings › Privacy & Security, then use "Restart fsearch" in the Setup section.

## How it works

- fsearch already runs a daemon; the extension talks JSON lines to its socket
  (`~/Library/Application Support/FSearch/fsearch.sock`).
- DigUp's `digup search-service` speaks JSON lines on stdin only, and Raycast unloads a command when its window
  closes. `assets/digup-bridge.cjs` keeps one search-service alive behind a Unix socket, so the index stays open
  and the query encoder warm between opens (it quits after 20 idle minutes). While typing, DigUp answers with
  exact words at once and embeds ahead; the meaning search runs once typing pauses (280 ms).
- Default fsearch filters (a preference) keep caches and `~/Library` out: `in:~ !Library/ !.*/ !node_modules/ …`.
  A query with its own `in:` skips them.

## Checks

```bash
npm run typecheck
npm test
```

## Known limits

- Apple Silicon only (both tools).
- DigUp's index follows new files only while the DigUp app runs.
- On macOS 27 the first Vision OCR pass compiles its language models for several minutes before indexing moves.
