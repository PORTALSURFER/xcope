//! Enumerated parameter types and UI-state snapshot for Xcope.

use crate::constants::{MAX_VISUAL_CHANNELS, ZOOM_X_DEFAULT, ZOOM_Y_DEFAULT};

/// Oscilloscope runtime mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeMode {
    /// Continuous scrolling, independent of host tempo.
    FreeRunning,
    /// Host-tempo aligned display window.
    TempoLocked,
}

impl ScopeMode {
    /// Return a compact persisted representation.
    pub const fn to_index(self) -> u32 {
        match self {
            Self::FreeRunning => 0,
            Self::TempoLocked => 1,
        }
    }

    /// Resolve one mode from its persisted representation.
    pub const fn from_index(value: u32) -> Self {
        match value {
            1 => Self::TempoLocked,
            _ => Self::FreeRunning,
        }
    }
}

/// Visible musical window length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeWindow {
    /// One beat view.
    OneBeat,
    /// One bar view.
    OneBar,
    /// Two bars view.
    TwoBars,
    /// Four bars view.
    FourBars,
}

impl TimeWindow {
    /// Return a compact persisted representation.
    pub const fn to_index(self) -> u32 {
        match self {
            Self::OneBeat => 0,
            Self::OneBar => 1,
            Self::TwoBars => 2,
            Self::FourBars => 3,
        }
    }

    /// Resolve one window from its persisted representation.
    pub const fn from_index(value: u32) -> Self {
        match value {
            0 => Self::OneBeat,
            2 => Self::TwoBars,
            3 => Self::FourBars,
            _ => Self::OneBar,
        }
    }
}

/// Grid subdivision density.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridSubdivision {
    /// Eighth-note subdivision.
    Div8,
    /// Sixteenth-note subdivision.
    Div16,
    /// Thirty-second-note subdivision.
    Div32,
}

impl GridSubdivision {
    /// Return a compact persisted representation.
    pub const fn to_index(self) -> u32 {
        match self {
            Self::Div8 => 0,
            Self::Div16 => 1,
            Self::Div32 => 2,
        }
    }

    /// Resolve one subdivision from its persisted representation.
    pub const fn from_index(value: u32) -> Self {
        match value {
            0 => Self::Div8,
            2 => Self::Div32,
            _ => Self::Div16,
        }
    }
}

/// Channel arrangement mode in the scope display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    /// Render visible channels overlaid in one region.
    Overlay,
    /// Render visible channels in vertically split lanes.
    Split,
}

impl DisplayMode {
    /// Return a compact persisted representation.
    pub const fn to_index(self) -> u32 {
        match self {
            Self::Overlay => 0,
            Self::Split => 1,
        }
    }

    /// Resolve one display mode from its persisted representation.
    pub const fn from_index(value: u32) -> Self {
        match value {
            1 => Self::Split,
            _ => Self::Overlay,
        }
    }
}

/// Snapshot of core user-facing UI state.
#[derive(Clone, Debug, PartialEq)]
pub struct XcopeUiState {
    /// Current scope mode.
    pub mode: ScopeMode,
    /// Current time window preset.
    pub time_window: TimeWindow,
    /// Current grid subdivision.
    pub grid_subdivision: GridSubdivision,
    /// Triplet subdivision toggle.
    pub grid_triplet: bool,
    /// Current display mode.
    pub display_mode: DisplayMode,
    /// Freeze flag.
    pub freeze: bool,
    /// Horizontal zoom multiplier.
    pub zoom_x: f32,
    /// Vertical zoom multiplier.
    pub zoom_y: f32,
    /// Per-channel visibility flags.
    pub channel_visible: [bool; MAX_VISUAL_CHANNELS],
    /// Per-channel color palette index.
    pub channel_color: [u32; MAX_VISUAL_CHANNELS],
}

impl Default for XcopeUiState {
    fn default() -> Self {
        Self {
            mode: ScopeMode::FreeRunning,
            time_window: TimeWindow::OneBar,
            grid_subdivision: GridSubdivision::Div16,
            grid_triplet: false,
            display_mode: DisplayMode::Overlay,
            freeze: false,
            zoom_x: ZOOM_X_DEFAULT,
            zoom_y: ZOOM_Y_DEFAULT,
            channel_visible: [true, true],
            channel_color: [0, 1],
        }
    }
}
