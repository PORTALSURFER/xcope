//! Scope window-selection helpers.

use crate::params::XcopeUiState;
use crate::scope::{ScopeCaptureBuffer, ScopeFrame};
use crate::transport::{resolve_visible_sample_count, TransportSnapshot};

/// Resolve visible sample count for current scope settings.
pub fn resolve_scope_window_samples(
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
    sample_rate_hz: f32,
) -> usize {
    resolve_visible_sample_count(ui_state, transport, sample_rate_hz)
}

/// Resolve one live frame from the capture ring.
pub fn resolve_live_frame(
    capture: &ScopeCaptureBuffer,
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
    sample_rate_hz: f32,
) -> ScopeFrame {
    let samples = resolve_scope_window_samples(ui_state, transport, sample_rate_hz);
    capture.snapshot_recent(samples)
}
