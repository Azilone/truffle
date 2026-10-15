#!/bin/bash
# End-to-end check on Linux, in a container: index pictures, documents, audio and video by meaning, search them, then
# let the daemon pick up a new file. Needs the model and DigUp's testbed (scripts/make-testbed.sh in DigUp):
#   docker run --rm -v "$PWD":/src -v truffle-target:/target -e CARGO_TARGET_DIR=/target -w /src \
#     -v <models dir>:/models:ro -v ~/DigUpTestbed:/testbed:ro rust:1 bash scripts/e2e-linux.sh
set -u
apt-get update -qq >/dev/null; apt-get install -y -qq cmake ffmpeg python3 >/dev/null 2>&1
CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2} cargo build --release -p truffle-search 2>&1 | grep -E "^error" -A5
T=${CARGO_TARGET_DIR:-target}/release/truffle
export TRUFFLE_MODELS=/models
mkdir -p /root/files && cp -r /testbed/synthetic/img /testbed/synthetic/docs /testbed/synthetic/audio /testbed/synthetic/video /root/files/
t0=$(date +%s); $T meaning index /root/files 2>&1 | tail -2; echo "index: $(( $(date +%s)-t0 )) s"
for q in "a penguin" "a zebra in a video" "rainy weather" "invoice"; do
  echo "== $q"; $T meaning search "$q" -n 3 2>&1 | grep -E '^ *[0-9]\.' | cut -c1-110
done
echo "== daemon"
$T meaning folders add /root/files >/dev/null
($T serve > /tmp/daemon.log 2>&1 &)
for i in $(seq 1 60); do $T status 2>/dev/null | grep -q '"ok":true' && break; sleep 1; done
sleep 5
cp "/root/files/img/$(ls /root/files/img | head -1)" /root/files/brand-new.jpg
t0=$(date +%s)
for i in $(seq 1 120); do
  grep -q '"event":"file"' ~/.local/share/truffle/logs/indexer.log 2>/dev/null && { echo "new file indexed after $(( $(date +%s)-t0 )) s"; break; }
  sleep 1
done
python3 - <<'PY'
import socket, json, os
s = socket.socket(socket.AF_UNIX); s.connect(os.path.expanduser("~/.local/share/truffle/truffle.sock")); f = s.makefile("rw")
for i, r in enumerate([{"op": "meaning.folders"}, {"op": "meaning.prewarm"}, {"op": "meaning.search", "q": "a penguin", "limit": 3}, {"q": "brand-new", "limit": 2}]):
    r["id"] = i; f.write(json.dumps(r) + "\n"); f.flush(); a = json.loads(f.readline())
    if "hits" in a: a["hits"] = [os.path.basename(h["path"]) for h in a["hits"]]
    print(json.dumps(a)[:220])
PY
ps -o rss,cmd -C truffle | head -3
