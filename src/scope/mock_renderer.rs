//! Deterministic scope renderer used by the vertical slice.

use toybox::gui::declarative::SurfaceCommand;
use toybox::gui::{Color, Point, Rect};

use crate::params::{DisplayMode, ScopeMode, TimeWindow, XcopeUiState};
use crate::scope::ScopeFrame;
use crate::transport::{subdivisions_for_grid, TransportSnapshot};

/// Build region draw commands for one scope frame.
pub fn build_scope_surface_commands(
    frame: &ScopeFrame,
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
    width: u32,
    height: u32,
) -> Vec<SurfaceCommand> {
    let width_i32 = width.max(1) as i32;
    let height_i32 = height.max(1) as i32;
    let mut commands = Vec::with_capacity(2048);

    commands.push(SurfaceCommand::FillRect {
        rect: Rect {
            origin: Point { x: 0, y: 0 },
            size: toybox::gui::Size { width, height },
        },
        color: Color::rgb(14, 17, 20),
    });

    let grid_line_count = vertical_grid_lines(ui_state, transport).max(2);
    for step in 0..=grid_line_count {
        let x = ((step as f32 / grid_line_count as f32) * width_i32 as f32).round() as i32;
        commands.push(SurfaceCommand::Line {
            start: Point {
                x: x.clamp(0, width_i32),
                y: 0,
            },
            end: Point {
                x: x.clamp(0, width_i32),
                y: height_i32,
            },
            color: if step % 4 == 0 {
                Color::rgb(44, 50, 57)
            } else {
                Color::rgb(30, 35, 41)
            },
        });
    }

    let horizontal_lines = 8;
    for step in 0..=horizontal_lines {
        let y = ((step as f32 / horizontal_lines as f32) * height_i32 as f32).round() as i32;
        commands.push(SurfaceCommand::Line {
            start: Point { x: 0, y },
            end: Point { x: width_i32, y },
            color: if step == horizontal_lines / 2 {
                Color::rgb(53, 61, 69)
            } else {
                Color::rgb(27, 31, 37)
            },
        });
    }

    if frame.sample_count() < 2 {
        return commands;
    }

    let visible_channels: Vec<usize> = (0..frame.channel_count)
        .filter(|index| ui_state.channel_visible[*index])
        .collect();
    if visible_channels.is_empty() {
        return commands;
    }

    match ui_state.display_mode {
        DisplayMode::Overlay => {
            for channel in visible_channels {
                draw_waveform_channel(
                    &mut commands,
                    frame,
                    channel,
                    width_i32,
                    height_i32,
                    0,
                    height_i32,
                    ui_state.zoom_y,
                    palette_color(ui_state.channel_color[channel]),
                );
            }
        }
        DisplayMode::Split => {
            let lane_count = visible_channels.len().max(1) as i32;
            for (lane_index, channel) in visible_channels.iter().enumerate() {
                let lane_top = lane_index as i32 * height_i32 / lane_count;
                let lane_bottom = ((lane_index as i32 + 1) * height_i32 / lane_count)
                    .clamp(lane_top + 1, height_i32);
                draw_waveform_channel(
                    &mut commands,
                    frame,
                    *channel,
                    width_i32,
                    height_i32,
                    lane_top,
                    lane_bottom,
                    ui_state.zoom_y,
                    palette_color(ui_state.channel_color[*channel]),
                );
                if lane_index > 0 {
                    commands.push(SurfaceCommand::Line {
                        start: Point { x: 0, y: lane_top },
                        end: Point {
                            x: width_i32,
                            y: lane_top,
                        },
                        color: Color::rgb(42, 48, 54),
                    });
                }
            }
        }
    }

    commands
}

#[allow(clippy::too_many_arguments)]
fn draw_waveform_channel(
    commands: &mut Vec<SurfaceCommand>,
    frame: &ScopeFrame,
    channel: usize,
    width: i32,
    _height: i32,
    lane_top: i32,
    lane_bottom: i32,
    zoom_y: f32,
    color: Color,
) {
    let lane_height = (lane_bottom - lane_top).max(1);
    let center_y = lane_top + lane_height / 2;
    let scale_y = (lane_height as f32 * 0.45) / zoom_y.max(0.05);

    let sample_count = frame.sample_count();
    let points = width.max(2) as usize;
    let step = (sample_count as f32 / points as f32).max(1.0);

    let mut prev = None;
    for point_index in 0..points {
        let sample_index = ((point_index as f32 * step) as usize).min(sample_count - 1);
        let sample = frame.sample(channel, sample_index).clamp(-1.2, 1.2);
        let x = ((point_index as f32 / (points - 1) as f32) * width as f32).round() as i32;
        let y = (center_y as f32 - sample * scale_y).round() as i32;
        let current = Point {
            x: x.clamp(0, width),
            y: y.clamp(lane_top, lane_bottom),
        };
        if let Some(previous) = prev {
            commands.push(SurfaceCommand::Line {
                start: previous,
                end: current,
                color,
            });
        }
        prev = Some(current);
    }
}

fn vertical_grid_lines(ui_state: &XcopeUiState, transport: TransportSnapshot) -> u32 {
    match ui_state.mode {
        ScopeMode::TempoLocked if transport.song_pos_beats.is_some() => {
            let beats_per_bar = beats_per_bar(transport.time_sig_num, transport.time_sig_denom);
            let beats_visible = match ui_state.time_window {
                TimeWindow::OneBeat => 1.0,
                TimeWindow::OneBar => beats_per_bar,
                TimeWindow::TwoBars => beats_per_bar * 2.0,
                TimeWindow::FourBars => beats_per_bar * 4.0,
            };
            let subdivisions =
                subdivisions_for_grid(ui_state.grid_subdivision, ui_state.grid_triplet);
            ((beats_visible * subdivisions as f32).round() as u32).max(2)
        }
        _ => 8,
    }
}

fn beats_per_bar(num: u16, denom: u16) -> f32 {
    let denom = match denom {
        1 | 2 | 4 | 8 | 16 => denom,
        _ => 4,
    } as f32;
    num.clamp(1, 32) as f32 * (4.0 / denom)
}

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
    use crate::params::{DisplayMode, GridSubdivision};

    #[test]
    fn renderer_emits_background_and_grid_without_samples() {
        let frame = ScopeFrame::default();
        let state = XcopeUiState {
            display_mode: DisplayMode::Overlay,
            ..XcopeUiState::default()
        };
        let commands =
            build_scope_surface_commands(&frame, &state, TransportSnapshot::default(), 320, 180);
        assert!(!commands.is_empty());
    }

    #[test]
    fn tempo_locked_grid_density_increases_with_triplet_subdivisions() {
        let mut state = XcopeUiState {
            mode: ScopeMode::TempoLocked,
            grid_subdivision: GridSubdivision::Div16,
            grid_triplet: false,
            ..XcopeUiState::default()
        };
        let straight = vertical_grid_lines(
            &state,
            TransportSnapshot {
                song_pos_beats: Some(0.0),
                ..TransportSnapshot::default()
            },
        );
        state.grid_triplet = true;
        let triplet = vertical_grid_lines(
            &state,
            TransportSnapshot {
                song_pos_beats: Some(0.0),
                ..TransportSnapshot::default()
            },
        );
        assert!(triplet > straight);
    }
}
