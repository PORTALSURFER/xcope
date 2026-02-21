//! Scope window-selection helpers.

use crate::params::XcopeUiState;
use crate::scope::{ScopeCaptureBuffer, ScopeFrame};
use crate::transport::{
    resolve_tempo_locked_window, resolve_visible_sample_count, TransportSnapshot,
};

/// Resolve visible sample count for current scope settings.
pub fn resolve_scope_window_samples(
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
    sample_rate_hz: f32,
) -> usize {
    resolve_visible_sample_count(ui_state, transport, sample_rate_hz)
}

/// Resolve one live frame from the capture ring.
pub fn resolve_live_frame(
    capture: &ScopeCaptureBuffer,
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
    sample_rate_hz: f32,
) -> ScopeFrame {
    let samples = resolve_scope_window_samples(ui_state, transport, sample_rate_hz);
    let mut frame = capture.snapshot_recent(samples);
    align_tempo_locked_phase(&mut frame, ui_state, transport);
    frame
}

/// Rotate tempo-locked frames so beat phase stays visually anchored.
///
/// We sample the most-recent window from the ring buffer, then circularly
/// rotate that window by the current beat phase. This keeps periodic material
/// visually stable against the musical grid instead of slowly drifting.
fn align_tempo_locked_phase(
    frame: &mut ScopeFrame,
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
) {
    if frame.sample_count() < 2 {
        return;
    }
    let Some(window) = resolve_tempo_locked_window(ui_state, transport) else {
        return;
    };
    if !window.beats_visible.is_finite() || window.beats_visible <= 0.0 {
        return;
    }

    let phase_beats = window.end_beat.rem_euclid(window.beats_visible);
    let phase_norm = (phase_beats / window.beats_visible).clamp(0.0, 1.0);
    let shift =
        ((phase_norm * frame.sample_count() as f64).round() as usize) % frame.sample_count();
    if shift > 0 {
        frame.samples.rotate_right(shift);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{DisplayMode, GridSubdivision, ScopeMode, TimeWindow, XcopeUiState};

    fn tempo_locked_state() -> XcopeUiState {
        XcopeUiState {
            mode: ScopeMode::TempoLocked,
            time_window: TimeWindow::OneBar,
            grid_subdivision: GridSubdivision::Div16,
            grid_triplet: false,
            display_mode: DisplayMode::Overlay,
            freeze: false,
            zoom_x: 1.0,
            zoom_y: 1.0,
            channel_visible: [true, true],
            channel_color: [0, 1],
        }
    }

    #[test]
    fn tempo_locked_frame_rotation_tracks_beat_phase() {
        let capture = ScopeCaptureBuffer::new(32);
        for value in 0..16 {
            capture.write_sample([value as f32, 0.0], 1);
        }

        let state = tempo_locked_state();
        let at_bar = resolve_live_frame(
            &capture,
            &state,
            TransportSnapshot {
                song_pos_beats: Some(4.0),
                ..TransportSnapshot::default()
            },
            8.0,
        );
        let half_bar_later = resolve_live_frame(
            &capture,
            &state,
            TransportSnapshot {
                song_pos_beats: Some(6.0),
                ..TransportSnapshot::default()
            },
            8.0,
        );

        assert_eq!(at_bar.sample_count(), 16);
        assert_eq!(half_bar_later.sample_count(), 16);
        assert_eq!(at_bar.sample(0, 0), 0.0);
        assert_eq!(half_bar_later.sample(0, 0), 8.0);
        assert_eq!(half_bar_later.sample(0, 8), 0.0);
    }
}
