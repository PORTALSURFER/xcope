//! GUI composition (declarative).

use toybox::gui::declarative::{panel, root_frame_sized, textbox, UiSpec};
use toybox::gui::Size;

/// Default window width (logical px).
pub const WINDOW_WIDTH: u32 = 520;
/// Default window height (logical px).
pub const WINDOW_HEIGHT: u32 = 320;

/// Build the initial UI spec for this plugin.
pub fn build_spec() -> UiSpec {
    UiSpec::new(root_frame_sized(
        "root",
        panel("panel", textbox("TODO: build UI")),
        Size {
            width: WINDOW_WIDTH,
            height: WINDOW_HEIGHT,
        },
    ))
}

#[cfg(all(test, feature = "screenshot-test"))]
mod screenshot_tests {
    use toybox::gui::{screenshot_harness, Size};

    use super::{build_spec, WINDOW_HEIGHT, WINDOW_WIDTH};

    #[test]
    fn screenshot_renders_initial_ui() {
        screenshot_harness::capture_initial_ui_screenshots_if_enabled(
            env!("CARGO_PKG_NAME"),
            Size {
                width: WINDOW_WIDTH,
                height: WINDOW_HEIGHT,
            },
            |_input| build_spec(),
        )
        .expect("failed to capture headless screenshots");
    }
}
