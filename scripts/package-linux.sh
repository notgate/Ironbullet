#!/usr/bin/env bash
# Assemble a self-contained Ironbullet Linux release bundle.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:?usage: scripts/package-linux.sh <version>}"
MAIN_BIN="${MAIN_BIN:-$ROOT_DIR/target/release/ironbullet}"
SIDECAR_BIN="${SIDECAR_BIN:-$ROOT_DIR/sidecar/reqflow-sidecar}"
OUT_DIR="${OUT_DIR:-$ROOT_DIR/dist}"
BUNDLE_DIR="$OUT_DIR/ironbullet-v${VERSION}-linux-x64"
ARCHIVE="$OUT_DIR/ironbullet-v${VERSION}-linux-x64.zip"

for required in "$MAIN_BIN" "$SIDECAR_BIN" "$ROOT_DIR/start.sh"; do
    if [[ ! -f "$required" ]]; then
        echo "ERROR: required release input is missing: $required" >&2
        exit 1
    fi
done

rm -rf "$BUNDLE_DIR" "$ARCHIVE"
mkdir -p "$BUNDLE_DIR"
install -m 0755 "$MAIN_BIN" "$BUNDLE_DIR/ironbullet"
install -m 0755 "$SIDECAR_BIN" "$BUNDLE_DIR/reqflow-sidecar"
install -m 0755 "$ROOT_DIR/start.sh" "$BUNDLE_DIR/start.sh"

python3 - "$BUNDLE_DIR" "$VERSION" <<'PY'
import hashlib
import json
import pathlib
import sys

bundle = pathlib.Path(sys.argv[1])
entries = []
for name in ("ironbullet", "reqflow-sidecar", "start.sh"):
    path = bundle / name
    entries.append({
        "path": name,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "executable": bool(path.stat().st_mode & 0o111),
    })
(bundle / "release-manifest.json").write_text(json.dumps({
    "version": sys.argv[2],
    "platform": "linux-x64",
    "files": entries,
}, indent=2) + "\n")
PY

if command -v zip >/dev/null 2>&1; then
    (
        cd "$BUNDLE_DIR"
        zip -q -r "$ARCHIVE" .
    )
else
    python3 - "$BUNDLE_DIR" "$ARCHIVE" <<'PY'
import os
import pathlib
import stat
import sys
import zipfile

bundle = pathlib.Path(sys.argv[1])
archive = pathlib.Path(sys.argv[2])
with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
    for path in sorted(p for p in bundle.rglob("*") if p.is_file()):
        info = zipfile.ZipInfo(path.relative_to(bundle).as_posix())
        info.external_attr = (stat.S_IMODE(path.stat().st_mode) | stat.S_IFREG) << 16
        output.writestr(info, path.read_bytes(), compress_type=zipfile.ZIP_DEFLATED)
PY
fi
printf 'Created %s\n' "$ARCHIVE"
