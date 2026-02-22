# MEMORY

Last Updated (UTC): 2026-02-22 09:05:04Z

## Current State

- The repository uses `bash scripts/run_agent_request.sh` for wake-up preflight.
- The local validation entrypoint is `bash scripts/ci_local.sh`.
- Runtime UI sizing now follows host `InputState.window_size` with cache invalidation on resize.
- Host-behavior validation tests cover loop-wrap, tempo automation, sample-rate/buffer-size projection, and project-reload state roundtrip.
- Active slice plan: `docs/plans/active/runtime-resize-host-validation.md`.
- Scope window selection now uses deterministic transport-anchored absolute sample windows (no phase-rotation shifting).
- Waveform rendering now applies an adaptive local min/max envelope stability policy for high samples-per-pixel regions.
- A standalone toybox request for envelope waveform painting was drafted for framework-level reuse work.

## Active Mission

- Keep the plugin repository ready for feature iteration while maintaining clear handoff state for stateless agent wake-up.

## Immediate Next Actions

1. Run Windows DAW smoke checks listed in `docs/plans/active/todo.md` for loop, tempo automation, sample-rate/buffer-size, and reload behavior.
2. Forward the prepared toybox waveform renderer request and track adoption timing in the active plan.
3. Define the next `docs/PRODUCT_SPEC.md` slice in a focused active plan note before implementation.

## Constraints And Notes

- Do not expand `AGENTS.md` beyond portal responsibilities.
- Keep detailed planning in `docs/plans/`.
- VST3 checks remain opt-in and require `VST3_SDK_DIR`.
