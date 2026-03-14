#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# publish_flatpak.sh
# Updates the Flathub submission repo with the latest tagged release and
# pushes it so Flathub CI picks up the new version.
#
# Usage:  ./publish_flatpak.sh [path/to/flathub-wifichecker]
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FLATHUB_REPO="${1:-/home/danst/Skripte/flathub-wifichecker}"
APP_ID="io.github.danst0.wifichecker"
MANIFEST="$APP_ID.yml"

# ── 1. Resolve version & tag ────────────────────────────────────────────────
VERSION="$(cat "$SCRIPT_DIR/VERSION" | tr -d '[:space:]')"
TAG="v$VERSION"

echo "▶  Publishing $APP_ID $TAG"

# Verify the tag exists
if ! git -C "$SCRIPT_DIR" rev-parse "$TAG" &>/dev/null; then
    echo "✗  Tag $TAG not found in source repo. Did you forget to tag and push?"
    exit 1
fi

COMMIT_SHA="$(git -C "$SCRIPT_DIR" rev-list -n1 "$TAG")"
echo "   Commit: $COMMIT_SHA"

# ── 2. Regenerate cargo-sources.json ────────────────────────────────────────
echo "▶  Regenerating cargo-sources.json …"
pip3 install --quiet tomlkit
GENERATOR="$(mktemp /tmp/flatpak-cargo-generator.XXXXXX.py)"
curl -sLo "$GENERATOR" \
    https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
python3 "$GENERATOR" "$SCRIPT_DIR/Cargo.lock" -o "$SCRIPT_DIR/cargo-sources.json"
rm "$GENERATOR"
echo "   Done."

# ── 3. Sync flathub repo ─────────────────────────────────────────────────────
echo "▶  Syncing $FLATHUB_REPO …"
if [[ ! -d "$FLATHUB_REPO/.git" ]]; then
    echo "✗  $FLATHUB_REPO is not a git repo. Clone your Flathub fork first."
    exit 1
fi

git -C "$FLATHUB_REPO" pull --rebase origin "$APP_ID"

# Copy manifest and cargo sources
cp "$SCRIPT_DIR/$MANIFEST"          "$FLATHUB_REPO/$MANIFEST"
cp "$SCRIPT_DIR/cargo-sources.json" "$FLATHUB_REPO/cargo-sources.json"

# ── 4. Update tag + commit in manifest ──────────────────────────────────────
echo "▶  Updating manifest tag → $TAG, commit → $COMMIT_SHA"
sed -i \
    -e "s|tag: v[0-9]\+\.[0-9]\+\.[0-9]\+|tag: $TAG|" \
    -e "s|commit: [0-9a-f]\{40\}|commit: $COMMIT_SHA|" \
    "$FLATHUB_REPO/$MANIFEST"

# ── 5. Lint ──────────────────────────────────────────────────────────────────
echo "▶  Running flatpak-builder-lint …"
if flatpak run --command=flatpak-builder-lint org.flatpak.Builder \
        manifest "$FLATHUB_REPO/$MANIFEST" 2>&1; then
    echo "   ✓ Lint passed."
else
    echo "✗  Lint failed — aborting push."
    exit 1
fi

# ── 6. Commit & push ─────────────────────────────────────────────────────────
echo "▶  Committing …"
git -C "$FLATHUB_REPO" add "$MANIFEST" cargo-sources.json
git -C "$FLATHUB_REPO" diff --cached --quiet && {
    echo "   Nothing changed, already up to date."
    exit 0
}
git -C "$FLATHUB_REPO" commit -m "Update to $TAG"

echo "▶  Pushing …"
git -C "$FLATHUB_REPO" push origin "$APP_ID"

echo ""
echo "✓  Done. Flathub CI will now rebuild $APP_ID $TAG."
