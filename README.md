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
