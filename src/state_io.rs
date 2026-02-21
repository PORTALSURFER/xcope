//! Versioned state payload helpers for Xcope.

use crate::params::{
    clamp_color_index, clamp_zoom, DisplayMode, GridSubdivision, ScopeMode, TimeWindow,
    XcopeParams, XcopeUiState,
};

const PAYLOAD_LEN_V1: usize = 1 + 1 + 1 + 1 + 1 + 1 + 4 + 4 + 4 + 4;

/// Persisted V1 state payload.
#[derive(Clone, Debug, PartialEq)]
pub struct XcopeStateV1 {
    /// Scope mode.
    pub mode: ScopeMode,
    /// Visible time window.
    pub time_window: TimeWindow,
    /// Grid subdivision selector.
    pub grid_subdivision: GridSubdivision,
    /// Triplet subdivision toggle.
    pub grid_triplet: bool,
    /// Display mode selector.
    pub display_mode: DisplayMode,
    /// Freeze toggle.
    pub freeze: bool,
    /// Horizontal zoom.
    pub zoom_x: f32,
    /// Vertical zoom.
    pub zoom_y: f32,
    /// Per-channel visibility flags.
    pub channel_visible: [bool; 4],
    /// Per-channel color indices.
    pub channel_color: [u32; 4],
}

impl Default for XcopeStateV1 {
    fn default() -> Self {
        Self::from_ui_state(&XcopeUiState::default())
    }
}

impl XcopeStateV1 {
    /// Create persisted state from one runtime UI snapshot.
    pub fn from_ui_state(state: &XcopeUiState) -> Self {
        Self {
            mode: state.mode,
            time_window: state.time_window,
            grid_subdivision: state.grid_subdivision,
            grid_triplet: state.grid_triplet,
            display_mode: state.display_mode,
            freeze: state.freeze,
            zoom_x: clamp_zoom(state.zoom_x),
            zoom_y: clamp_zoom(state.zoom_y),
            channel_visible: state.channel_visible,
            channel_color: state.channel_color.map(clamp_color_index),
        }
    }

    /// Convert persisted state to runtime UI snapshot.
    pub fn to_ui_state(&self) -> XcopeUiState {
        XcopeUiState {
            mode: self.mode,
            time_window: self.time_window,
            grid_subdivision: self.grid_subdivision,
            grid_triplet: self.grid_triplet,
            display_mode: self.display_mode,
            freeze: self.freeze,
            zoom_x: clamp_zoom(self.zoom_x),
            zoom_y: clamp_zoom(self.zoom_y),
            channel_visible: self.channel_visible,
            channel_color: self.channel_color.map(clamp_color_index),
        }
    }

