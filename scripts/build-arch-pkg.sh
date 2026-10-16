#!/usr/bin/env bash
# Builds an Arch Linux package (release/<name>-<ver>-1-x86_64.pkg.tar.zst).
# Install with: sudo pacman -U release/*.pkg.tar.zst
set -euo pipefail

cd "$(dirname "$0")/.."

NAME=multi-browser-manager
VERSION=$(node -p "require('./package.json').version")
PKGREL=1
OUT_DIR=release
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

npm ci
npx tauri build --no-bundle

BIN=src-tauri/target/release/$NAME
install -Dm755 "$BIN" "$STAGE/usr/bin/$NAME"
ln -s "$NAME" "$STAGE/usr/bin/mbm"
install -Dm644 packaging/$NAME.desktop "$STAGE/usr/share/applications/$NAME.desktop"
install -Dm644 src-tauri/icons/icon.svg "$STAGE/usr/share/icons/hicolor/scalable/apps/$NAME.svg"
install -Dm644 src-tauri/icons/128x128.png "$STAGE/usr/share/icons/hicolor/128x128/apps/$NAME.png"
install -Dm644 docs/niri-keybinds.kdl "$STAGE/usr/share/doc/$NAME/niri-keybinds.kdl"
install -Dm644 README.md "$STAGE/usr/share/doc/$NAME/README.md"

INSTALLED_KB=$(du -sk "$STAGE" | cut -f1)
cat > "$STAGE/.PKGINFO" <<INFO
pkgname = $NAME
pkgbase = $NAME
pkgver = $VERSION-$PKGREL
pkgdesc = Desktop app to manage multiple isolated Chromium browser profiles from one dashboard
url = https://github.com/wilisssss/multi-browser-manager
builddate = $(date +%s)
packager = wilisssss
size = $((INSTALLED_KB * 1024))
arch = x86_64
license = custom
depend = webkit2gtk-4.1
depend = gtk3
depend = libayatana-appindicator
depend = dbus
optdepend = chromium: default browser untuk profile
optdepend = brave-bin: Brave (AUR)
optdepend = google-chrome: Google Chrome (AUR)
optdepend = gnome-keyring: penyimpanan password proxy (Secret Service)
INFO

mkdir -p "$OUT_DIR"
PKG="$OUT_DIR/$NAME-$VERSION-$PKGREL-x86_64.pkg.tar.zst"

# Hapus paket lama (semua varian versi) supaya tidak ada file build basi yang
# bisa salah dipasang — hasil di folder ini selalu build terbaru saja.
rm -f "$OUT_DIR"/"$NAME"-*-x86_64.pkg.tar.zst

tar -C "$STAGE" --owner=0 --group=0 --numeric-owner \
  --use-compress-program="zstd -19 -T0" -cf "$PKG" .PKGINFO usr
echo "Built $PKG"
