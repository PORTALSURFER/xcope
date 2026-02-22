//! Xcope adapter around the reusable Toybox waveform view renderer.

use toybox::gui::waveform::{
    build_waveform_surface_commands, WaveformChannelStyle, WaveformDisplayMode, WaveformGridMode,
    WaveformViewConfig, WaveformViewStyle,
};
use toybox::gui::{declarative::SurfaceCommand, Color};

use crate::constants::MAX_VISUAL_CHANNELS;
use crate::params::{DisplayMode, XcopeUiState};
use crate::scope::ScopeFrame;
use crate::transport::{resolve_tempo_locked_window, subdivisions_for_grid, TransportSnapshot};

/// Xcope-local waveform stability policy applied before renderer sampling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderStabilityPolicy {
    /// Minimum samples-per-pixel ratio required to switch to envelope bins.
    pub minmax_trigger_samples_per_pixel: f32,
    /// Envelope points emitted per pixel when min/max mode is active.
    pub envelope_points_per_pixel: usize,
}

impl Default for RenderStabilityPolicy {
    fn default() -> Self {
        Self {
            minmax_trigger_samples_per_pixel: 2.0,
            envelope_points_per_pixel: 2,
        }
    }
}

/// Build region draw commands for one scope frame.
pub fn build_scope_surface_commands(
    frame: &ScopeFrame,
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
    width: u32,
    height: u32,
) -> Vec<SurfaceCommand> {
    let stable_frame =
        apply_render_stability_policy(frame, width, RenderStabilityPolicy::default());
    let channel_count = stable_frame.channel_count.min(MAX_VISUAL_CHANNELS);
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
    config.zoom_y = ui_state.zoom_y;
    config.grid_mode = grid_mode;
    config.horizontal_grid_lines = 8;
    config.style = xcope_waveform_style();

    build_waveform_surface_commands(
        width,
        height,
        stable_frame.sample_count(),
        channel_count,
        |channel, index| stable_frame.sample(channel, index),
        &config,
    )
}

fn apply_render_stability_policy(
    frame: &ScopeFrame,
    width: u32,
    policy: RenderStabilityPolicy,
) -> ScopeFrame {
    if frame.sample_count() < 2 || frame.channel_count == 0 {
        return frame.clone();
    }
    let pixels = width.max(1) as f32;
    let samples_per_pixel = frame.sample_count() as f32 / pixels;
    if samples_per_pixel < policy.minmax_trigger_samples_per_pixel {
        return frame.clone();
    }
    minmax_envelope_frame(
        frame,
        width.max(1) as usize,
        policy.envelope_points_per_pixel.max(2),
    )
}

fn minmax_envelope_frame(
    frame: &ScopeFrame,
    columns: usize,
    points_per_column: usize,
) -> ScopeFrame {
    if columns == 0 || points_per_column < 2 || frame.sample_count() < 2 {
        return frame.clone();
    }
    let channel_count = frame.channel_count.min(MAX_VISUAL_CHANNELS);
    if channel_count == 0 {
        return frame.clone();
    }

    let mut samples = Vec::with_capacity(columns.saturating_mul(points_per_column));
    let total = frame.sample_count();
    for column in 0..columns {
        let start = column.saturating_mul(total) / columns;
        let mut end = (column + 1).saturating_mul(total) / columns;
        if end <= start {
            end = (start + 1).min(total);
        }
        if start >= total {
            break;
        }

        let mut min_frame = [0.0f32; MAX_VISUAL_CHANNELS];
        let mut max_frame = [0.0f32; MAX_VISUAL_CHANNELS];
        for channel in 0..channel_count {
            let mut min_value = f32::INFINITY;
            let mut max_value = f32::NEG_INFINITY;
            for index in start..end {
                let value = frame.sample(channel, index);
                min_value = min_value.min(value);
                max_value = max_value.max(value);
            }
            min_frame[channel] = min_value.clamp(-1.2, 1.2);
            max_frame[channel] = max_value.clamp(-1.2, 1.2);
        }
        samples.push(min_frame);
        samples.push(max_frame);
    }

    ScopeFrame {
        channel_count,
        samples,
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
    fn stability_policy_keeps_low_density_frames_unchanged() {
        let frame = ScopeFrame {
            channel_count: 1,
            samples: (0..40).map(|i| [i as f32 / 40.0, 0.0]).collect(),
        };
        let stable = apply_render_stability_policy(&frame, 160, RenderStabilityPolicy::default());
        assert_eq!(stable, frame);
    }

    #[test]
    fn stability_policy_uses_minmax_envelope_for_high_density_frames() {
        let frame = ScopeFrame {
            channel_count: 1,
            samples: (0..120)
                .map(|index| {
                    let value = if index % 2 == 0 { -1.0 } else { 1.0 };
                    [value, 0.0]
                })
                .collect(),
        };
        let stable = apply_render_stability_policy(&frame, 20, RenderStabilityPolicy::default());
        assert_eq!(stable.sample_count(), 40);
        assert_eq!(stable.sample(0, 0), -1.0);
        assert_eq!(stable.sample(0, 1), 1.0);
    }

    #[test]
    fn stability_policy_is_deterministic_for_identical_inputs() {
        let frame = ScopeFrame {
            channel_count: 1,
            samples: (0..100)
                .map(|i| [((i % 7) as f32 / 3.0) - 1.0, 0.0])
                .collect(),
        };
        let a = apply_render_stability_policy(&frame, 30, RenderStabilityPolicy::default());
        let b = apply_render_stability_policy(&frame, 30, RenderStabilityPolicy::default());
        assert_eq!(a, b);
    }
}