    /// Encode this state into a fixed-size payload.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(PAYLOAD_LEN_V1);
        out.push(self.mode.to_index() as u8);
        out.push(self.time_window.to_index() as u8);
        out.push(self.grid_subdivision.to_index() as u8);
        out.push(u8::from(self.grid_triplet));
        out.push(self.display_mode.to_index() as u8);
        out.push(u8::from(self.freeze));
        out.extend_from_slice(&clamp_zoom(self.zoom_x).to_le_bytes());
        out.extend_from_slice(&clamp_zoom(self.zoom_y).to_le_bytes());
        for visible in self.channel_visible {
            out.push(u8::from(visible));
        }
        for color_index in self.channel_color {
            out.push(clamp_color_index(color_index) as u8);
        }
        out
    }

    /// Decode one state payload.
    pub fn decode(payload: &[u8]) -> Result<Self, String> {
        if payload.len() != PAYLOAD_LEN_V1 {
            return Err(format!(
                "invalid payload length: expected {PAYLOAD_LEN_V1}, got {}",
                payload.len()
            ));
        }

        let mut offset = 0usize;
        let read_u8 = |payload: &[u8], offset: &mut usize| {
            let value = payload[*offset];
            *offset += 1;
            value
        };
        let read_f32 = |payload: &[u8], offset: &mut usize| {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&payload[*offset..*offset + 4]);
            *offset += 4;
            f32::from_le_bytes(bytes)
        };

        let mode = ScopeMode::from_index(read_u8(payload, &mut offset) as u32);
        let time_window = TimeWindow::from_index(read_u8(payload, &mut offset) as u32);
        let grid_subdivision = GridSubdivision::from_index(read_u8(payload, &mut offset) as u32);
        let grid_triplet = read_u8(payload, &mut offset) != 0;
        let display_mode = DisplayMode::from_index(read_u8(payload, &mut offset) as u32);
        let freeze = read_u8(payload, &mut offset) != 0;
        let zoom_x = clamp_zoom(read_f32(payload, &mut offset));
        let zoom_y = clamp_zoom(read_f32(payload, &mut offset));
        let channel_visible = std::array::from_fn(|_| read_u8(payload, &mut offset) != 0);
        let channel_color =
            std::array::from_fn(|_| clamp_color_index(read_u8(payload, &mut offset) as u32));

        Ok(Self {
            mode,
            time_window,
            grid_subdivision,
            grid_triplet,
            display_mode,
            freeze,
            zoom_x,
            zoom_y,
            channel_visible,
            channel_color,
        })
    }
}

/// Encode a versioned payload from current parameter values.
pub fn encode_state_payload(params: &XcopeParams) -> Vec<u8> {
    XcopeStateV1::from_ui_state(&params.snapshot()).encode()
}

/// Decode and apply one versioned payload to the parameter store.
pub fn decode_state_payload(params: &XcopeParams, payload: &[u8]) -> Result<(), String> {
    let state = XcopeStateV1::decode(payload)?;
    params.apply_snapshot(&state.to_ui_state());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip_preserves_fields() {
        let state = XcopeStateV1 {
            mode: ScopeMode::TempoLocked,
            time_window: TimeWindow::FourBars,
            grid_subdivision: GridSubdivision::Div32,
            grid_triplet: true,
            display_mode: DisplayMode::Split,
            freeze: true,
            zoom_x: 1.75,
            zoom_y: 2.25,
            channel_visible: [true, false, true, true],
            channel_color: [1, 3, 5, 7],
        };

        let encoded = state.encode();
        let decoded = XcopeStateV1::decode(&encoded).expect("state should decode");
        assert_eq!(decoded, state);
    }

    #[test]
    fn state_decode_rejects_invalid_length() {
        let err = XcopeStateV1::decode(&[0u8; 3]).expect_err("decode should fail");
        assert!(err.contains("invalid payload length"));
    }

    #[test]
    fn decode_state_payload_applies_snapshot() {
        let params = XcopeParams::new();
        let state = XcopeStateV1 {
            mode: ScopeMode::TempoLocked,
            time_window: TimeWindow::TwoBars,
            grid_subdivision: GridSubdivision::Div8,
            grid_triplet: true,
            display_mode: DisplayMode::Split,
            freeze: true,
            zoom_x: 1.2,
            zoom_y: 2.4,
            channel_visible: [true, true, true, false],
            channel_color: [0, 1, 2, 3],
        };

        decode_state_payload(&params, &state.encode()).expect("payload should apply");
        let snapshot = params.snapshot();
        assert_eq!(snapshot.mode, ScopeMode::TempoLocked);
        assert_eq!(snapshot.time_window, TimeWindow::TwoBars);
        assert_eq!(snapshot.grid_subdivision, GridSubdivision::Div8);
        assert!(snapshot.grid_triplet);
        assert_eq!(snapshot.display_mode, DisplayMode::Split);
        assert!(snapshot.freeze);
        assert_eq!(snapshot.zoom_x, 1.2);
        assert_eq!(snapshot.zoom_y, 2.4);
        assert_eq!(snapshot.channel_visible, [true, true, true, false]);
    }
}
