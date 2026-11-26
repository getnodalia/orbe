#!/usr/bin/env bash
# Build the Orbe editor for one macOS architecture: bin/orbe.macos.editor.<arch>
#
# Usage: packaging/macos/build.sh arm64|x86_64
#
# Metal draws on Apple silicon; Intel Macs run the Compatibility renderer, so the build needs no
# Vulkan SDK and no prebuilt ANGLE (vulkan=no angle=no): nothing here comes from outside this
# repository or Xcode. Needs scons 4.4 or newer.
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"

ARCH="${1:-}"
case "$ARCH" in
  arm64 | x86_64) ;;
  *) echo "usage: $0 arm64|x86_64" >&2; exit 2 ;;
esac

cd "$ROOT"
scons platform=macos target=editor arch="$ARCH" vulkan=no angle=no accesskit=no \
  module_mono_enabled=no optimize=speed lto=none debug_symbols=no \
  -j"$(sysctl -n hw.ncpu)"
test -f "bin/orbe.macos.editor.$ARCH" || { echo "error: bin/orbe.macos.editor.$ARCH is missing" >&2; exit 1; }
echo "==> built bin/orbe.macos.editor.$ARCH"
