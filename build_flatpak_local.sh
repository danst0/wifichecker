#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# build_flatpak_local.sh
# Builds the Flatpak from local source and installs it for the current user.
#
# Usage:  ./build_flatpak_local.sh [--run]
#   --run   Launch the app automatically after installing
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_ID="io.github.danst0.wifichecker"
BUILD_DIR="$SCRIPT_DIR/.flatpak-build"
REPO_DIR="$SCRIPT_DIR/.flatpak-repo"
MANIFEST="$APP_ID.yml"
LOCAL_MANIFEST="$SCRIPT_DIR/$APP_ID-local.yml"
RUN_AFTER=false

for arg in "$@"; do
    [[ "$arg" == "--run" ]] && RUN_AFTER=true
done

# ── 1. Create a local manifest (replaces git source with current dir) ────────
echo "▶  Preparing local manifest …"
python3 - <<PYEOF
import yaml, sys

with open("$SCRIPT_DIR/$MANIFEST") as f:
    manifest = yaml.safe_load(f)

for module in manifest.get("modules", []):
    if isinstance(module, dict) and module.get("name") == "wifichecker":
        module["sources"] = [
            {"type": "dir", "path": "$SCRIPT_DIR"},
            "cargo-sources.json",
        ]

with open("$LOCAL_MANIFEST", "w") as f:
    yaml.dump(manifest, f, default_flow_style=False, allow_unicode=True)
PYEOF

# ── 2. Generate cargo-sources.json ──────────────────────────────────────────
echo "▶  Generating cargo-sources.json …"
GENERATOR="$(mktemp /tmp/flatpak-cargo-generator.XXXXXX.py)"
curl -sLo "$GENERATOR" \
    https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
python3 "$GENERATOR" "$SCRIPT_DIR/Cargo.lock" -o "$SCRIPT_DIR/cargo-sources.json"
rm "$GENERATOR"

# ── 3. Build ─────────────────────────────────────────────────────────────────
echo "▶  Building (this will take a while on first run) …"
flatpak-builder \
    --user \
    --install \
    --install-deps-from=flathub \
    --force-clean \
    --state-dir="$BUILD_DIR/state" \
    --repo="$REPO_DIR" \
    "$BUILD_DIR/build" \
    "$LOCAL_MANIFEST"

echo ""
echo "✓  Installed $APP_ID"
echo "   Run with:  flatpak run $APP_ID"

# ── 5. Optionally launch ─────────────────────────────────────────────────────
if $RUN_AFTER; then
    echo "▶  Launching …"
    flatpak run "$APP_ID"
fi
