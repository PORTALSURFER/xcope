# xcope

This is an independent plugin repository living inside the meta-workspace at `/home/uhx/dev/audiodev`.

## Workflow

- Run local CI (matches GitHub Actions):

  ```bash
  bash scripts/ci.sh
  ```

- VST3 is opt-in:

  ```bash
  VST3_SDK_DIR=/mnt/e/lib/vst3sdk bash scripts/ci.sh --vst3
  ```

- Windows VST3 release build (writes `.vst3` under `C:/dist`):

  ```powershell
  Remove-Item Env:TOYBOX_ACTIVE_ARTIFACT -ErrorAction SilentlyContinue
  cargo build -r --features vst3
  ```

  `--features vst3` now forces VST3 artifact output for the invocation, even if
  `TOYBOX_ACTIVE_ARTIFACT` is set to a different format.

- GUI screenshot harness (opt-in):

  ```bash
  bash scripts/ci.sh --screenshots
  ```

Notes:
- Add this plugin to the meta-root screenshot coverage policy file:
  `../scripts/screenshot-coverage.toml`.

## Docs

- Product spec: `docs/PRODUCT_SPEC.md`
- Project notes: `docs/PROJECT.md`
- Development notes: `docs/DEVELOPMENT.md`
- Strict declarative checklist (meta-root): `../docs/STRICT-DECLARATIVE-CHECKLIST.md`
