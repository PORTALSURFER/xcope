//! Declarative UI layout composition for Xcope.

use toybox::gui::declarative::{
    button, column_slots, dropdown, panel, root_frame_sized, row_slots, slider, surface, textbox,
    toggle, weighted_slot, Node, SurfaceCommand, UiSpec,
};
use toybox::gui::Size;

use crate::constants::{WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::params::XcopeUiState;

/// Root frame key.
pub const ROOT_KEY: &str = "xcope-root";
/// Scope surface key.
pub const SCOPE_SURFACE_KEY: &str = "scope-surface";

/// Mode selector key.
pub const MODE_KEY: &str = "mode";
/// Time-window selector key.
pub const WINDOW_KEY: &str = "time-window";
/// Display-mode selector key.
pub const DISPLAY_KEY: &str = "display-mode";
/// Grid subdivision selector key.
pub const GRID_SUBDIV_KEY: &str = "grid-subdivision";
/// Triplet toggle key.
pub const GRID_TRIPLET_KEY: &str = "grid-triplet";
/// Freeze toggle key.
pub const FREEZE_KEY: &str = "freeze";

/// Horizontal zoom slider key.
pub const ZOOM_X_KEY: &str = "zoom-x";
/// Vertical zoom slider key.
pub const ZOOM_Y_KEY: &str = "zoom-y";
/// Reset-zoom button key.
pub const RESET_ZOOM_KEY: &str = "reset-zoom";

/// Channel visibility toggle keys.
pub const CHANNEL_VISIBLE_KEYS: [&str; 4] =
    ["ch1-visible", "ch2-visible", "ch3-visible", "ch4-visible"];

/// Toolbar region fixed design-space height.
pub const TOOLBAR_HEIGHT: u32 = 84;
/// Bottom control-bar fixed design-space height.
pub const BOTTOM_BAR_HEIGHT: u32 = 96;
/// Scope region fixed design-space height.
pub const SCOPE_HEIGHT: u32 = WINDOW_HEIGHT - TOOLBAR_HEIGHT - BOTTOM_BAR_HEIGHT;

/// Build one full UI spec from state and rendered scope commands.
pub fn build_ui_spec(snapshot: &XcopeUiState, scope_commands: Vec<SurfaceCommand>) -> UiSpec {
    let toolbar = panel("toolbar", toolbar_row(snapshot));
    let scope = panel(
        "scope-panel",
        surface(
            SCOPE_SURFACE_KEY,
            Size {
                width: WINDOW_WIDTH,
                height: SCOPE_HEIGHT,
            },
            scope_commands,
        )
        .fill(),
    );
    let bottom = panel("bottom-bar", bottom_row(snapshot));

    let content = column_slots(vec![
        weighted_slot(toolbar, TOOLBAR_HEIGHT as u16),
        weighted_slot(scope, SCOPE_HEIGHT as u16),
        weighted_slot(bottom, BOTTOM_BAR_HEIGHT as u16),
    ]);

    UiSpec::new(root_frame_sized(
        ROOT_KEY,
        content,
        Size {
            width: WINDOW_WIDTH,
            height: WINDOW_HEIGHT,
        },
    ))
}

fn toolbar_row(snapshot: &XcopeUiState) -> Node {
    row_slots(vec![
        weighted_slot(toolbar_field("MODE", mode_dropdown(snapshot)), 17),
        weighted_slot(toolbar_field("WINDOW", window_dropdown(snapshot)), 17),
        weighted_slot(toolbar_field("DISPLAY", display_dropdown(snapshot)), 17),
        weighted_slot(toolbar_field("GRID", grid_dropdown(snapshot)), 17),
        weighted_slot(toolbar_field("TRIPLET", triplet_toggle(snapshot)), 16),
        weighted_slot(toolbar_field("FREEZE", freeze_toggle(snapshot)), 16),
    ])
}

fn bottom_row(snapshot: &XcopeUiState) -> Node {
    let zoom_group = row_slots(vec![
        weighted_slot(toolbar_field("ZOOM X", zoom_x_slider(snapshot)), 50),
        weighted_slot(toolbar_field("ZOOM Y", zoom_y_slider(snapshot)), 50),
    ]);

    let channels = row_slots(vec![
        weighted_slot(toolbar_field("CH1", channel_toggle(0, snapshot)), 16),
        weighted_slot(toolbar_field("CH2", channel_toggle(1, snapshot)), 16),
        weighted_slot(toolbar_field("CH3", channel_toggle(2, snapshot)), 16),
        weighted_slot(toolbar_field("CH4", channel_toggle(3, snapshot)), 16),
        weighted_slot(toolbar_field("", reset_zoom_button()), 36),
    ]);

    row_slots(vec![
        weighted_slot(zoom_group, 50),
        weighted_slot(channels, 50),
    ])
}

fn toolbar_field(label: &str, control: Node) -> Node {
    panel(
        format!("field-{label}"),
        column_slots(vec![
            weighted_slot(textbox(label).text_align_center().fill(), 35),
            weighted_slot(control.fill(), 65),
        ]),
    )
    .pad_all(4)
}

fn mode_dropdown(snapshot: &XcopeUiState) -> Node {
    dropdown(MODE_KEY, 2, snapshot.mode.to_index() as usize)
        .dropdown_option_labels(vec!["FREE".into(), "LOCK".into()])
        .control_size(Size {
            width: 110,
            height: 28,
        })
}

fn window_dropdown(snapshot: &XcopeUiState) -> Node {
    dropdown(WINDOW_KEY, 4, snapshot.time_window.to_index() as usize)
        .dropdown_option_labels(vec![
            "1BEAT".into(),
            "1BAR".into(),
            "2BAR".into(),
            "4BAR".into(),
        ])
        .control_size(Size {
            width: 110,
            height: 28,
        })
}

fn display_dropdown(snapshot: &XcopeUiState) -> Node {
    dropdown(DISPLAY_KEY, 2, snapshot.display_mode.to_index() as usize)
        .dropdown_option_labels(vec!["OVER".into(), "SPLIT".into()])
        .control_size(Size {
            width: 110,
            height: 28,
        })
}

fn grid_dropdown(snapshot: &XcopeUiState) -> Node {
    dropdown(
        GRID_SUBDIV_KEY,
        3,
        snapshot.grid_subdivision.to_index() as usize,
    )
    .dropdown_option_labels(vec!["1/8".into(), "1/16".into(), "1/32".into()])
    .control_size(Size {
        width: 110,
        height: 28,
    })
}

fn triplet_toggle(snapshot: &XcopeUiState) -> Node {
    toggle(GRID_TRIPLET_KEY, snapshot.grid_triplet).control_size(Size {
        width: 86,
        height: 28,
    })
}

fn freeze_toggle(snapshot: &XcopeUiState) -> Node {
    toggle(FREEZE_KEY, snapshot.freeze).control_size(Size {
        width: 86,
        height: 28,
    })
}

fn zoom_x_slider(snapshot: &XcopeUiState) -> Node {
    slider(
        ZOOM_X_KEY,
        snapshot.zoom_x,
        (crate::constants::ZOOM_MIN, crate::constants::ZOOM_MAX),
    )
    .control_size(Size {
        width: 180,
        height: 24,
    })
}

fn zoom_y_slider(snapshot: &XcopeUiState) -> Node {
    slider(
        ZOOM_Y_KEY,
        snapshot.zoom_y,
        (crate::constants::ZOOM_MIN, crate::constants::ZOOM_MAX),
    )
    .control_size(Size {
        width: 180,
        height: 24,
    })
}

fn channel_toggle(index: usize, snapshot: &XcopeUiState) -> Node {
    toggle(CHANNEL_VISIBLE_KEYS[index], snapshot.channel_visible[index]).control_size(Size {
        width: 60,
        height: 24,
    })
}

fn reset_zoom_button() -> Node {
    button(RESET_ZOOM_KEY)
        .button_label("RESET ZOOM")
        .control_size(Size {
            width: 144,
            height: 24,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{DisplayMode, GridSubdivision, ScopeMode, TimeWindow, XcopeUiState};
    use toybox::gui::declarative::measure_checked;

    #[test]
    fn build_ui_spec_uses_fixed_root_size() {
        let spec = build_ui_spec(&XcopeUiState::default(), Vec::new());
        let layout = spec.root.layout;
        assert_eq!(layout.min_width(), Some(WINDOW_WIDTH));
        assert_eq!(layout.min_height(), Some(WINDOW_HEIGHT));
    }

    #[test]
    fn selectors_reflect_state_indices() {
        let state = XcopeUiState {
            mode: ScopeMode::TempoLocked,
            time_window: TimeWindow::FourBars,
            display_mode: DisplayMode::Split,
            grid_subdivision: GridSubdivision::Div32,
            ..XcopeUiState::default()
        };

        let mode = mode_dropdown(&state);
        let window = window_dropdown(&state);
        let display = display_dropdown(&state);
        let grid = grid_dropdown(&state);

        let Node::Dropdown(mode) = mode else {
            panic!("mode should be dropdown")
        };
        let Node::Dropdown(window) = window else {
            panic!("window should be dropdown")
        };
        let Node::Dropdown(display) = display else {
            panic!("display should be dropdown")
        };
        let Node::Dropdown(grid) = grid else {
            panic!("grid should be dropdown")
        };

        assert_eq!(mode.selected, 1);
        assert_eq!(window.selected, 3);
        assert_eq!(display.selected, 1);
        assert_eq!(grid.selected, 2);
    }

    #[test]
    fn emitted_ui_spec_passes_strict_slot_validation() {
        let spec = build_ui_spec(&XcopeUiState::default(), Vec::new());
        measure_checked(&spec).expect("emitted tree must pass strict declarative validation");
    }
}
