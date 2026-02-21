# Project

## Summary

Xcope is a precision, tempo-synchronized, multi-channel oscilloscope VST3
plugin for Windows-based DAWs. It targets waveform inspection, rhythmic
analysis, transient validation, and phase-alignment workflows.

The versioned product scope and acceptance criteria live in
`docs/PRODUCT_SPEC.md`.

## Constraints

- Keep DSP realtime-safe (no allocations/blocking in audio callback).
- Keep the repo thin: framework/GUI mechanics belong in `toybox`.
