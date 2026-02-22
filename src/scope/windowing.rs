//! Scope window-selection helpers.

use crate::params::XcopeUiState;
use crate::scope::{ScopeCaptureBuffer, ScopeFrame};
use crate::transport::{
    resolve_tempo_locked_window, resolve_visible_sample_count, TransportSnapshot,
};

/// One resolved render view containing both data and transport alignment.
#[derive(Clone, Debug)]
pub struct ResolvedScopeView {
    /// Scope frame sampled from the capture ring for this render pass.
    pub frame: ScopeFrame,
    /// Transport snapshot aligned to the same resolved frame window.
    pub render_transport: TransportSnapshot,
}

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
    resolve_live_view(capture, ui_state, transport, sample_rate_hz).frame
}

/// Resolve one coherent render view from capture and transport.
///
/// The returned `frame` and `render_transport` are aligned to the same end
/// sample so waveform data and tempo grid phase remain coherent.
pub fn resolve_live_view(
    capture: &ScopeCaptureBuffer,
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
    sample_rate_hz: f32,
) -> ResolvedScopeView {
    let sample_count = resolve_scope_window_samples(ui_state, transport, sample_rate_hz);
    let alignment = resolve_window_alignment(capture, ui_state, transport);
    let frame = capture.snapshot_ending_at(alignment.end_exclusive, sample_count);
    let mut render_transport = transport;
    if let Some(song_pos_beats) = alignment.aligned_song_pos_beats {
        render_transport.song_pos_beats = Some(song_pos_beats);
    }

    ResolvedScopeView {
        frame,
        render_transport,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WindowAlignment {
    end_exclusive: u64,
    aligned_song_pos_beats: Option<f64>,
}

/// Resolve one absolute end-sample index and aligned beat-domain position.
fn resolve_window_alignment(
    capture: &ScopeCaptureBuffer,
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
) -> WindowAlignment {
    let fallback_end = capture.total_written_samples();
    let Some(_window) = resolve_tempo_locked_window(ui_state, transport) else {
        return WindowAlignment {
            end_exclusive: fallback_end,
            aligned_song_pos_beats: None,
        };
    };

    // Anchor-first policy: render window and grid phase are both derived from
    // the exact audio-thread anchor to avoid GUI-thread transport jitter.
    let Some((anchor_sample, anchor_beats)) = capture.transport_anchor() else {
        return WindowAlignment {
            end_exclusive: fallback_end,
            aligned_song_pos_beats: None,
        };
    };
    WindowAlignment {
        end_exclusive: anchor_sample,
        aligned_song_pos_beats: Some(anchor_beats),
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
    fn tempo_locked_window_uses_latest_anchor_sample_for_render_alignment() {
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
        assert_eq!(at_bar.sample(0, 0), 544.0);
        assert_eq!(at_bar.sample(0, 95), 639.0);
        assert_eq!(one_beat_earlier.sample(0, 0), 544.0);
        assert_eq!(one_beat_earlier.sample(0, 95), 639.0);
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

    #[test]
    fn sub_sample_transport_shift_preserves_aligned_render_transport() {
        let capture = ScopeCaptureBuffer::new(64);
        for value in 0..32 {
            capture.write_sample([value as f32, 0.0], 1);
        }
        capture.set_transport_anchor(Some(8.0));
        let state = tempo_locked_state();

        let a = resolve_live_view(
            &capture,
            &state,
            TransportSnapshot {
                tempo_bpm: 120.0,
                song_pos_beats: Some(4.00),
                ..TransportSnapshot::default()
            },
            8.0,
        );
        let b = resolve_live_view(
            &capture,
            &state,
            TransportSnapshot {
                tempo_bpm: 120.0,
                song_pos_beats: Some(3.95),
                ..TransportSnapshot::default()
            },
            8.0,
        );

        assert_eq!(a.frame.samples, b.frame.samples);
        assert_eq!(
            a.render_transport.song_pos_beats,
            b.render_transport.song_pos_beats
        );
    }
}
