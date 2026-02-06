#!/usr/bin/env bash
set -euo pipefail

# Convenience wrapper so you can run:
#   bash send-test.sh ...
# from the repo root.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$ROOT/scripts/send-test.sh" "$@"

