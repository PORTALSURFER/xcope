# MEMORY

Last Updated (UTC): 2026-02-22 08:46:12Z

## Current State

- The repository uses `bash scripts/run_agent_request.sh` for wake-up preflight.
- The local validation entrypoint is `bash scripts/ci_local.sh`.
- Runtime UI sizing now follows host `InputState.window_size` with cache invalidation on resize.
- Host-behavior validation tests cover loop-wrap, tempo automation, sample-rate/buffer-size projection, and project-reload state roundtrip.
- Active slice plan: `docs/plans/active/runtime-resize-host-validation.md`.

## Active Mission

- Keep the plugin repository ready for feature iteration while maintaining clear handoff state for stateless agent wake-up.

## Immediate Next Actions

1. Run Windows DAW smoke checks listed in `docs/plans/active/todo.md` for loop, tempo automation, sample-rate/buffer-size, and reload behavior.
2. Draft toybox handoff requests for any reusable framework needs discovered during smoke validation.
3. Define the next `docs/PRODUCT_SPEC.md` slice in a focused active plan note before implementation.

## Constraints And Notes

- Do not expand `AGENTS.md` beyond portal responsibilities.
- Keep detailed planning in `docs/plans/`.
- VST3 checks remain opt-in and require `VST3_SDK_DIR`.
