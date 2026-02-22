//! Parameter/state model for the Xcope vertical-slice scaffold.

mod normalized;
mod store;
mod types;

pub use normalized::{apply_param_normalized, param_count, read_param_normalized};
pub use store::{clamp_color_index, clamp_zoom, XcopeParams};
pub use types::{DisplayMode, GridSubdivision, ScopeMode, TimeWindow, XcopeUiState};
