//! Parameter/state model for the Xcope vertical-slice scaffold.

use std::array;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::constants::{
    ParamId, CHANNEL_COLOR_COUNT, MAX_VISUAL_CHANNELS, ZOOM_MAX, ZOOM_MIN, ZOOM_X_DEFAULT,
    ZOOM_Y_DEFAULT,
};

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
            channel_visible: [true, true, false, false],
            channel_color: [0, 1, 2, 3],
        }
    }
}

/// Shared atomic parameter store used by processor/controller/UI.
#[derive(Debug)]
pub struct XcopeParams {
    mode: AtomicU32,
    time_window: AtomicU32,
    grid_subdivision: AtomicU32,
    grid_triplet: AtomicBool,
    display_mode: AtomicU32,
    freeze: AtomicBool,
    zoom_x_bits: AtomicU32,
    zoom_y_bits: AtomicU32,
    channel_visible: [AtomicBool; MAX_VISUAL_CHANNELS],
    channel_color: [AtomicU32; MAX_VISUAL_CHANNELS],
}

impl Default for XcopeParams {
    fn default() -> Self {
        Self::new()
    }
}

impl XcopeParams {
    /// Create parameters initialized to V1 defaults.
    pub fn new() -> Self {
        let defaults = XcopeUiState::default();
        Self {
            mode: AtomicU32::new(defaults.mode.to_index()),
            time_window: AtomicU32::new(defaults.time_window.to_index()),
            grid_subdivision: AtomicU32::new(defaults.grid_subdivision.to_index()),
            grid_triplet: AtomicBool::new(defaults.grid_triplet),
            display_mode: AtomicU32::new(defaults.display_mode.to_index()),
            freeze: AtomicBool::new(defaults.freeze),
            zoom_x_bits: AtomicU32::new(defaults.zoom_x.to_bits()),
            zoom_y_bits: AtomicU32::new(defaults.zoom_y.to_bits()),
            channel_visible: array::from_fn(|index| {
                AtomicBool::new(defaults.channel_visible[index])
            }),
            channel_color: array::from_fn(|index| AtomicU32::new(defaults.channel_color[index])),
        }
    }

    /// Return a full UI-state snapshot.
    pub fn snapshot(&self) -> XcopeUiState {
        let channel_visible =
            array::from_fn(|index| self.channel_visible[index].load(Ordering::Relaxed));
        let channel_color = array::from_fn(|index| {
            clamp_color_index(self.channel_color[index].load(Ordering::Relaxed))
        });
        XcopeUiState {
            mode: ScopeMode::from_index(self.mode.load(Ordering::Relaxed)),
            time_window: TimeWindow::from_index(self.time_window.load(Ordering::Relaxed)),
            grid_subdivision: GridSubdivision::from_index(
                self.grid_subdivision.load(Ordering::Relaxed),
            ),
            grid_triplet: self.grid_triplet.load(Ordering::Relaxed),
            display_mode: DisplayMode::from_index(self.display_mode.load(Ordering::Relaxed)),
            freeze: self.freeze.load(Ordering::Relaxed),
            zoom_x: clamp_zoom(f32::from_bits(self.zoom_x_bits.load(Ordering::Relaxed))),
            zoom_y: clamp_zoom(f32::from_bits(self.zoom_y_bits.load(Ordering::Relaxed))),
            channel_visible,
            channel_color,
        }
    }

    /// Overwrite all persisted state fields.
    pub fn apply_snapshot(&self, state: &XcopeUiState) {
        self.mode.store(state.mode.to_index(), Ordering::Relaxed);
        self.time_window
            .store(state.time_window.to_index(), Ordering::Relaxed);
        self.grid_subdivision
            .store(state.grid_subdivision.to_index(), Ordering::Relaxed);
        self.grid_triplet
            .store(state.grid_triplet, Ordering::Relaxed);
        self.display_mode
            .store(state.display_mode.to_index(), Ordering::Relaxed);
        self.freeze.store(state.freeze, Ordering::Relaxed);
        self.zoom_x_bits
            .store(clamp_zoom(state.zoom_x).to_bits(), Ordering::Relaxed);
        self.zoom_y_bits
            .store(clamp_zoom(state.zoom_y).to_bits(), Ordering::Relaxed);
        for index in 0..MAX_VISUAL_CHANNELS {
            self.channel_visible[index].store(state.channel_visible[index], Ordering::Relaxed);
            self.channel_color[index].store(
                clamp_color_index(state.channel_color[index]),
                Ordering::Relaxed,
            );
        }
    }

    /// Set mode.
    pub fn set_mode(&self, mode: ScopeMode) {
        self.mode.store(mode.to_index(), Ordering::Relaxed);
    }

    /// Set time window.
    pub fn set_time_window(&self, window: TimeWindow) {
        self.time_window.store(window.to_index(), Ordering::Relaxed);
    }

    /// Set grid subdivision.
    pub fn set_grid_subdivision(&self, subdivision: GridSubdivision) {
        self.grid_subdivision
            .store(subdivision.to_index(), Ordering::Relaxed);
    }

