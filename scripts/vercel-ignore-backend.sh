#!/usr/bin/env bash
set -u

base="${ZERANT_DIFF_BASE:-HEAD^}"
head="${ZERANT_DIFF_HEAD:-HEAD}"

if ! git rev-parse --verify "$base" >/dev/null 2>&1 || ! git rev-parse --verify "$head" >/dev/null 2>&1; then
  echo "Backend build required: comparison refs are unavailable."
  exit 1
fi

changed_files="$(git diff --name-only "$base" "$head")"

if printf '%s\n' "$changed_files" | grep -Eq '^(Cargo\.toml|Cargo\.lock|Dockerfile\.vercel|\.dockerignore|vercel\.json|crates/|services/zerant-api/|scripts/vercel-ignore-backend\.sh$)'; then
  echo "Backend build required: backend-sensitive files changed."
  exit 1
fi

echo "Backend build skipped: no backend-sensitive files changed."
exit 0
