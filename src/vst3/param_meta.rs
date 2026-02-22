//! Parameter metadata and display-string helpers for VST3 host queries.

use crate::constants::{ParamId, ZOOM_MAX, ZOOM_MIN};

/// Return the host-visible parameter title for one id.
pub(super) fn param_title(param_id: u32) -> &'static str {
    match ParamId::from_raw(param_id) {
        Some(ParamId::ScopeMode) => "Mode",
        Some(ParamId::TimeWindow) => "Window",
        Some(ParamId::GridSubdivision) => "Grid",
        Some(ParamId::GridTriplet) => "Triplet",
        Some(ParamId::DisplayMode) => "Display",
        Some(ParamId::Freeze) => "Freeze",
        Some(ParamId::ZoomX) => "Zoom X",
        Some(ParamId::ZoomY) => "Zoom Y",
        Some(ParamId::Channel1Visible) => "Ch1",
        Some(ParamId::Channel2Visible) => "Ch2",
        Some(ParamId::Channel1Color) => "Ch1 Color",
        Some(ParamId::Channel2Color) => "Ch2 Color",
        None => "Unknown",
    }
}

/// Return the host-visible unit label for one id.
pub(super) fn param_units(param_id: u32) -> &'static str {
    match ParamId::from_raw(param_id) {
        Some(ParamId::ZoomX | ParamId::ZoomY) => "x",
        _ => "",
    }
}

/// Return the discrete step count for one parameter id.
pub(super) fn param_steps(param_id: u32) -> i32 {
    match ParamId::from_raw(param_id) {
        Some(
            ParamId::ScopeMode
            | ParamId::DisplayMode
            | ParamId::GridTriplet
            | ParamId::Freeze
            | ParamId::Channel1Visible
            | ParamId::Channel2Visible,
        ) => 1,
        Some(ParamId::TimeWindow) => 3,
        Some(ParamId::GridSubdivision) => 2,
        Some(ParamId::Channel1Color | ParamId::Channel2Color) => 7,
        _ => 0,
    }
}

/// Return one textual parameter value representation for host UIs.
pub(super) fn param_display_string(param_id: u32, normalized: f64) -> String {
    match ParamId::from_raw(param_id) {
        Some(ParamId::ZoomX | ParamId::ZoomY) => {
            let zoom = zoom_from_normalized(normalized);
            format!("{zoom:.2}x")
        }
        Some(_) => format!("{normalized:.3}"),
        None => "-".to_string(),
    }
}

fn zoom_from_normalized(normalized: f64) -> f64 {
    let value = normalized.clamp(0.0, 1.0);
    ZOOM_MIN as f64 + (ZOOM_MAX - ZOOM_MIN) as f64 * value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_display_uses_zoom_multiplier_not_normalized_value() {
        let at_default = param_display_string(ParamId::ZoomX.raw(), 0.2);
        assert_eq!(at_default, "1.00x");
        let at_max = param_display_string(ParamId::ZoomY.raw(), 1.0);
        assert_eq!(at_max, "4.00x");
    }
}
