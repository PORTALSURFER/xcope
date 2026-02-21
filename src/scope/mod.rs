//! Scope capture/windowing/render helpers.

mod capture_buffer;
mod mock_renderer;
mod render_sampling;
mod windowing;

pub use capture_buffer::{ScopeCaptureBuffer, ScopeFrame};
pub use mock_renderer::build_scope_surface_commands;
pub use render_sampling::{
    decimate_frame_channel, decimate_min_max, resample_frame_channel_linear, ColumnExtrema,
};
pub use windowing::{resolve_live_frame, resolve_scope_window_samples};
