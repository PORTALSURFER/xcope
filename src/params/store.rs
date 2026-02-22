//! Atomic parameter storage and clamping helpers.

use std::array;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::constants::{
    CHANNEL_COLOR_COUNT, MAX_VISUAL_CHANNELS, ZOOM_MAX, ZOOM_MIN, ZOOM_X_DEFAULT,
};

use super::{DisplayMode, GridSubdivision, ScopeMode, TimeWindow, XcopeUiState};

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
            channel_color: [99, 42],
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
        params.set_channel_visible(1, true);
        params.set_channel_color(1, 5);

        let snapshot = params.snapshot();
        assert_eq!(snapshot.mode, ScopeMode::TempoLocked);
        assert_eq!(snapshot.time_window, TimeWindow::FourBars);
        assert_eq!(snapshot.grid_subdivision, GridSubdivision::Div32);
        assert!(snapshot.grid_triplet);
        assert_eq!(snapshot.display_mode, DisplayMode::Split);
        assert!(snapshot.freeze);
        assert_eq!(snapshot.zoom_x, 1.5);
        assert_eq!(snapshot.zoom_y, 2.5);
        assert!(snapshot.channel_visible[1]);
        assert_eq!(snapshot.channel_color[1], 5);
    }
}
