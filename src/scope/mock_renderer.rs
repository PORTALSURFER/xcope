//! Xcope adapter around the reusable Toybox waveform view renderer.

use toybox::gui::waveform::{
    build_waveform_surface_commands, WaveformChannelStyle, WaveformDisplayMode, WaveformGridMode,
    WaveformSamplingMode, WaveformViewConfig, WaveformViewStyle,
};
use toybox::gui::{declarative::SurfaceCommand, Color};

use crate::constants::MAX_VISUAL_CHANNELS;
use crate::params::{DisplayMode, ScopeMode, XcopeUiState};
use crate::scope::ScopeFrame;
use crate::transport::{resolve_tempo_locked_window, subdivisions_for_grid, TransportSnapshot};

/// Build region draw commands for one scope frame.
pub fn build_scope_surface_commands(
    frame: &ScopeFrame,
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
    width: u32,
    height: u32,
) -> Vec<SurfaceCommand> {
    let channel_count = frame.channel_count.min(MAX_VISUAL_CHANNELS);
    let channel_styles: [WaveformChannelStyle; MAX_VISUAL_CHANNELS] =
        std::array::from_fn(|index| WaveformChannelStyle {
            visible: index < channel_count && ui_state.channel_visible[index],
            color: palette_color(ui_state.channel_color[index]),
        });
    let grid_mode = grid_mode_from_transport(ui_state, transport);
    let display_mode = match ui_state.display_mode {
        DisplayMode::Overlay => WaveformDisplayMode::Overlay,
        DisplayMode::Split => WaveformDisplayMode::Split,
    };

    let mut config = WaveformViewConfig::new(&channel_styles[..channel_count]);
    config.display_mode = display_mode;
    config.sampling_mode = sampling_mode_for_frame(ui_state, frame.sample_count(), width);
    config.zoom_y = ui_state.zoom_y;
    config.grid_mode = grid_mode;
    config.horizontal_grid_lines = 8;
    config.style = xcope_waveform_style();

    build_waveform_surface_commands(
        width,
        height,
        frame.sample_count(),
        channel_count,
        |channel, index| frame.sample(channel, index),
        &config,
    )
}

/// Select one renderer sampling mode from the current sample-to-pixel density.
fn sampling_mode_for_frame(
    ui_state: &XcopeUiState,
    sample_count: usize,
    width: u32,
) -> WaveformSamplingMode {
    // Tempo-locked rendering must be deterministic across frame updates.
    // Keep one mode in lock view to avoid visual churn near density thresholds.
    if ui_state.mode == ScopeMode::TempoLocked {
        return WaveformSamplingMode::EnvelopeMinMax;
    }

    let columns = width.max(1) as usize;
    if sample_count > columns.saturating_mul(2) {
        WaveformSamplingMode::EnvelopeMinMax
    } else {
        WaveformSamplingMode::Linear
    }
}

/// Resolve waveform grid mode from transport and UI state.
fn grid_mode_from_transport(
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
) -> WaveformGridMode {
    if let Some(window) = resolve_tempo_locked_window(ui_state, transport) {
        return WaveformGridMode::TempoLocked {
            beats_visible: window.beats_visible,
            beats_per_bar: beats_per_bar(transport.time_sig_num, transport.time_sig_denom) as f64,
            subdivisions_per_beat: subdivisions_for_grid(
                ui_state.grid_subdivision,
                ui_state.grid_triplet,
            )
            .max(1),
        };
    }
    WaveformGridMode::Fixed { line_count: 8 }
}

/// Return Xcope-specific waveform style tokens.
fn xcope_waveform_style() -> WaveformViewStyle {
    WaveformViewStyle {
        background: Color::rgb(14, 17, 20),
        grid_bar: Color::rgb(44, 50, 57),
        grid_beat: Color::rgb(36, 41, 47),
        grid_subdivision: Color::rgb(30, 35, 41),
        grid_horizontal: Color::rgb(27, 31, 37),
        grid_horizontal_center: Color::rgb(53, 61, 69),
        lane_divider: Color::rgb(42, 48, 54),
    }
}

/// Compute beats-per-bar from one time-signature pair.
fn beats_per_bar(num: u16, denom: u16) -> f32 {
    let denom = match denom {
        1 | 2 | 4 | 8 | 16 => denom,
        _ => 4,
    } as f32;
    num.clamp(1, 32) as f32 * (4.0 / denom)
}

/// Resolve one stable channel color from the current palette index.
fn palette_color(index: u32) -> Color {
    match index % 8 {
        0 => Color::rgb(136, 224, 255),
        1 => Color::rgb(255, 161, 128),
        2 => Color::rgb(182, 255, 141),
        3 => Color::rgb(239, 189, 255),
        4 => Color::rgb(255, 233, 153),
        5 => Color::rgb(170, 187, 255),
        6 => Color::rgb(255, 166, 220),
        _ => Color::rgb(191, 255, 235),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_emits_background_and_grid_without_samples() {
        let frame = ScopeFrame::default();
        let commands = build_scope_surface_commands(
            &frame,
            &XcopeUiState::default(),
            TransportSnapshot::default(),
            320,
            180,
        );
        assert!(!commands.is_empty());
    }

    #[test]
    fn tempo_locked_grid_is_phase_stable() {
        let state = XcopeUiState {
            mode: crate::params::ScopeMode::TempoLocked,
            grid_subdivision: crate::params::GridSubdivision::Div8,
            time_window: crate::params::TimeWindow::OneBar,
            ..XcopeUiState::default()
        };
        let frame = ScopeFrame {
            channel_count: 1,
            samples: vec![[0.0; MAX_VISUAL_CHANNELS]; 16],
        };
        let at_bar = build_scope_surface_commands(
            &frame,
            &state,
            TransportSnapshot {
                song_pos_beats: Some(4.0),
                ..TransportSnapshot::default()
            },
            320,
            180,
        );
        let quarter_beat_later = build_scope_surface_commands(
            &frame,
            &state,
            TransportSnapshot {
                song_pos_beats: Some(4.25),
                ..TransportSnapshot::default()
            },
            320,
            180,
        );
        assert_eq!(at_bar, quarter_beat_later);
    }

    #[test]
    fn sampling_mode_uses_linear_when_density_is_low() {
        assert_eq!(
            sampling_mode_for_frame(&XcopeUiState::default(), 128, 256),
            WaveformSamplingMode::Linear
        );
    }

    #[test]
    fn sampling_mode_uses_envelope_when_density_is_high() {
        assert_eq!(
            sampling_mode_for_frame(&XcopeUiState::default(), 513, 256),
            WaveformSamplingMode::EnvelopeMinMax
        );
    }

    #[test]
    fn sampling_mode_handles_zero_width() {
        assert_eq!(
            sampling_mode_for_frame(&XcopeUiState::default(), 2, 0),
            WaveformSamplingMode::Linear
        );
    }

    #[test]
    fn tempo_locked_sampling_is_always_envelope() {
        let state = XcopeUiState {
            mode: crate::params::ScopeMode::TempoLocked,
            ..XcopeUiState::default()
        };
        assert_eq!(
            sampling_mode_for_frame(&state, 2, 1024),
            WaveformSamplingMode::EnvelopeMinMax
        );
    }
}
