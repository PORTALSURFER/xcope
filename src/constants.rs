//! Shared constants for the Xcope vertical-slice scaffold.

/// Human-readable plugin name shown in hosts.
pub const PLUGIN_NAME: &str = "xcope";
/// Stable plugin identifier string.
pub const PLUGIN_ID: &str = "com.portalsurfer.xcope";

/// Default logical editor width in design-space pixels.
pub const WINDOW_WIDTH: u32 = 880;
/// Default logical editor height in design-space pixels.
pub const WINDOW_HEIGHT: u32 = 500;

/// State payload magic (`XCOP`).
pub const STATE_MAGIC: u32 = u32::from_le_bytes(*b"XCOP");
/// State payload format version.
pub const STATE_VERSION: u32 = 1;

/// Maximum number of channels visualized at once.
pub const MAX_VISUAL_CHANNELS: usize = 4;
/// Number of color slots available for channel tint selection.
pub const CHANNEL_COLOR_COUNT: u32 = 8;

/// Minimum zoom multiplier.
pub const ZOOM_MIN: f32 = 0.25;
/// Maximum zoom multiplier.
pub const ZOOM_MAX: f32 = 4.0;
/// Default horizontal zoom multiplier.
pub const ZOOM_X_DEFAULT: f32 = 1.0;
/// Default vertical zoom multiplier.
pub const ZOOM_Y_DEFAULT: f32 = 1.0;

/// Minimum scope window sample count.
pub const MIN_SCOPE_WINDOW_SAMPLES: usize = 64;
/// Maximum scope window sample count.
pub const MAX_SCOPE_WINDOW_SAMPLES: usize = 262_144;
/// Ring-buffer capacity used by the real-time scope capture path.
pub const CAPTURE_BUFFER_CAPACITY: usize = 262_144;

/// Parameter ids used by the VST3 controller and processor.
#[allow(clippy::enum_clike_unportable_variant)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ParamId {
    /// Scope mode (free-running or tempo-locked).
    ScopeMode = 1,
    /// Visible musical window length selector.
    TimeWindow = 2,
    /// Grid subdivision selector.
    GridSubdivision = 3,
    /// Grid triplet toggle.
    GridTriplet = 4,
    /// Channel display mode selector (overlay/split).
    DisplayMode = 5,
    /// Freeze toggle.
    Freeze = 6,
    /// Horizontal zoom factor.
    ZoomX = 7,
    /// Vertical zoom factor.
    ZoomY = 8,
    /// Channel 1 visibility.
    Channel1Visible = 9,
    /// Channel 2 visibility.
    Channel2Visible = 10,
    /// Channel 3 visibility.
    Channel3Visible = 11,
    /// Channel 4 visibility.
    Channel4Visible = 12,
    /// Channel 1 color palette index.
    Channel1Color = 13,
    /// Channel 2 color palette index.
    Channel2Color = 14,
    /// Channel 3 color palette index.
    Channel3Color = 15,
    /// Channel 4 color palette index.
    Channel4Color = 16,
}

impl ParamId {
    /// Return the stable id as a raw `u32`.
    pub const fn raw(self) -> u32 {
        self as u32
    }

    /// Resolve one parameter id from its raw `u32` representation.
    pub const fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            1 => Some(Self::ScopeMode),
            2 => Some(Self::TimeWindow),
            3 => Some(Self::GridSubdivision),
            4 => Some(Self::GridTriplet),
            5 => Some(Self::DisplayMode),
            6 => Some(Self::Freeze),
            7 => Some(Self::ZoomX),
            8 => Some(Self::ZoomY),
            9 => Some(Self::Channel1Visible),
            10 => Some(Self::Channel2Visible),
            11 => Some(Self::Channel3Visible),
            12 => Some(Self::Channel4Visible),
            13 => Some(Self::Channel1Color),
            14 => Some(Self::Channel2Color),
            15 => Some(Self::Channel3Color),
            16 => Some(Self::Channel4Color),
            _ => None,
        }
    }
}
