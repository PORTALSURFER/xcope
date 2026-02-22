# Runtime Resize And Host Validation Slice

## Objective

Implement the next product-spec slice by:

1. Making GUI layout/render sizing follow host window size at runtime.
2. Adding deterministic validation coverage for host-behavior scenarios.
3. Keeping plan/todo artifacts explicit for fast stateless wake-up.

## Scope

- Update `src/gui` layout/render sizing to use runtime `InputState.window_size`.
- Ensure UI-spec caching invalidates on window-size changes.
- Add tests for:
  - loop-wrap handling in tempo-locked mode,
  - tempo automation effects on visible window size,
  - sample-rate and buffer-size transport projection behavior,
  - project reload state roundtrip.

## Acceptance Criteria

- Runtime UI geometry derives from host window size (with minimum defaults).
- Scope surface rendering dimensions match current runtime layout geometry.
- New host-behavior tests run under `cargo test`.
- Active todo queue references concrete follow-up work, not generic placeholders.
