#!/usr/bin/env bash
# Personal vs work database profiles for Larder.
#
# Usage:
#   source scripts/profile.sh personal   # sets DATABASE_URL for this shell
#   source scripts/profile.sh work
#   ./scripts/profile.sh personal serve  # run a command against that profile
#   ./scripts/profile.sh work init
#   ./scripts/profile.sh work import-bundle /path/to/work-bundle.json
#
# Default data dir: ~/.local/share/larder/
# Override with LARDER_DATA_DIR.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="${LARDER_DATA_DIR:-$HOME/.local/share/larder}"
mkdir -p "$DATA_DIR"

usage() {
  cat <<'EOF'
Usage:
  source scripts/profile.sh <personal|work>
  ./scripts/profile.sh <personal|work> <command> [args...]

Commands (when not sourcing):
  env                 Print DATABASE_URL
  init                Initialize empty DB for this profile
  serve               Start API + web (LARDER_ADDR default 127.0.0.1:18080)
  tui                 Open TUI
  export-work         Export recipes tagged #work to a bundle JSON
  import-bundle FILE  Import a bundle into this profile's DB

Examples:
  source scripts/profile.sh personal
  larder tag add "Protein Pitas" work
  ./scripts/profile.sh personal export-work
  ./scripts/profile.sh work init
  ./scripts/profile.sh work import-bundle "$HOME/.local/share/larder/work-bundle.json"
  ./scripts/profile.sh work serve
EOF
}

profile_db() {
  case "$1" in
    personal) echo "$DATA_DIR/personal.db" ;;
    work) echo "$DATA_DIR/work.db" ;;
    *) echo "Unknown profile: $1 (use personal or work)" >&2; return 1 ;;
  esac
}

if [[ $# -lt 1 ]]; then
  usage
  exit 1
fi

PROFILE="$1"
shift
DB="$(profile_db "$PROFILE")"
export DATABASE_URL="sqlite:$DB"

# If sourced, only set env and stop.
if [[ "${BASH_SOURCE[0]}" != "${0}" ]]; then
  echo "DATABASE_URL=$DATABASE_URL ($PROFILE)"
  return 0 2>/dev/null || exit 0
fi

CMD="${1:-env}"
shift || true

LARDER_BIN="${LARDER_BIN:-}"
if [[ -z "$LARDER_BIN" ]]; then
  if [[ -x "$ROOT/target/release/larder" ]]; then
    LARDER_BIN="$ROOT/target/release/larder"
  elif [[ -x "$ROOT/target/debug/larder" ]]; then
    LARDER_BIN="$ROOT/target/debug/larder"
  elif command -v larder >/dev/null 2>&1; then
    LARDER_BIN="$(command -v larder)"
  else
    echo "larder binary not found. Build with: cargo build -p larder-tui --bin larder" >&2
    exit 1
  fi
fi

case "$CMD" in
  env)
    echo "profile=$PROFILE"
    echo "DATABASE_URL=$DATABASE_URL"
    ;;
  init)
    "$LARDER_BIN" --database "$DATABASE_URL" init
    ;;
  serve)
    export LARDER_ADDR="${LARDER_ADDR:-127.0.0.1:18080}"
    echo "Serving $PROFILE on $LARDER_ADDR (db=$DB)"
    "$LARDER_BIN" --database "$DATABASE_URL" serve
    ;;
  tui)
    "$LARDER_BIN" --database "$DATABASE_URL" tui
    ;;
  export-work)
    OUT="${1:-$DATA_DIR/work-bundle.json}"
    TAG="${LARDER_WORK_TAG:-work}"
    "$LARDER_BIN" --database "$DATABASE_URL" export --format json --tag "$TAG" --output "$OUT"
    echo "Work bundle: $OUT (tag #$TAG)"
    ;;
  import-bundle)
    FILE="${1:?import-bundle requires a file path}"
    "$LARDER_BIN" --database "$DATABASE_URL" import --file "$FILE"
    ;;
  *)
    # Pass through any other larder subcommand
    "$LARDER_BIN" --database "$DATABASE_URL" "$CMD" "$@"
    ;;
esac
