#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

required_paths=(
  "AGENTS.md"
  "MEMORY.md"
  "docs/README.md"
  "docs/plans/index.md"
  "docs/plans/active/todo.md"
  "scripts/ci_local.sh"
)

for path in "${required_paths[@]}"; do
  if [[ ! -f "${path}" ]]; then
    echo "[agent-request] missing required handoff file: ${path}" >&2
    exit 1
  fi
done

bash scripts/policy-check.sh

echo "[agent-request] preflight ok"
