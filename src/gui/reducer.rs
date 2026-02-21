//! UI action reducer for Xcope GUI interactions.

use toybox::gui::declarative::UiAction;

use crate::params::{DisplayMode, GridSubdivision, ScopeMode, TimeWindow, XcopeParams};

use super::layout::{
    CHANNEL_VISIBLE_KEYS, DISPLAY_KEY, FREEZE_KEY, GRID_SUBDIV_KEY, GRID_TRIPLET_KEY, MODE_KEY,
    RESET_ZOOM_KEY, WINDOW_KEY, ZOOM_X_KEY, ZOOM_Y_KEY,
};

/// Apply one declarative UI action to shared parameters.
///
/// Returns `true` when freeze state changed.
pub fn apply_ui_action(params: &XcopeParams, action: UiAction) -> bool {
    let before_freeze = params.snapshot().freeze;

    match action {
        UiAction::DropdownSelected { key, index } => match key.as_str() {
            MODE_KEY => params.set_mode(match index {
                1 => ScopeMode::TempoLocked,
                _ => ScopeMode::FreeRunning,
            }),
            WINDOW_KEY => params.set_time_window(match index {
                0 => TimeWindow::OneBeat,
                2 => TimeWindow::TwoBars,
                3 => TimeWindow::FourBars,
                _ => TimeWindow::OneBar,
            }),
            DISPLAY_KEY => params.set_display_mode(match index {
                1 => DisplayMode::Split,
                _ => DisplayMode::Overlay,
            }),
            GRID_SUBDIV_KEY => params.set_grid_subdivision(match index {
                0 => GridSubdivision::Div8,
                2 => GridSubdivision::Div32,
                _ => GridSubdivision::Div16,
            }),
            _ => {}
        },
        UiAction::ToggleChanged { key, value } => match key.as_str() {
            FREEZE_KEY => params.set_freeze(value),
            GRID_TRIPLET_KEY => params.set_grid_triplet(value),
            _ => {
                for (index, channel_key) in CHANNEL_VISIBLE_KEYS.iter().enumerate() {
                    if key == *channel_key {
                        params.set_channel_visible(index, value);
                    }
                }
            }
        },
        UiAction::SliderChanged { key, value } => match key.as_str() {
            ZOOM_X_KEY => params.set_zoom_x(value),
            ZOOM_Y_KEY => params.set_zoom_y(value),
            _ => {}
        },
        UiAction::ButtonPressed { key } if key == RESET_ZOOM_KEY => {
            params.set_zoom_x(crate::constants::ZOOM_X_DEFAULT);
            params.set_zoom_y(crate::constants::ZOOM_Y_DEFAULT);
        }
        _ => {}
    }

    before_freeze != params.snapshot().freeze
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reducer_updates_dropdown_backed_state() {
        let params = XcopeParams::new();
        let changed = apply_ui_action(
            &params,
            UiAction::DropdownSelected {
                key: MODE_KEY.to_string(),
                index: 1,
            },
        );
        assert!(!changed);
        assert_eq!(params.snapshot().mode, ScopeMode::TempoLocked);
    }

    #[test]
    fn reducer_reports_freeze_toggle_changes() {
        let params = XcopeParams::new();
        let changed = apply_ui_action(
            &params,
            UiAction::ToggleChanged {
                key: FREEZE_KEY.to_string(),
                value: true,
            },
        );
        assert!(changed);
        assert!(params.snapshot().freeze);
    }

    #[test]
    fn reducer_resets_zoom() {
        let params = XcopeParams::new();
        params.set_zoom_x(3.0);
        params.set_zoom_y(2.0);
        let _ = apply_ui_action(
            &params,
            UiAction::ButtonPressed {
                key: RESET_ZOOM_KEY.to_string(),
            },
        );
        let snapshot = params.snapshot();
        assert_eq!(snapshot.zoom_x, crate::constants::ZOOM_X_DEFAULT);
        assert_eq!(snapshot.zoom_y, crate::constants::ZOOM_Y_DEFAULT);
    }
}
