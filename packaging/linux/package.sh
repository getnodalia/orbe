#!/usr/bin/env bash
# Build and package Orbe for Linux (x86_64):
#
#   $DIST/orbe-<version>-linux-x86_64.AppImage  any distro with glibc >= the build host's
#   $DIST/orbe-<version>-linux-x86_64.AppImage.zsync  delta updates (needs zsyncmake)
#   $DIST/orbe-<version>-linux-x86_64.deb       Debian, Ubuntu, Mint, Pop!_OS, ...
#   $DIST/orbe-<version>-linux-x86_64.rpm       Fedora, openSUSE, RHEL, ...
#   $DIST/orbe-<version>-linux-x86_64.tar.gz    plain FHS-style tree (bin/, share/)
#
# Usage: packaging/linux/package.sh [--skip-build] [--formats "appimage deb rpm tar"]
#
# Needs: scons and the engine's Linux build dependencies; nfpm for deb/rpm
# (https://nfpm.goreleaser.com); appimagetool for the AppImage (downloaded if missing). Build on
# an old distro (CI: Ubuntu 22.04, glibc 2.35) so the binary runs on newer ones.
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/linux"
APP_ID=dev.danielmiguelt.Orbe

SKIP_BUILD=0
FORMATS="appimage deb rpm tar"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1; shift ;;
    --formats) FORMATS="$2"; shift 2 ;;
    -h | --help) sed -n '2,15p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64) DEB_ARCH=amd64 ;;
  *) echo "unsupported architecture $ARCH" >&2; exit 2 ;;
esac
export ORBE_MAINTAINER="${ORBE_MAINTAINER:-Nodalia <https://github.com/getnodalia/orbe/issues>}"
BASENAME="orbe-$VERSION-linux-$ARCH"

echo "==> Orbe $VERSION for Linux $ARCH ($FORMATS)"

BIN="$ROOT/bin/orbe.linuxbsd.editor.$ARCH"
if [[ "$SKIP_BUILD" = 0 ]]; then
  (cd "$ROOT" && scons platform=linuxbsd target=editor arch="$ARCH" accesskit=no \
    module_mono_enabled=no optimize=speed lto=none debug_symbols=no \
    -j"$(nproc)")
fi
test -x "$BIN" || { echo "error: $BIN is missing" >&2; exit 1; }

WORK="$ROOT/dist-work/linux-package"
STAGE="$WORK/root"
rm -rf "$WORK"

# ---- stage an FHS tree (shared by every format) -------------------------------------------------
install -Dm755 "$BIN" "$STAGE/usr/bin/orbe"
strip "$STAGE/usr/bin/orbe" 2>/dev/null || true
install -Dm644 "$HERE/$APP_ID.desktop" "$STAGE/usr/share/applications/$APP_ID.desktop"
install -Dm644 "$HERE/$APP_ID.mime.xml" "$STAGE/usr/share/mime/packages/$APP_ID.xml"
mkdir -p "$STAGE/usr/share/metainfo"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@DATE@/$ORBE_BUILD_DATE/g" \
  "$HERE/$APP_ID.metainfo.xml.in" >"$STAGE/usr/share/metainfo/$APP_ID.metainfo.xml"
install -Dm644 "$ROOT/misc/logo/icon.svg" "$STAGE/usr/share/icons/hicolor/scalable/apps/$APP_ID.svg"
install -Dm644 "$ROOT/misc/logo/icon.png" "$STAGE/usr/share/icons/hicolor/256x256/apps/$APP_ID.png"
# Licences and notices: the MIT licence and every third-party notice travel with the package.
install -Dm644 "$ROOT/LICENSE.txt" "$STAGE/usr/share/doc/orbe/LICENSE.txt"
install -Dm644 "$ROOT/COPYRIGHT.txt" "$STAGE/usr/share/doc/orbe/COPYRIGHT.txt"
install -Dm644 "$ROOT/NOTICE" "$STAGE/usr/share/doc/orbe/NOTICE"

if command -v desktop-file-validate >/dev/null; then
  desktop-file-validate "$STAGE/usr/share/applications/$APP_ID.desktop"
fi
if command -v appstreamcli >/dev/null; then
  appstreamcli validate --no-net --explain "$STAGE/usr/share/metainfo/$APP_ID.metainfo.xml"
fi

has() { local format="$1"; case " $FORMATS " in *" $format "*) return 0 ;; *) return 1 ;; esac; }

# ---- .tar.gz ------------------------------------------------------------------------------------
if has tar; then
  mkdir -p "$WORK/tar"
  cp -R "$STAGE/usr" "$WORK/tar/$BASENAME"
  tar -C "$WORK/tar" -czf "$DIST/$BASENAME.tar.gz" "$BASENAME"
  echo "wrote $DIST/$BASENAME.tar.gz"
fi

# ---- .deb / .rpm --------------------------------------------------------------------------------
if has deb || has rpm; then
  command -v nfpm >/dev/null || { echo "error: nfpm not found (https://nfpm.goreleaser.com/install/)" >&2; exit 1; }
  export VERSION
  export NFPM_ARCH="$DEB_ARCH"
  # nfpm expands env vars in fields like `version` and `arch`, but not in `contents[].src`.
  sed "s|\${STAGE}|$STAGE|g" "$HERE/nfpm.yaml" >"$WORK/nfpm.yaml"
  for fmt in deb rpm; do
    if has "$fmt"; then (cd "$ROOT" && nfpm package -f "$WORK/nfpm.yaml" -p "$fmt" -t "$DIST/$BASENAME.$fmt"); fi
  done
fi

# ---- AppImage -----------------------------------------------------------------------------------
if has appimage; then
  APPDIR="$WORK/Orbe.AppDir"
  cp -R "$STAGE" "$APPDIR"
  ln -s usr/bin/orbe "$APPDIR/AppRun"
  cp "$HERE/$APP_ID.desktop" "$APPDIR/$APP_ID.desktop"
  cp "$ROOT/misc/logo/icon.png" "$APPDIR/$APP_ID.png"
  ln -s "$APP_ID.png" "$APPDIR/.DirIcon"

  TOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
  if [[ -z "$TOOL" ]]; then
    TOOL="$WORK/appimagetool-$ARCH.AppImage"
    curl --proto '=https' -fsSL -o "$TOOL" \
      "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
    chmod +x "$TOOL"
  fi
  OUT="$(cd "$DIST" && pwd)/$BASENAME.AppImage"
  APPDIR="$(cd "$APPDIR" && pwd)"
  TOOL="$(cd "$(dirname "$TOOL")" && pwd)/$(basename "$TOOL")"
  rm -f "$OUT.zsync"
  # Update information: AppImageUpdate reads it from the file and fetches only the blocks that
  # changed in a newer release, through the .zsync published next to each AppImage.
  REPO="${GITHUB_REPOSITORY:-getnodalia/orbe}"
  UPDATE_INFO="gh-releases-zsync|${REPO%%/*}|${REPO#*/}|latest|orbe-*-linux-$ARCH.AppImage.zsync"
  # Extract-and-run works without FUSE (containers, CI). With zsyncmake on the host appimagetool
  # also writes the .zsync, into its working directory, hence the cd.
  (cd "$DIST" && ARCH="$ARCH" APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream -u "$UPDATE_INFO" "$APPDIR" "$OUT")
  echo "wrote $OUT"
  if [[ ! -s "$OUT.zsync" ]]; then
    warn "zsyncmake not found, so $OUT.zsync was not written; AppImage delta updates need it"
  fi
fi

"$STAGE/usr/bin/orbe" --version
echo "==> done"
ls -lh "$DIST"
