# Development

This repo is intended to be built and checked independently.

## Local checks

```bash
bash scripts/ci.sh
```

## VST3 checks

```bash
VST3_SDK_DIR=/mnt/e/lib/vst3sdk bash scripts/ci.sh --vst3
```

## Windows VST3 build output

Build with:

```powershell
Remove-Item Env:TOYBOX_ACTIVE_ARTIFACT -ErrorAction SilentlyContinue
cargo build -r --features vst3
```

Expected artifact path:

`C:/dist/xcope-v<version>.vst3/Contents/x86_64-win/xcope-v<version>.vst3`

## Ableton Live smoke protocol

1. Re-scan plugins in Live after build.
2. Insert `xcope` on an audio track; verify instantiation succeeds.
3. Verify audio passthrough (no mute/glitch/artifact introduced).
4. Open UI and resize; verify stable layout.
5. Toggle `FREEZE`; verify capture freezes/unfreezes cleanly.
6. Change mode/window/display/grid controls; verify responsive updates.
7. Save and reload project; verify xcope state restores.

## Working against local toybox (local-only)

See the meta-root `DEVELOPMENT.md` for the approved local-only patch workflow.
