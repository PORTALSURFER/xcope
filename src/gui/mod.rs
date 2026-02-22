//! Xcope GUI host-wrapper and state reducer wiring.

mod layout;
mod reducer;

pub use layout::SCOPE_SURFACE_KEY;

use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use toybox::clack_extensions::gui::{GuiSize, Window};
use toybox::clack_plugin::plugin::PluginError;
use toybox::clap::gui::{GuiHostWindow, GuiOpenRequest, InputState};
use toybox::gui::declarative::{UiAction, UiSpec};
use toybox::gui::Size;
use toybox::raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use crate::constants::{WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::scope::{build_scope_surface_commands, resolve_live_view, ScopeFrame};
use crate::XcopeShared;

/// Frame pacing target for expensive scope frame rebuilds.
const UI_FRAME_INTERVAL: Duration = Duration::from_micros(16_666);

/// Host-window wrapper for the Xcope editor.
#[derive(Default)]
pub struct XcopeGui {
    window: GuiHostWindow,
}

impl XcopeGui {
    /// Attach a raw host window handle.
    pub fn set_parent_raw(&mut self, parent: RawWindowHandle) {
        self.window.set_parent(parent);
    }

    /// Attach a CLAP-compatible host parent window.
    pub fn set_parent(&mut self, window: Window<'_>) {
        self.set_parent_raw(window.raw_window_handle());
    }

    /// Open Xcope editor for one shared runtime.
    pub fn open(&mut self, shared: Arc<XcopeShared>) -> Result<(), PluginError> {
        self.window
            .set_aspect_ratio(Some(WINDOW_WIDTH as f32 / WINDOW_HEIGHT as f32));
        let state = GuiState::new(shared);
        let open_size = preferred_window_size();

        self.window
            .open_parented_with(GuiOpenRequest::<GuiState, _, _, _>::new(
                "xcope".to_string(),
                open_size,
                state,
                Box::new(|_state: &mut GuiState| {}),
                Box::new(|input: &InputState, state: &GuiState| state.build_ui(input)),
                Box::new(|state: &mut GuiState, action: UiAction| state.reduce_action(action)),
            ))
    }

    /// Apply one host size request.
    pub fn request_resize(&self, width: u32, height: u32) {
        self.window.request_resize(width, height);
    }

    /// Close the editor if it is open.
    pub fn close(&mut self) {
        self.window.hide();
    }

    /// Return the last known logical editor size.
    pub fn last_size(&self) -> Option<(u32, u32)> {
        self.window.last_size()
    }

    /// Return true if host-driven resize is enabled.
    pub fn host_resize_enabled(&self) -> bool {
        self.window.host_resize_enabled()
    }

    /// Clamp one host-provided size against Xcope minimums.
    pub fn adjust_host_size(&self, size: GuiSize) -> Option<GuiSize> {
        self.window
            .adjust_host_size(size)
            .map(constrained_host_size)
    }

    /// Apply one host-provided size to the native editor window.
    pub fn apply_host_size(&self, size: GuiSize) {
        self.window.apply_host_size(constrained_host_size(size));
    }
}

/// Return preferred default editor size.
pub fn preferred_window_size() -> (u32, u32) {
    (WINDOW_WIDTH, WINDOW_HEIGHT)
}

fn constrained_host_size(size: GuiSize) -> GuiSize {
    GuiSize {
        width: size.width.max(WINDOW_WIDTH),
        height: size.height.max(WINDOW_HEIGHT),
    }
}

struct GuiState {
    shared: Arc<XcopeShared>,
    runtime: Mutex<GuiRuntime>,
}

#[derive(Debug, Default)]
struct GuiRuntime {
    frozen_frame: Option<ScopeFrame>,
    last_live_frame: ScopeFrame,
    cached_ui_spec: Option<UiSpec>,
    last_ui_build_at: Option<Instant>,
    last_window_size: Option<Size>,
    ui_dirty: bool,
}

impl GuiState {
    fn new(shared: Arc<XcopeShared>) -> Self {
        Self {
            shared,
            runtime: Mutex::new(GuiRuntime::default()),
        }
    }

    fn build_ui(&self, input: &InputState) -> toybox::gui::declarative::UiSpec {
        let geometry = layout::resolve_layout_geometry(input.window_size);
        let now = Instant::now();
        if let Some(spec) = self.try_take_cached_ui_spec(now, geometry.root_size) {
            return spec;
        }

        let snapshot = self.shared.params.snapshot();
        let transport = self.shared.transport.snapshot();
        let sample_rate = self.shared.sample_rate_hz();
        let resolved_view = resolve_live_view(
            self.shared.scope_buffer.as_ref(),
            &snapshot,
            transport,
            sample_rate,
        );
        let live_frame = resolved_view.frame;
        let render_transport = resolved_view.render_transport;
        let frame = if let Ok(mut runtime) = self.runtime.lock() {
            runtime.last_live_frame = live_frame.clone();
            if snapshot.freeze {
                if runtime.frozen_frame.is_none() {
                    runtime.frozen_frame = Some(live_frame.clone());
                }
                runtime
                    .frozen_frame
                    .clone()
                    .unwrap_or_else(|| live_frame.clone())
            } else {
                runtime.frozen_frame = None;
                live_frame
            }
        } else {
            live_frame
        };
        let commands = build_scope_surface_commands(
            &frame,
            &snapshot,
            render_transport,
            geometry.root_size.width,
            geometry.scope_height,
        );
        let spec = layout::build_ui_spec(&snapshot, commands, geometry);
        self.cache_ui_spec(now, geometry.root_size, &spec);
        spec
    }

    fn try_take_cached_ui_spec(&self, now: Instant, window_size: Size) -> Option<UiSpec> {
        let Ok(runtime) = self.runtime.lock() else {
            return None;
        };
        if runtime.ui_dirty {
            return None;
        }
        if runtime.last_window_size != Some(window_size) {
            return None;
        }
        let built_at = runtime.last_ui_build_at?;
        if now.duration_since(built_at) < UI_FRAME_INTERVAL {
            return runtime.cached_ui_spec.clone();
        }
        None
    }

    fn cache_ui_spec(&self, now: Instant, window_size: Size, spec: &UiSpec) {
        let Ok(mut runtime) = self.runtime.lock() else {
            return;
        };
        runtime.cached_ui_spec = Some(spec.clone());
        runtime.last_ui_build_at = Some(now);
        runtime.last_window_size = Some(window_size);
        runtime.ui_dirty = false;
    }

    fn reduce_action(&mut self, action: UiAction) {
        let freeze_changed = reducer::apply_ui_action(self.shared.params.as_ref(), action);
        let snapshot = self.shared.params.snapshot();
        let Ok(mut runtime) = self.runtime.lock() else {
            return;
        };
        runtime.ui_dirty = true;
        runtime.cached_ui_spec = None;
        runtime.last_ui_build_at = None;
        runtime.last_window_size = None;
        if snapshot.freeze {
            if (freeze_changed || runtime.frozen_frame.is_none())
                && runtime.last_live_frame.sample_count() > 0
            {
                runtime.frozen_frame = Some(runtime.last_live_frame.clone());
            }
        } else {
            runtime.frozen_frame = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::ScopeMode;

    #[test]
    fn preferred_window_size_matches_constants() {
        assert_eq!(preferred_window_size(), (WINDOW_WIDTH, WINDOW_HEIGHT));
    }

    #[test]
    fn build_ui_reflects_parameter_changes() {
        let shared = Arc::new(XcopeShared::new());
        shared.params.set_mode(ScopeMode::TempoLocked);

        let state = GuiState::new(shared);
        let spec = state.build_ui(&InputState::default());
        assert_eq!(spec.root.key, layout::ROOT_KEY);
    }
}
