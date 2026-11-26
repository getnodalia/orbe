#!/usr/bin/env bash
# Package the macOS editor:
#
#   $DIST/orbe-<version>-macos-<arch>.dmg   Orbe.app on a drag-to-Applications DMG
#
# Usage: packaging/macos/package.sh [--arch universal|arm64|x86_64] [--skip-build]
#
# The binaries come from packaging/macos/build.sh (bin/orbe.macos.editor.<arch>); without
# --skip-build this builds them first. Orbe.app is signed ad hoc: there is no Developer ID, so a
# person opens it the first time with right click > Open. Needs a Mac (lipo, codesign, hdiutil).
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/macos"

ARCH=universal
SKIP_BUILD=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    -h | --help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
case "$ARCH" in
  universal) ARCHES=(arm64 x86_64) ;;
  arm64 | x86_64) ARCHES=("$ARCH") ;;
  *) echo "unknown --arch $ARCH" >&2; exit 2 ;;
esac

SHORT_VERSION="${VERSION%%-*}"
WORK="$ROOT/dist-work/macos-package"
APP="$WORK/Orbe.app"
DMG="$DIST/orbe-$VERSION-macos-$ARCH.dmg"
IDENTITY="${MACOS_SIGN_IDENTITY:--}"
[[ "$IDENTITY" = "-" ]] && warn "macOS: ad-hoc signing; the app is not notarized"

echo "==> Orbe $VERSION for macOS ($ARCH)"

# ---- the executable ----------------------------------------------------------------------------
if [[ "$SKIP_BUILD" = 0 ]]; then
  for a in "${ARCHES[@]}"; do "$HERE/build.sh" "$a"; done
fi
rm -rf "$WORK"
mkdir -p "$WORK"
inputs=()
for a in "${ARCHES[@]}"; do
  test -f "$ROOT/bin/orbe.macos.editor.$a" || { echo "error: bin/orbe.macos.editor.$a is missing" >&2; exit 1; }
  inputs+=("$ROOT/bin/orbe.macos.editor.$a")
done
lipo -create -output "$WORK/Orbe" "${inputs[@]}"
lipo -info "$WORK/Orbe"
strip -x "$WORK/Orbe" 2>/dev/null || true

# ---- Orbe.app ----------------------------------------------------------------------------------
echo "==> assembling $APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$WORK/Orbe" "$APP/Contents/MacOS/Orbe"
cp "$HERE/Orbe.icns" "$APP/Contents/Resources/Orbe.icns"
cp "$ROOT/LICENSE.txt" "$ROOT/COPYRIGHT.txt" "$ROOT/NOTICE" "$APP/Contents/Resources/"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@SHORT_VERSION@/$SHORT_VERSION/g" \
  -e "s/@BUILD_SHA@/${ORBE_BUILD_SHA:-unknown}/g" \
  "$HERE/Info.plist.in" >"$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
printf 'APPL????' >"$APP/Contents/PkgInfo"

sign() {
  local ts=(--timestamp)
  [[ "$IDENTITY" = "-" ]] && ts=(--timestamp=none)
  codesign --force --sign "$IDENTITY" "${ts[@]}" "$@"
}
sign --options runtime --entitlements "$HERE/entitlements.plist" "$APP/Contents/MacOS/Orbe"
sign --options runtime --entitlements "$HERE/entitlements.plist" "$APP"
codesign --verify --strict --deep --verbose=2 "$APP"

# ---- DMG ---------------------------------------------------------------------------------------
echo "==> building $DMG"
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/Orbe.app"
ln -s /Applications "$STAGE/Applications"
# Finder window layout: background, icon size and positions (packaging/macos/dmg/README.md).
mkdir -p "$STAGE/.background"
cp "$HERE/dmg/background.tiff" "$STAGE/.background/background.tiff"
# The layout names the volume (the background's alias) and the app (its icon position). A layout
# left over from an older name opens a plain window with no background, so stop the build instead.
python3 - "$HERE/dmg/dmg-layout.DS_Store" "Orbe" "Orbe.app" <<'PY'
import sys
path, volume, app = sys.argv[1:4]
data = open(path, "rb").read()
if f"/Volumes/{volume}".encode() not in data or app.encode("utf-16-be") not in data:
    sys.exit(f"error: {path} is laid out for another volume or app; run packaging/macos/dmg/generate.py")
PY
cp "$HERE/dmg/dmg-layout.DS_Store" "$STAGE/.DS_Store"
rm -f "$DMG" "$WORK/raw.dmg"
# makehybrid + convert builds the image without attaching a device, unlike `create -srcfolder`,
# which is flaky on CI runners ("Resource busy").
# The volume name has no version: .DS_Store finds the background through an alias that includes it.
hdiutil makehybrid -hfs -hfs-volume-name "Orbe" -hfs-openfolder "$STAGE" -o "$WORK/raw.dmg" "$STAGE"
hdiutil convert "$WORK/raw.dmg" -format UDZO -imagekey zlib-level=9 -o "$DMG"
rm -f "$WORK/raw.dmg"
sign "$DMG"
codesign --verify --strict --verbose=2 "$DMG"

echo "==> done"
ls -lh "$DMG"
