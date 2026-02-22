# MEMORY

Last Updated (UTC): 2026-02-22 08:35:07Z

## Current State

- The repository uses `bash scripts/run_agent_request.sh` for wake-up preflight.
- The local validation entrypoint is `bash scripts/ci_local.sh`.
- Handoff context is centralized across `AGENTS.md`, this file, and `docs/plans/active/todo.md`.

## Active Mission

- Keep the plugin repository ready for feature iteration while maintaining clear handoff state for stateless agent wake-up.

## Immediate Next Actions

1. Pull the next work item from `docs/plans/active/todo.md` and scope it before coding.
2. Keep `MEMORY.md` and the active todo queue aligned whenever task status changes.
3. Run `bash scripts/ci_local.sh` before each handoff or commit.

## Constraints And Notes

- Do not expand `AGENTS.md` beyond portal responsibilities.
- Keep detailed planning in `docs/plans/`.
- VST3 checks remain opt-in and require `VST3_SDK_DIR`.
