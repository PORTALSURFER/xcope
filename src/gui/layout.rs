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
pub const CHANNEL_VISIBLE_KEYS: [&str; 2] = ["ch1-visible", "ch2-visible"];
/// Channel color-dropdown keys.
pub const CHANNEL_COLOR_KEYS: [&str; 2] = ["ch1-color", "ch2-color"];

/// Toolbar region fixed design-space height.
pub const TOOLBAR_HEIGHT: u32 = 84;
/// Bottom control-bar fixed design-space height.
pub const BOTTOM_BAR_HEIGHT: u32 = 96;
/// Scope region fixed design-space height.
pub const SCOPE_HEIGHT: u32 = WINDOW_HEIGHT - TOOLBAR_HEIGHT - BOTTOM_BAR_HEIGHT;
const TOTAL_VERTICAL_WEIGHT: u32 = TOOLBAR_HEIGHT + SCOPE_HEIGHT + BOTTOM_BAR_HEIGHT;

/// Runtime-resolved layout geometry for the current editor size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutGeometry {
    /// Root editor size in pixels.
    pub root_size: Size,
    /// Toolbar region height in pixels.
    pub toolbar_height: u32,
    /// Scope surface region height in pixels.
    pub scope_height: u32,
    /// Bottom control-bar region height in pixels.
    pub bottom_bar_height: u32,
}

/// Resolve runtime layout geometry from one host-provided window size.
pub fn resolve_layout_geometry(window_size: Size) -> LayoutGeometry {
    let root_size = Size {
        width: window_size.width.max(WINDOW_WIDTH),
        height: window_size.height.max(WINDOW_HEIGHT),
    };
    let toolbar_height = proportional_height(root_size.height, TOOLBAR_HEIGHT).max(1);
    let bottom_bar_height = proportional_height(root_size.height, BOTTOM_BAR_HEIGHT).max(1);
    let reserved = toolbar_height
        .saturating_add(bottom_bar_height)
        .min(root_size.height.saturating_sub(1));
    let scope_height = root_size.height.saturating_sub(reserved).max(1);

    LayoutGeometry {
        root_size,
        toolbar_height,
        scope_height,
        bottom_bar_height,
    }
}

/// Build one full UI spec from state and rendered scope commands.
pub fn build_ui_spec(
    snapshot: &XcopeUiState,
    scope_commands: Vec<SurfaceCommand>,
    geometry: LayoutGeometry,
) -> UiSpec {
    let toolbar = panel("toolbar", toolbar_row(snapshot));
    let scope = panel(
        "scope-panel",
        surface(
            SCOPE_SURFACE_KEY,
            Size {
                width: geometry.root_size.width,
                height: geometry.scope_height,
            },
            scope_commands,
        )
        .fill(),
    );
    let bottom = panel("bottom-bar", bottom_row(snapshot));

    let content = column_slots(vec![
        weighted_slot(toolbar, slot_weight(geometry.toolbar_height)),
        weighted_slot(scope, slot_weight(geometry.scope_height)),
        weighted_slot(bottom, slot_weight(geometry.bottom_bar_height)),
    ]);

    UiSpec::new(root_frame_sized(ROOT_KEY, content, geometry.root_size))
}

fn proportional_height(total_height: u32, weight: u32) -> u32 {
    ((total_height as u64).saturating_mul(weight as u64) / TOTAL_VERTICAL_WEIGHT as u64) as u32
}

fn slot_weight(height: u32) -> u16 {
    height.clamp(1, u16::MAX as u32) as u16
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
        weighted_slot(toolbar_field("CH1", channel_control(0, snapshot)), 22),
        weighted_slot(toolbar_field("CH2", channel_control(1, snapshot)), 22),
        weighted_slot(toolbar_field("", reset_zoom_button()), 56),
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
        height: 12,
    })
}

fn channel_color_dropdown(index: usize, snapshot: &XcopeUiState) -> Node {
    dropdown(
        CHANNEL_COLOR_KEYS[index],
        crate::constants::CHANNEL_COLOR_COUNT as usize,
        snapshot.channel_color[index] as usize,
    )
    .dropdown_option_labels(vec![
        "AQUA".into(),
        "ORNG".into(),
        "LIME".into(),
        "LILA".into(),
        "GOLD".into(),
        "INDG".into(),
        "PINK".into(),
        "MINT".into(),
    ])
    .control_size(Size {
        width: 60,
        height: 12,
    })
}

fn channel_control(index: usize, snapshot: &XcopeUiState) -> Node {
    column_slots(vec![
        weighted_slot(channel_toggle(index, snapshot), 50),
        weighted_slot(channel_color_dropdown(index, snapshot), 50),
    ])
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
    fn build_ui_spec_uses_resolved_root_size() {
        let geometry = resolve_layout_geometry(Size {
            width: 1200,
            height: 700,
        });
        let spec = build_ui_spec(&XcopeUiState::default(), Vec::new(), geometry);
        let layout = spec.root.layout;
        assert_eq!(layout.min_width(), Some(1200));
        assert_eq!(layout.min_height(), Some(700));
    }

    #[test]
    fn resolve_layout_geometry_clamps_to_minimum_window_size() {
        let geometry = resolve_layout_geometry(Size::default());
        assert_eq!(geometry.root_size.width, WINDOW_WIDTH);
        assert_eq!(geometry.root_size.height, WINDOW_HEIGHT);
        assert!(geometry.scope_height > 0);
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
        let ch2_color = channel_color_dropdown(1, &state);

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
        let Node::Dropdown(ch2_color) = ch2_color else {
            panic!("channel color should be dropdown")
        };

        assert_eq!(mode.selected, 1);
        assert_eq!(window.selected, 3);
        assert_eq!(display.selected, 1);
        assert_eq!(grid.selected, 2);
        assert_eq!(ch2_color.selected, state.channel_color[1] as usize);
    }

    #[test]
    fn emitted_ui_spec_passes_strict_slot_validation() {
        let spec = build_ui_spec(
            &XcopeUiState::default(),
            Vec::new(),
            resolve_layout_geometry(Size {
                width: WINDOW_WIDTH,
                height: WINDOW_HEIGHT,
            }),
        );
        measure_checked(&spec).expect("emitted tree must pass strict declarative validation");
    }
}