    /// Set triplet toggle.
    pub fn set_grid_triplet(&self, enabled: bool) {
        self.grid_triplet.store(enabled, Ordering::Relaxed);
    }

    /// Set display mode.
    pub fn set_display_mode(&self, mode: DisplayMode) {
        self.display_mode.store(mode.to_index(), Ordering::Relaxed);
    }

    /// Set freeze state.
    pub fn set_freeze(&self, freeze: bool) {
        self.freeze.store(freeze, Ordering::Relaxed);
    }

    /// Set horizontal zoom.
    pub fn set_zoom_x(&self, zoom: f32) {
        self.zoom_x_bits
            .store(clamp_zoom(zoom).to_bits(), Ordering::Relaxed);
    }

    /// Set vertical zoom.
    pub fn set_zoom_y(&self, zoom: f32) {
        self.zoom_y_bits
            .store(clamp_zoom(zoom).to_bits(), Ordering::Relaxed);
    }

    /// Set per-channel visibility.
    pub fn set_channel_visible(&self, channel_index: usize, visible: bool) {
        if channel_index < MAX_VISUAL_CHANNELS {
            self.channel_visible[channel_index].store(visible, Ordering::Relaxed);
        }
    }

    /// Set per-channel color index.
    pub fn set_channel_color(&self, channel_index: usize, color_index: u32) {
        if channel_index < MAX_VISUAL_CHANNELS {
            self.channel_color[channel_index]
                .store(clamp_color_index(color_index), Ordering::Relaxed);
        }
    }
}

/// Clamp one zoom value into the supported range.
pub fn clamp_zoom(zoom: f32) -> f32 {
    if !zoom.is_finite() {
        return ZOOM_X_DEFAULT;
    }
    zoom.clamp(ZOOM_MIN, ZOOM_MAX)
}

/// Clamp one channel color index into the supported palette range.
pub fn clamp_color_index(color_index: u32) -> u32 {
    color_index.min(CHANNEL_COLOR_COUNT.saturating_sub(1))
}

/// Return the number of exposed parameters.
pub const fn param_count() -> u32 {
    ParamId::Channel4Color.raw()
}

/// Return one normalized parameter value.
pub fn read_param_normalized(params: &XcopeParams, param_id: u32) -> Option<f64> {
    let snapshot = params.snapshot();
    let value = match ParamId::from_raw(param_id)? {
        ParamId::ScopeMode => snapshot.mode.to_index() as f64,
        ParamId::TimeWindow => snapshot.time_window.to_index() as f64 / 3.0,
        ParamId::GridSubdivision => snapshot.grid_subdivision.to_index() as f64 / 2.0,
        ParamId::GridTriplet => bool_to_normalized(snapshot.grid_triplet),
        ParamId::DisplayMode => snapshot.display_mode.to_index() as f64,
        ParamId::Freeze => bool_to_normalized(snapshot.freeze),
        ParamId::ZoomX => zoom_to_normalized(snapshot.zoom_x),
        ParamId::ZoomY => zoom_to_normalized(snapshot.zoom_y),
        ParamId::Channel1Visible => bool_to_normalized(snapshot.channel_visible[0]),
        ParamId::Channel2Visible => bool_to_normalized(snapshot.channel_visible[1]),
        ParamId::Channel3Visible => bool_to_normalized(snapshot.channel_visible[2]),
        ParamId::Channel4Visible => bool_to_normalized(snapshot.channel_visible[3]),
        ParamId::Channel1Color => color_to_normalized(snapshot.channel_color[0]),
        ParamId::Channel2Color => color_to_normalized(snapshot.channel_color[1]),
        ParamId::Channel3Color => color_to_normalized(snapshot.channel_color[2]),
        ParamId::Channel4Color => color_to_normalized(snapshot.channel_color[3]),
    };
    Some(value.clamp(0.0, 1.0))
}

