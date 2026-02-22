//! Scope capture/windowing/render helpers.

mod capture_buffer;
mod mock_renderer;
mod windowing;

pub use capture_buffer::{ScopeCaptureBuffer, ScopeFrame};
pub use mock_renderer::build_scope_surface_commands;
pub use windowing::{resolve_live_frame, resolve_live_view, resolve_scope_window_samples};
