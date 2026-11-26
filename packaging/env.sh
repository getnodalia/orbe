# shellcheck shell=bash
# Shared setup for the packaging scripts. Source it: `. "$(dirname "$0")/../env.sh"`.
#
# Exports:
#   ROOT          repository root
#   VERSION       ORBE_VERSION (override: ORBE_PACKAGE_VERSION)
#   DIST          output directory for release files (default: $ROOT/dist)
#   ORBE_BUILD_DATE   UTC build date, YYYY-MM-DD

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROOT

VERSION="${ORBE_PACKAGE_VERSION:-$(tr -d '[:space:]' <"$ROOT/ORBE_VERSION")}"
if [[ -z "$VERSION" ]]; then
  echo "error: ORBE_VERSION is empty" >&2
  exit 1
fi
export VERSION

DIST="${DIST:-$ROOT/dist}"
mkdir -p "$DIST"
export DIST

export ORBE_BUILD_DATE="${ORBE_BUILD_DATE:-$(date -u +%Y-%m-%d)}"

warn() { echo "warning: $*" >&2; }
