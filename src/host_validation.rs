//! Host-behavior validation coverage for transport and state paths.

#[cfg(test)]
mod tests {
    use crate::params::{DisplayMode, ScopeMode, TimeWindow, XcopeParams, XcopeUiState};
    use crate::state_io::{decode_state_payload, encode_state_payload};
    use crate::transport::{
        project_song_position_beats, resolve_tempo_locked_window, resolve_visible_sample_count,
        TransportSnapshot,
    };

    fn tempo_locked_state(window: TimeWindow) -> XcopeUiState {
        XcopeUiState {
            mode: ScopeMode::TempoLocked,
            time_window: window,
            ..XcopeUiState::default()
        }
    }

    #[test]
    fn loop_wrap_keeps_tempo_locked_window_valid() {
        let state = tempo_locked_state(TimeWindow::OneBar);
        let before_loop = resolve_tempo_locked_window(
            &state,
            TransportSnapshot {
                tempo_bpm: 128.0,
                is_playing: true,
                song_pos_beats: Some(7.75),
                time_sig_num: 4,
                time_sig_denom: 4,
            },
        )
        .expect("tempo-locked window should resolve before loop wrap");
        let after_loop = resolve_tempo_locked_window(
            &state,
            TransportSnapshot {
                tempo_bpm: 128.0,
                is_playing: true,
                song_pos_beats: Some(0.25),
                time_sig_num: 4,
                time_sig_denom: 4,
            },
        )
        .expect("tempo-locked window should resolve after loop wrap");

        assert_eq!(before_loop.beats_visible, after_loop.beats_visible);
        assert!(after_loop.start_beat < after_loop.end_beat);
    }

    #[test]
    fn tempo_automation_updates_visible_sample_window() {
        let state = tempo_locked_state(TimeWindow::OneBar);
        let slow = resolve_visible_sample_count(
            &state,
            TransportSnapshot {
                tempo_bpm: 90.0,
                is_playing: true,
                song_pos_beats: Some(8.0),
                time_sig_num: 4,
                time_sig_denom: 4,
            },
            48_000.0,
        );
        let fast = resolve_visible_sample_count(
            &state,
            TransportSnapshot {
                tempo_bpm: 150.0,
                is_playing: true,
                song_pos_beats: Some(8.0),
                time_sig_num: 4,
                time_sig_denom: 4,
            },
            48_000.0,
        );

        assert!(slow > fast);
    }

    #[test]
    fn transport_projection_handles_sample_rate_and_buffer_changes() {
        let base_beats = 32.0;
        let one_large_block = project_song_position_beats(base_beats, 120.0, true, 512, 48_000.0);
        let two_small_blocks = project_song_position_beats(
            project_song_position_beats(base_beats, 120.0, true, 256, 48_000.0),
            120.0,
            true,
            256,
            48_000.0,
        );
        let at_96k = project_song_position_beats(base_beats, 120.0, true, 512, 96_000.0);

        assert!((one_large_block - two_small_blocks).abs() < 1.0e-9);
        assert!(one_large_block > at_96k);
    }

    #[test]
    fn project_reload_roundtrip_restores_ui_state() {
        let source = XcopeParams::new();
        source.set_mode(ScopeMode::TempoLocked);
        source.set_time_window(TimeWindow::FourBars);
        source.set_grid_triplet(true);
        source.set_display_mode(DisplayMode::Split);
        source.set_freeze(true);
        source.set_zoom_x(2.0);
        source.set_zoom_y(1.5);
        source.set_channel_visible(0, true);
        source.set_channel_visible(1, false);
        source.set_channel_color(0, 3);
        source.set_channel_color(1, 6);

        let payload = encode_state_payload(&source);
        let reloaded = XcopeParams::new();
        decode_state_payload(&reloaded, &payload).expect("saved state should decode");

        assert_eq!(reloaded.snapshot(), source.snapshot());
    }
}
