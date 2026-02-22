# AGENTS

This file is a stateless wake-up portal. Keep it short and explicit.

## 60-Second Wake-Up

1. Run preflight: `bash scripts/run_agent_request.sh`.
2. Read active mission and queue: `docs/plans/active/todo.md`.
3. Read current state snapshot: `MEMORY.md`.
4. Confirm product boundaries (when doing feature work): `docs/PRODUCT_SPEC.md`.
5. Before handoff, run `bash scripts/ci_local.sh`, then update `MEMORY.md` and `docs/plans/active/todo.md`.

## Source Of Truth

- Active task queue: `docs/plans/active/todo.md`
- Plan directory map: `docs/plans/index.md`
- Current repository state: `MEMORY.md`
- Product requirements: `docs/PRODUCT_SPEC.md`
- Development workflow: `docs/DEVELOPMENT.md`

## Guardrails

- Keep this file as a portal, not a knowledge base.
- Put all detailed plans and technical notes under `docs/`.
- Remove stale instructions instead of appending exceptions.
