#!/usr/bin/env sh
# Installe Xion pour l'utilisateur courant : binaire, icônes, entrée de bureau.
#
# Rien n'est écrit hors de $HOME, donc aucun sudo. Passer --uninstall retire
# exactement ce que cette installation a posé.
#
#   ./scripts/install.sh              installe (compile en release au passage)
#   ./scripts/install.sh --uninstall  retire

set -eu

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor"
ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"

# Les tailles où l'orbite deviendrait une bouillie de deux pixels reçoivent le
# dessin simplifié : c'est ce que fait Adwaita, un dessin par taille plutôt
# qu'une réduction mécanique du grand.
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
    # Sans ça, l'entrée n'apparaît dans aucun menu avant la prochaine session,
    # et « Ouvrir avec » ne propose pas Xion.
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
install -m 644 "$ROOT/assets/xion.svg" "$ICON_DIR/scalable/apps/xion.svg"
for size in $LARGE_SIZES; do
    mkdir -p "$ICON_DIR/${size}x${size}/apps"
    rsvg-convert -w "$size" -h "$size" "$ROOT/assets/xion.svg" \
        -o "$ICON_DIR/${size}x${size}/apps/xion.png"
done
for size in $SMALL_SIZES; do
    mkdir -p "$ICON_DIR/${size}x${size}/apps"
    rsvg-convert -w "$size" -h "$size" "$ROOT/assets/xion-small.svg" \
        -o "$ICON_DIR/${size}x${size}/apps/xion.png"
done

echo "→ entrée de bureau"
mkdir -p "$APP_DIR"
install -m 644 "$ROOT/assets/xion.desktop" "$APP_DIR/xion.desktop"

refresh_caches

echo
echo "Installé dans $PREFIX."
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "Attention : $BIN_DIR n'est pas dans votre PATH." ;;
esac
echo "Pour en faire le gestionnaire de fichiers par défaut :"
echo "    xdg-mime default xion.desktop inode/directory"
