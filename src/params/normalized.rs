//! Normalized parameter conversion helpers for host automation.

use crate::constants::{ParamId, CHANNEL_COLOR_COUNT, ZOOM_MAX, ZOOM_MIN};

use super::{
    clamp_color_index, clamp_zoom, DisplayMode, GridSubdivision, ScopeMode, TimeWindow, XcopeParams,
};

/// Return the number of exposed parameters.
pub const fn param_count() -> u32 {
    ParamId::Channel2Color.raw()
}

/// Return one normalized parameter value.
pub fn read_param_normalized(params: &XcopeParams, param_id: u32) -> Option<f64> {
    let snapshot = params.snapshot();
    let value = match ParamId::from_raw(param_id)? {
        ParamId::ScopeMode => snapshot.mode.to_index() as f64,
        ParamId::TimeWindow => snapshot.time_window.to_index() as f64 / 3.0,
        ParamId::GridSubdivision => snapshot.grid_subdivision.to_index() as f64 / 2.0,
        ParamId::GridTriplet => bool_to_normalized(snapshot.grid_triplet),
        ParamId::DisplayMode => snapshot.display_mode.to_index() as f64,
        ParamId::Freeze => bool_to_normalized(snapshot.freeze),
        ParamId::ZoomX => zoom_to_normalized(snapshot.zoom_x),
        ParamId::ZoomY => zoom_to_normalized(snapshot.zoom_y),
        ParamId::Channel1Visible => bool_to_normalized(snapshot.channel_visible[0]),
        ParamId::Channel2Visible => bool_to_normalized(snapshot.channel_visible[1]),
        ParamId::Channel1Color => color_to_normalized(snapshot.channel_color[0]),
        ParamId::Channel2Color => color_to_normalized(snapshot.channel_color[1]),
    };
    Some(value.clamp(0.0, 1.0))
}

/// Apply one normalized parameter value.
pub fn apply_param_normalized(params: &XcopeParams, param_id: u32, normalized: f64) -> bool {
    let value = normalized.clamp(0.0, 1.0);
    let Some(param) = ParamId::from_raw(param_id) else {
        return false;
    };
    match param {
        ParamId::ScopeMode => params.set_mode(if value >= 0.5 {
            ScopeMode::TempoLocked
        } else {
            ScopeMode::FreeRunning
        }),
        ParamId::TimeWindow => params.set_time_window(match discrete_from_normalized(value, 4) {
            0 => TimeWindow::OneBeat,
            2 => TimeWindow::TwoBars,
            3 => TimeWindow::FourBars,
            _ => TimeWindow::OneBar,
        }),
        ParamId::GridSubdivision => {
            params.set_grid_subdivision(match discrete_from_normalized(value, 3) {
                0 => GridSubdivision::Div8,
                2 => GridSubdivision::Div32,
                _ => GridSubdivision::Div16,
            })
        }
        ParamId::GridTriplet => params.set_grid_triplet(value >= 0.5),
        ParamId::DisplayMode => params.set_display_mode(if value >= 0.5 {
            DisplayMode::Split
        } else {
            DisplayMode::Overlay
        }),
        ParamId::Freeze => params.set_freeze(value >= 0.5),
        ParamId::ZoomX => params.set_zoom_x(normalized_to_zoom(value)),
        ParamId::ZoomY => params.set_zoom_y(normalized_to_zoom(value)),
        ParamId::Channel1Visible => params.set_channel_visible(0, value >= 0.5),
        ParamId::Channel2Visible => params.set_channel_visible(1, value >= 0.5),
        ParamId::Channel1Color => params.set_channel_color(0, normalized_to_color(value)),
        ParamId::Channel2Color => params.set_channel_color(1, normalized_to_color(value)),
    }
    true
}

fn bool_to_normalized(value: bool) -> f64 {
    if value {
        1.0
    } else {
        0.0
    }
}

fn zoom_to_normalized(value: f32) -> f64 {
    ((clamp_zoom(value) - ZOOM_MIN) / (ZOOM_MAX - ZOOM_MIN)) as f64
}

fn normalized_to_zoom(value: f64) -> f32 {
    (ZOOM_MIN + (ZOOM_MAX - ZOOM_MIN) * value as f32).clamp(ZOOM_MIN, ZOOM_MAX)
}

fn color_to_normalized(value: u32) -> f64 {
    clamp_color_index(value) as f64 / (CHANNEL_COLOR_COUNT.saturating_sub(1).max(1) as f64)
}

fn normalized_to_color(value: f64) -> u32 {
    discrete_from_normalized(value, CHANNEL_COLOR_COUNT as usize) as u32
}

fn discrete_from_normalized(value: f64, options: usize) -> usize {
    let max_index = options.saturating_sub(1);
    ((value.clamp(0.0, 1.0) * max_index as f64).round() as usize).min(max_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_param_roundtrip_updates_zoom() {
        let params = XcopeParams::new();
        assert!(apply_param_normalized(&params, ParamId::ZoomX.raw(), 0.75));
        let normalized = read_param_normalized(&params, ParamId::ZoomX.raw())
            .expect("zoom param should be readable");
        assert!((normalized - 0.75).abs() < 0.01);
    }
}