/// Apply one normalized parameter value.
pub fn apply_param_normalized(params: &XcopeParams, param_id: u32, normalized: f64) -> bool {
    let value = normalized.clamp(0.0, 1.0);
    let Some(param) = ParamId::from_raw(param_id) else {
        return false;
    };
    match param {
        ParamId::ScopeMode => params.set_mode(if value >= 0.5 {
            ScopeMode::TempoLocked
        } else {
            ScopeMode::FreeRunning
        }),
        ParamId::TimeWindow => params.set_time_window(match discrete_from_normalized(value, 4) {
            0 => TimeWindow::OneBeat,
            2 => TimeWindow::TwoBars,
            3 => TimeWindow::FourBars,
            _ => TimeWindow::OneBar,
        }),
        ParamId::GridSubdivision => {
            params.set_grid_subdivision(match discrete_from_normalized(value, 3) {
                0 => GridSubdivision::Div8,
                2 => GridSubdivision::Div32,
                _ => GridSubdivision::Div16,
            })
        }
        ParamId::GridTriplet => params.set_grid_triplet(value >= 0.5),
        ParamId::DisplayMode => params.set_display_mode(if value >= 0.5 {
            DisplayMode::Split
        } else {
            DisplayMode::Overlay
        }),
        ParamId::Freeze => params.set_freeze(value >= 0.5),
        ParamId::ZoomX => params.set_zoom_x(normalized_to_zoom(value)),
        ParamId::ZoomY => params.set_zoom_y(normalized_to_zoom(value)),
        ParamId::Channel1Visible => params.set_channel_visible(0, value >= 0.5),
        ParamId::Channel2Visible => params.set_channel_visible(1, value >= 0.5),
        ParamId::Channel3Visible => params.set_channel_visible(2, value >= 0.5),
        ParamId::Channel4Visible => params.set_channel_visible(3, value >= 0.5),
        ParamId::Channel1Color => params.set_channel_color(0, normalized_to_color(value)),
        ParamId::Channel2Color => params.set_channel_color(1, normalized_to_color(value)),
        ParamId::Channel3Color => params.set_channel_color(2, normalized_to_color(value)),
        ParamId::Channel4Color => params.set_channel_color(3, normalized_to_color(value)),
    }
    true
}

fn bool_to_normalized(value: bool) -> f64 {
    if value {
        1.0
    } else {
        0.0
    }
}

fn zoom_to_normalized(value: f32) -> f64 {
    ((clamp_zoom(value) - ZOOM_MIN) / (ZOOM_MAX - ZOOM_MIN)) as f64
}

fn normalized_to_zoom(value: f64) -> f32 {
    (ZOOM_MIN + (ZOOM_MAX - ZOOM_MIN) * value as f32).clamp(ZOOM_MIN, ZOOM_MAX)
}

fn color_to_normalized(value: u32) -> f64 {
    clamp_color_index(value) as f64 / (CHANNEL_COLOR_COUNT.saturating_sub(1).max(1) as f64)
}

fn normalized_to_color(value: f64) -> u32 {
    discrete_from_normalized(value, CHANNEL_COLOR_COUNT as usize) as u32
}

fn discrete_from_normalized(value: f64, options: usize) -> usize {
    let max_index = options.saturating_sub(1);
    ((value.clamp(0.0, 1.0) * max_index as f64).round() as usize).min(max_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_zoom_rejects_non_finite_values() {
        assert_eq!(clamp_zoom(f32::NAN), ZOOM_X_DEFAULT);
        assert_eq!(clamp_zoom(f32::INFINITY), ZOOM_X_DEFAULT);
    }

    #[test]
    fn apply_snapshot_clamps_zoom_and_colors() {
        let params = XcopeParams::new();
        let state = XcopeUiState {
            zoom_x: 999.0,
            zoom_y: 0.0,
            channel_color: [99, 42, 1, 0],
            ..XcopeUiState::default()
        };
        params.apply_snapshot(&state);

        let snapshot = params.snapshot();
        assert_eq!(snapshot.zoom_x, ZOOM_MAX);
        assert_eq!(snapshot.zoom_y, ZOOM_MIN);
        assert_eq!(snapshot.channel_color[0], CHANNEL_COLOR_COUNT - 1);
        assert_eq!(snapshot.channel_color[1], CHANNEL_COLOR_COUNT - 1);
    }

    #[test]
    fn snapshot_roundtrip_preserves_values() {
        let params = XcopeParams::new();
        params.set_mode(ScopeMode::TempoLocked);
        params.set_time_window(TimeWindow::FourBars);
        params.set_grid_subdivision(GridSubdivision::Div32);
        params.set_grid_triplet(true);
        params.set_display_mode(DisplayMode::Split);
        params.set_freeze(true);
        params.set_zoom_x(1.5);
        params.set_zoom_y(2.5);
        params.set_channel_visible(3, true);
        params.set_channel_color(2, 5);

        let snapshot = params.snapshot();
        assert_eq!(snapshot.mode, ScopeMode::TempoLocked);
        assert_eq!(snapshot.time_window, TimeWindow::FourBars);
        assert_eq!(snapshot.grid_subdivision, GridSubdivision::Div32);
        assert!(snapshot.grid_triplet);
        assert_eq!(snapshot.display_mode, DisplayMode::Split);
        assert!(snapshot.freeze);
        assert_eq!(snapshot.zoom_x, 1.5);
        assert_eq!(snapshot.zoom_y, 2.5);
        assert!(snapshot.channel_visible[3]);
        assert_eq!(snapshot.channel_color[2], 5);
    }

    #[test]
    fn normalized_param_roundtrip_updates_zoom() {
        let params = XcopeParams::new();
        assert!(apply_param_normalized(&params, ParamId::ZoomX.raw(), 0.75));
        let normalized = read_param_normalized(&params, ParamId::ZoomX.raw())
            .expect("zoom param should be readable");
        assert!((normalized - 0.75).abs() < 0.01);
    }
}
