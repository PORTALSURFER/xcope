# Xcope — Product Specification Document

Version 1.1  
Target Format: VST3  
Target Platform: Windows (64-bit) only  
Framework: Toybox

---

## 1. Product Overview

**Xcope** is a precision, tempo-synchronized, multi-channel oscilloscope VST3 plugin for Windows-based DAWs. It is designed for waveform inspection, rhythmic analysis, transient validation, and phase alignment tasks in electronic music production and mixing workflows.

Xcope is a musically aware oscilloscope. It must integrate tightly with host tempo and transport so that waveform visualization becomes rhythmically meaningful rather than purely technical.

The product is intended for producers, mixing engineers, and sound designers working in rhythm-focused genres (techno, IDM, electronic).

---

## 2. Product Scope

### Included in Version 1

- Windows (64-bit) support only
- VST3 format only
- Real-time waveform display
- Multi-channel visualization (1–4 channels)
- Tempo synchronization
- Musical grid overlay
- Overlay and split display modes
- Zoom controls
- Freeze mode
- Resizable UI

### Explicitly Excluded in Version 1

- macOS support
- AU/AAX formats
- Standalone application
- Preset management
- Screenshot export
- MIDI trigger modes
- Cross-instance communication
- Spectrum or frequency-domain view
- Correlation meters

---

## 3. Product Goals

Xcope must:

1. Provide accurate waveform visualization.
2. Support musically meaningful tempo alignment.
3. Allow comparison of multiple channels.
4. Remain stable under all typical DAW transport conditions.
5. Maintain low CPU usage.
6. Scale cleanly at all window sizes.
7. Introduce zero audio artifacts.

The plugin must behave predictably under:

- Transport start/stop
- Loop playback
- Tempo automation
- Time signature changes
- Sample rate changes
- Buffer size changes
- Project reload

---

## 4. Functional Requirements

### 4.1 Real-Time Waveform Display

The plugin must display incoming audio in real time.

Requirements:

- Accurate amplitude rendering.
- Stable and smooth display.
- Support for at least 1–4 channels.
- Correct rendering at 44.1kHz, 48kHz, and 96kHz.
- No distortion of waveform shape due to rendering artifacts.

---

### 4.2 Display Modes

#### 4.2.1 Free-Running Mode

- Waveform scrolls continuously.
- Independent of host tempo.
- Suitable for inspecting raw signal shape.

#### 4.2.2 Tempo-Locked Mode

- Display window aligns to musical bars or beats.
- Waveform appears visually stable relative to grid during playback.
- Must support:
  - 1 bar view
  - 2 bar view
  - 4 bar view
  - Optional 1 beat view

When host transport stops, the plugin must behave predictably (either freeze or revert to stable static display).

---

### 4.3 Musical Grid Overlay

The plugin must render a visual grid in tempo-locked mode.

Grid must support:

- Beat divisions
- Subdivisions (1/8, 1/16, 1/32)
- Triplet mode
- Adaptation to time signature

Grid behavior requirements:

- Grid lines must align precisely to tempo.
- Grid must scale correctly with zoom.
- Grid must remain stable during playback.
- Grid must not visually drift.

---

### 4.4 Multi-Channel Support

The plugin must accept up to 4 channels of input.

Display Modes:

1. Overlay Mode
   - All visible channels rendered in the same vertical region.
   - Each channel has distinct color.
2. Split Mode
   - Each channel rendered in a vertically separated region.
   - Regions must scale proportionally with window size.

Channel Controls:

- Enable/disable visibility per channel.
- Per-channel color selection (at least basic palette).
- Optional per-channel amplitude scaling.

If more than 4 channels are present:

- Only first 4 must be visualized.
- No crash or undefined behavior.

---

### 4.5 Zoom Controls

#### Horizontal Zoom

- Adjust visible time window.
- In tempo-locked mode: change number of visible bars/beats.
- In free-running mode: change time duration window.

#### Vertical Zoom

- Adjust amplitude scaling.
- Must not introduce waveform clipping artifacts in rendering.

Zoom must:

- Update smoothly.
- Preserve grid alignment.
- Not destabilize rendering.

---

### 4.6 Freeze Mode

The plugin must include a Freeze function.

Behavior:

- Captures current display window.
- Stops waveform updates.
- Allows detailed inspection.
- Freeze must work in both free-running and tempo-locked modes.
- Unfreeze resumes normal operation cleanly.

---

## 5. User Interface Requirements

### 5.1 Layout Structure

The UI must follow a strict hierarchical layout system.

Structure:

Root

- Top Toolbar
- Scope Display Region
- Bottom Control Bar

Requirements:

- No overlapping elements.
- No text overflow.
- No clipping of UI controls.
- Fully resizable window.
- Consistent layout at all aspect ratios.
- UI must scale proportionally.

---

### 5.2 Top Toolbar

Must include:

- Mode selector (Free / Tempo-Locked)
- Bar/Beat length selector
- Overlay / Split toggle
- Freeze button

Controls must be clearly labeled and legible at all sizes.

---

### 5.3 Scope Display Region

Must include:

- Waveform rendering layer
- Grid layer
- Clear separation between channels (in split mode)
- Optional channel labels

The waveform region must visually dominate the UI.

---

### 5.4 Bottom Control Bar

Must include:

- Horizontal zoom control
- Vertical zoom control
- Channel visibility toggles
- Reset zoom button (optional but recommended)

---

## 6. Performance Requirements

The plugin must:

- Introduce negligible overhead on the audio thread.
- Avoid dynamic allocation in the audio processing path.
- Maintain stable CPU usage during UI rendering.
- Remain stable at high sample rates (96kHz).
- Operate correctly at various buffer sizes.

The plugin must never:

- Produce audio glitches.
- Block the audio thread.
- Cause DAW instability.

---

## 7. Behavioral Requirements

### 7.1 Transport Behavior

Must handle:

- Rapid play/stop
- Loop regions
- Tempo automation
- Time signature changes
- Scrubbing

Tempo-locked waveform must not visually jump or jitter.

---

### 7.2 Sample Rate and Buffer Changes

On sample rate change:

- Visualization must adjust correctly.
- No crash or visual corruption.

On buffer size change:

- Visualization must remain stable.

---

## 8. Visual Design Guidelines

Design must be:

- Technical
- Minimal
- High contrast
- Dark theme default
- Clean typography
- No decorative or unnecessary UI elements

Waveform must be the primary visual element.

---

## 9. Stability Requirements

The plugin must:

- Load and unload safely.
- Reopen projects without state corruption.
- Save and restore internal state (zoom level, mode, etc.).
- Handle missing transport information gracefully.

If transport info is unavailable:

- Fall back to free-running mode.
- Disable tempo-locked grid without crashing.

---

## 10. Acceptance Criteria

Xcope is considered complete when:

1. It runs reliably in major Windows DAWs (e.g., Ableton Live, Bitwig, Reaper).
2. Tempo-locked mode remains visually stable.
3. Multi-channel overlay and split modes work correctly.
4. UI scales without layout defects.
5. CPU usage remains low.
6. No audio artifacts are introduced.
7. All core features function as described.

---

## 11. Summary

Xcope is a Windows-only, VST3-only, musically synchronized oscilloscope plugin built with Toybox.

It must provide:

- Accurate waveform inspection
- Tempo alignment
- Multi-channel comparison
- Stable and efficient rendering
- Clean, scalable UI

The development team is responsible for implementation strategy, internal architecture, and optimization. The functional behavior and feature set defined in this document must be delivered in full for Version 1.
