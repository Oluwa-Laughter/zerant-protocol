#!/usr/bin/env bash
# Compatibility entry point. Projects only sanitized readiness; never wallet history.
set -euo pipefail
exec python3 "$(dirname "$0")/z3-check.py"
