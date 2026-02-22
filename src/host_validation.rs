//! Host-behavior validation coverage for transport and state paths.

#[cfg(test)]
mod tests {
    use toybox::gui::declarative::SurfaceCommand;

    use crate::params::{DisplayMode, ScopeMode, TimeWindow, XcopeParams, XcopeUiState};
    use crate::scope::{build_scope_surface_commands, resolve_live_frame, ScopeCaptureBuffer};
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

    #[test]
    fn stable_signal_render_is_unchanged_for_sub_sample_transport_shift() {
        let capture = ScopeCaptureBuffer::new(512);
        for index in 0..256 {
            let sample = ((index as f32) * 0.2).sin();
            capture.write_sample([sample, 0.0], 1);
        }
        capture.set_transport_anchor(Some(32.0));

        let state = XcopeUiState {
            mode: ScopeMode::TempoLocked,
            time_window: TimeWindow::OneBar,
            display_mode: DisplayMode::Overlay,
            ..XcopeUiState::default()
        };
        let transport_a = TransportSnapshot {
            tempo_bpm: 120.0,
            is_playing: true,
            song_pos_beats: Some(28.0),
            time_sig_num: 4,
            time_sig_denom: 4,
        };
        let transport_b = TransportSnapshot {
            song_pos_beats: Some(27.95),
            ..transport_a
        };

        let frame_a = resolve_live_frame(&capture, &state, transport_a, 8.0);
        let frame_b = resolve_live_frame(&capture, &state, transport_b, 8.0);
        let commands_a = build_scope_surface_commands(&frame_a, &state, transport_a, 320, 180);
        let commands_b = build_scope_surface_commands(&frame_b, &state, transport_b, 320, 180);

        assert_eq!(frame_a.samples, frame_b.samples);
        assert_eq!(
            waveform_foreground_commands(&commands_a),
            waveform_foreground_commands(&commands_b)
        );
    }

    fn waveform_foreground_commands(commands: &[SurfaceCommand]) -> Vec<SurfaceCommand> {
        commands
            .iter()
            .filter(
                |command| matches!(command, SurfaceCommand::Line { color, .. } if color.a < 255),
            )
            .cloned()
            .collect()
    }
}
