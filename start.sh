#!/usr/bin/env bash
# IronBullet Linux Launcher
# Run this script instead of ./ironbullet directly.
# It checks for required dependencies and shows a helpful error if anything is missing.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$SCRIPT_DIR/ironbullet"

if [[ ! -f "$BINARY" ]]; then
    echo "ERROR: ironbullet binary not found at $BINARY"
    exit 1
fi

# ── WebKitGTK check ──────────────────────────────────────────────────────────
# IronBullet requires WebKitGTK for its embedded web renderer.
# If not installed, the binary will crash with a dynamic linker error.

webkitgtk_found=0

# 1. Check via ldconfig (reliable on most distros)
if ldconfig -p 2>/dev/null | grep -q "libwebkit2gtk"; then
    webkitgtk_found=1
fi

# 2. Check common library paths directly
if [[ $webkitgtk_found -eq 0 ]]; then
    for lib in \
        /usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0 \
        /usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.0.so.37 \
        /usr/lib64/libwebkit2gtk-4.1.so.0 \
        /usr/lib64/libwebkit2gtk-4.0.so.37 \
        /usr/lib/libwebkit2gtk-4.1.so.0 \
        /usr/lib/libwebkit2gtk-4.0.so.37
    do
        if [[ -f "$lib" ]]; then
            webkitgtk_found=1
            break
        fi
    done
fi

if [[ $webkitgtk_found -eq 0 ]]; then
    MSG="IronBullet requires WebKitGTK but it is not installed.

Option 1 — Use the AppImage, if one is published for this release:
  Download the matching ironbullet-vX.Y.Z-linux-x64.AppImage from the release page,
  chmod +x it, and run it directly.

Option 2 — Install WebKitGTK for your distribution:

  Ubuntu / Debian (22.04+):  sudo apt install libwebkit2gtk-4.1-0
  Ubuntu / Debian (20.04):   sudo apt install libwebkit2gtk-4.0-37
  Fedora / RHEL:             sudo dnf install webkit2gtk4.1
  Arch Linux:                sudo pacman -S webkit2gtk-4.1
  openSUSE:                  sudo zypper install libwebkit2gtk-4_1-0

After installing, run this script again."

    echo "ERROR: $MSG"
    echo ""

    # Try a GUI dialog — best effort
    if command -v zenity &>/dev/null 2>&1; then
        zenity --error --title="IronBullet — Missing Dependency" \
               --text="$MSG" --width=500 2>/dev/null || true
    elif command -v kdialog &>/dev/null 2>&1; then
        kdialog --error "$MSG" --title "IronBullet — Missing Dependency" 2>/dev/null || true
    elif command -v xmessage &>/dev/null 2>&1; then
        xmessage -center "$MSG" 2>/dev/null || true
    fi

    exit 1
fi

# ── reqflow-sidecar check ────────────────────────────────────────────────────
SIDECAR="$SCRIPT_DIR/reqflow-sidecar"
if [[ ! -f "$SIDECAR" ]]; then
    echo "WARNING: reqflow-sidecar not found at $SIDECAR"
    echo "AzureTLS HTTP requests will not work. RustTLS requests are still available."
    echo "Download the full release zip and ensure reqflow-sidecar is in the same folder."
fi

# ── Make binaries executable ─────────────────────────────────────────────────
chmod +x "$BINARY" 2>/dev/null || true
[[ -f "$SIDECAR" ]] && chmod +x "$SIDECAR" 2>/dev/null || true

# ── Linux WebView env vars ───────────────────────────────────────────────────
# Prefer native Wayland when it is available, while retaining X11 as a fallback.
# Do not force WebKit compositing/DMABUF workarounds globally: those settings
# disable normal Wayland rendering and should be set only by users troubleshooting
# a specific NVIDIA/driver issue.
export GDK_BACKEND="${GDK_BACKEND:-wayland,x11}"

# ── Launch IronBullet ────────────────────────────────────────────────────────
exec "$BINARY" "$@"
