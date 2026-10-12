#!/usr/bin/env sh
# Install Xion for the current user: binary, icons, desktop entry.
#
# Nothing is written outside $HOME, so no sudo. Passing --uninstall removes
# exactly what this installation put in place.
#
#   ./scripts/install.sh              install (builds in release on the way)
#   ./scripts/install.sh --uninstall  remove

set -eu

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor"
ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"

# The sizes where the orbit would become a two-pixel smudge get the simplified
# drawing: that is what Adwaita does, one drawing per size rather than a
# mechanical reduction of the large one.
SMALL_SIZES="16 22 24"
LARGE_SIZES="32 48 64 128 256"

uninstall() {
    rm -f "$BIN_DIR/xion" "$APP_DIR/xion.desktop"
    rm -f "$ICON_DIR/scalable/apps/xion.svg"
    for size in $SMALL_SIZES $LARGE_SIZES; do
        rm -f "$ICON_DIR/${size}x${size}/apps/xion.png"
    done
    echo "Xion retiré de $PREFIX"
}

refresh_caches() {
    # Without this, the entry appears in no menu before the next session, and
    # "Open with" does not offer Xion.
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$APP_DIR" 2>/dev/null || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
    fi
}

if [ "${1:-}" = "--uninstall" ]; then
    uninstall
    refresh_caches
    exit 0
fi

if ! command -v rsvg-convert >/dev/null 2>&1; then
    echo "rsvg-convert est requis pour produire les icônes PNG." >&2
    echo "  Arch : pacman -S librsvg    Debian : apt install librsvg2-bin" >&2
    exit 1
fi

echo "→ compilation"
( cd "$ROOT" && cargo build --release )

echo "→ binaire"
mkdir -p "$BIN_DIR"
install -m 755 "$ROOT/target/release/xion" "$BIN_DIR/xion"

echo "→ icônes"
mkdir -p "$ICON_DIR/scalable/apps"
install -m 644 "$ROOT/assets/brand/xion.svg" "$ICON_DIR/scalable/apps/xion.svg"
for size in $LARGE_SIZES; do
    mkdir -p "$ICON_DIR/${size}x${size}/apps"
    rsvg-convert -w "$size" -h "$size" "$ROOT/assets/brand/xion.svg" \
        -o "$ICON_DIR/${size}x${size}/apps/xion.png"
done
for size in $SMALL_SIZES; do
    mkdir -p "$ICON_DIR/${size}x${size}/apps"
    rsvg-convert -w "$size" -h "$size" "$ROOT/assets/brand/xion-small.svg" \
        -o "$ICON_DIR/${size}x${size}/apps/xion.png"
done

echo "→ entrée de bureau"
mkdir -p "$APP_DIR"
install -m 644 "$ROOT/packaging/xion.desktop" "$APP_DIR/xion.desktop"

refresh_caches

echo
echo "Installé dans $PREFIX."
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "Attention : $BIN_DIR n'est pas dans votre PATH." ;;
esac
echo "Pour en faire le gestionnaire de fichiers par défaut :"
echo "    xdg-mime default xion.desktop inode/directory"
