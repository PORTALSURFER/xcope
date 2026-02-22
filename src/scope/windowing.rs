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
    let sample_count = resolve_scope_window_samples(ui_state, transport, sample_rate_hz);
    let end_exclusive = resolve_window_end_sample(capture, ui_state, transport, sample_rate_hz);
    capture.snapshot_ending_at(end_exclusive, sample_count)
}

/// Resolve one absolute end-sample index for deterministic window selection.
fn resolve_window_end_sample(
    capture: &ScopeCaptureBuffer,
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
    sample_rate_hz: f32,
) -> u64 {
    let fallback_end = capture.total_written_samples();
    let Some(window) = resolve_tempo_locked_window(ui_state, transport) else {
        return fallback_end;
    };
    let Some((anchor_sample, anchor_beats)) = capture.transport_anchor() else {
        return fallback_end;
    };
    let tempo = transport.tempo_bpm.max(1.0) as f64;
    let samples_per_beat = (sample_rate_hz.max(1.0) as f64 * 60.0) / tempo;
    let delta_beats = anchor_beats - window.end_beat;
    let delta_samples = delta_beats * samples_per_beat;
    // Ignore sub-sample transport deltas so UI polling jitter does not force
    // a one-sample window jump in tempo-locked mode.
    let resolved = if delta_samples.abs() < 1.0 {
        anchor_sample as f64
    } else {
        anchor_sample as f64 - delta_samples
    };
    if !resolved.is_finite() {
        return fallback_end;
    }
    resolved.round().max(0.0) as u64
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
    fn tempo_locked_window_reads_anchored_absolute_sample_range() {
        let capture = ScopeCaptureBuffer::new(1024);
        for value in 0..640 {
            capture.write_sample([value as f32, 0.0], 1);
        }
        capture.set_transport_anchor(Some(16.0));

        let state = tempo_locked_state();
        let expected_samples = resolve_scope_window_samples(
            &state,
            TransportSnapshot {
                tempo_bpm: 120.0,
                song_pos_beats: Some(12.0),
                ..TransportSnapshot::default()
            },
            48.0,
        );
        assert_eq!(expected_samples, 96);
        let at_bar = resolve_live_frame(
            &capture,
            &state,
            TransportSnapshot {
                tempo_bpm: 120.0,
                song_pos_beats: Some(12.0),
                ..TransportSnapshot::default()
            },
            48.0,
        );
        let one_beat_earlier = resolve_live_frame(
            &capture,
            &state,
            TransportSnapshot {
                tempo_bpm: 120.0,
                song_pos_beats: Some(11.0),
                ..TransportSnapshot::default()
            },
            48.0,
        );

        assert_eq!(at_bar.sample_count(), 96);
        assert_eq!(one_beat_earlier.sample_count(), 96);
        assert_eq!(at_bar.sample(0, 0), 448.0);
        assert_eq!(at_bar.sample(0, 95), 543.0);
        assert_eq!(one_beat_earlier.sample(0, 0), 424.0);
        assert_eq!(one_beat_earlier.sample(0, 95), 519.0);
    }

    #[test]
    fn sub_sample_transport_shift_does_not_force_pixel_window_jump() {
        let capture = ScopeCaptureBuffer::new(64);
        for value in 0..32 {
            capture.write_sample([value as f32, 0.0], 1);
        }
        capture.set_transport_anchor(Some(8.0));
        let state = tempo_locked_state();

        let a = resolve_live_frame(
            &capture,
            &state,
            TransportSnapshot {
                tempo_bpm: 120.0,
                song_pos_beats: Some(4.00),
                ..TransportSnapshot::default()
            },
            8.0,
        );
        let b = resolve_live_frame(
            &capture,
            &state,
            TransportSnapshot {
                tempo_bpm: 120.0,
                song_pos_beats: Some(3.95),
                ..TransportSnapshot::default()
            },
            8.0,
        );

        assert_eq!(a.samples, b.samples);
    }
}
