//! Transport/timing model used by scope window calculations.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::constants::{MAX_SCOPE_WINDOW_SAMPLES, MIN_SCOPE_WINDOW_SAMPLES};
use crate::params::{ScopeMode, TimeWindow, XcopeUiState};

const DEFAULT_TEMPO_BPM: f32 = 120.0;
const NONE_BEATS_SENTINEL: u64 = u64::MAX;

/// Immutable transport snapshot consumed by UI/timing code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransportSnapshot {
    /// Host tempo in beats-per-minute.
    pub tempo_bpm: f32,
    /// Host playback state.
    pub is_playing: bool,
    /// Song position in beat units (quarter-note beats), if provided.
    pub song_pos_beats: Option<f64>,
    /// Time-signature numerator.
    pub time_sig_num: u16,
    /// Time-signature denominator.
    pub time_sig_denom: u16,
}

impl Default for TransportSnapshot {
    fn default() -> Self {
        Self {
            tempo_bpm: DEFAULT_TEMPO_BPM,
            is_playing: false,
            song_pos_beats: None,
            time_sig_num: 4,
            time_sig_denom: 4,
        }
    }
}

/// Shared atomic transport mirror updated by the audio thread.
#[derive(Debug)]
pub struct TransportRuntime {
    tempo_bits: AtomicU32,
    is_playing: AtomicBool,
    song_pos_beats_bits: AtomicU64,
    time_sig_num: AtomicU32,
    time_sig_denom: AtomicU32,
}

impl Default for TransportRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl TransportRuntime {
    /// Create a runtime mirror initialized to transport defaults.
    pub fn new() -> Self {
        let defaults = TransportSnapshot::default();
        Self {
            tempo_bits: AtomicU32::new(defaults.tempo_bpm.to_bits()),
            is_playing: AtomicBool::new(defaults.is_playing),
            song_pos_beats_bits: AtomicU64::new(NONE_BEATS_SENTINEL),
            time_sig_num: AtomicU32::new(defaults.time_sig_num as u32),
            time_sig_denom: AtomicU32::new(defaults.time_sig_denom as u32),
        }
    }

    /// Update runtime transport state from one snapshot.
    pub fn update(&self, snapshot: TransportSnapshot) {
        self.tempo_bits.store(
            sanitize_tempo(snapshot.tempo_bpm).to_bits(),
            Ordering::Relaxed,
        );
        self.is_playing
            .store(snapshot.is_playing, Ordering::Relaxed);
        let beats_bits = snapshot
            .song_pos_beats
            .map(f64::to_bits)
            .unwrap_or(NONE_BEATS_SENTINEL);
        self.song_pos_beats_bits
            .store(beats_bits, Ordering::Relaxed);
        self.time_sig_num.store(
            sanitize_time_sig_num(snapshot.time_sig_num) as u32,
            Ordering::Relaxed,
        );
        self.time_sig_denom.store(
            sanitize_time_sig_denom(snapshot.time_sig_denom) as u32,
            Ordering::Relaxed,
        );
    }

    /// Snapshot current transport state.
    pub fn snapshot(&self) -> TransportSnapshot {
        let beats_bits = self.song_pos_beats_bits.load(Ordering::Relaxed);
        TransportSnapshot {
            tempo_bpm: sanitize_tempo(f32::from_bits(self.tempo_bits.load(Ordering::Relaxed))),
            is_playing: self.is_playing.load(Ordering::Relaxed),
            song_pos_beats: if beats_bits == NONE_BEATS_SENTINEL {
                None
            } else {
                Some(f64::from_bits(beats_bits))
            },
            time_sig_num: sanitize_time_sig_num(self.time_sig_num.load(Ordering::Relaxed) as u16),
            time_sig_denom: sanitize_time_sig_denom(
                self.time_sig_denom.load(Ordering::Relaxed) as u16
            ),
        }
    }
}

/// Resolve visible sample count for current scope settings.
pub fn resolve_visible_sample_count(
    ui_state: &XcopeUiState,
    transport: TransportSnapshot,
    sample_rate_hz: f32,
) -> usize {
    let sample_rate = sample_rate_hz.max(1.0);
    let zoom = ui_state.zoom_x.max(0.01);
    let base_samples = match ui_state.mode {
        ScopeMode::TempoLocked if transport.song_pos_beats.is_some() => {
            let beats_visible = beats_visible(ui_state.time_window, transport);
            beats_to_samples(beats_visible, transport.tempo_bpm, sample_rate)
        }
        _ => free_running_window_samples(ui_state.time_window, sample_rate),
    };

    let zoomed = (base_samples as f32 / zoom).round() as usize;
    zoomed.clamp(MIN_SCOPE_WINDOW_SAMPLES, MAX_SCOPE_WINDOW_SAMPLES)
}

/// Resolve beat subdivision line count for one visible beat span.
pub fn subdivisions_for_grid(base: crate::params::GridSubdivision, triplet: bool) -> u32 {
    let div = match base {
        crate::params::GridSubdivision::Div8 => 2,
        crate::params::GridSubdivision::Div16 => 4,
        crate::params::GridSubdivision::Div32 => 8,
    };
    if triplet {
        div * 3 / 2
    } else {
        div
    }
}

fn free_running_window_samples(window: TimeWindow, sample_rate: f32) -> usize {
    let seconds = match window {
        TimeWindow::OneBeat => 0.25,
        TimeWindow::OneBar => 1.0,
        TimeWindow::TwoBars => 2.0,
        TimeWindow::FourBars => 4.0,
    };
    (seconds * sample_rate) as usize
}

fn beats_visible(window: TimeWindow, transport: TransportSnapshot) -> f32 {
    let beats_per_bar = beats_per_bar(transport.time_sig_num, transport.time_sig_denom);
    match window {
        TimeWindow::OneBeat => 1.0,
        TimeWindow::OneBar => beats_per_bar,
        TimeWindow::TwoBars => beats_per_bar * 2.0,
        TimeWindow::FourBars => beats_per_bar * 4.0,
    }
}

fn beats_to_samples(beats: f32, tempo_bpm: f32, sample_rate_hz: f32) -> usize {
    let tempo = sanitize_tempo(tempo_bpm);
    let seconds = (beats.max(0.0) / tempo) * 60.0;
    (seconds * sample_rate_hz.max(1.0)) as usize
}

fn beats_per_bar(num: u16, denom: u16) -> f32 {
    let num = sanitize_time_sig_num(num) as f32;
    let denom = sanitize_time_sig_denom(denom) as f32;
    num * (4.0 / denom)
}

fn sanitize_tempo(tempo_bpm: f32) -> f32 {
    if tempo_bpm.is_finite() && tempo_bpm > 1.0 {
        tempo_bpm
    } else {
        DEFAULT_TEMPO_BPM
    }
}

fn sanitize_time_sig_num(value: u16) -> u16 {
    value.clamp(1, 32)
}

fn sanitize_time_sig_denom(value: u16) -> u16 {
    match value {
        1 | 2 | 4 | 8 | 16 => value,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{DisplayMode, GridSubdivision, ScopeMode};

    fn tempo_locked_state() -> XcopeUiState {
        XcopeUiState {
            mode: ScopeMode::TempoLocked,
            time_window: TimeWindow::OneBar,
            grid_subdivision: GridSubdivision::Div16,
            grid_triplet: false,
            display_mode: DisplayMode::Overlay,
            freeze: false,
            zoom_x: 1.0,
            zoom_y: 1.0,
            channel_visible: [true, true, false, false],
            channel_color: [0, 1, 2, 3],
        }
    }

    #[test]
    fn tempo_locked_window_scales_with_tempo() {
        let state = tempo_locked_state();
        let slow = resolve_visible_sample_count(
            &state,
            TransportSnapshot {
                tempo_bpm: 60.0,
                is_playing: true,
                song_pos_beats: Some(0.0),
                time_sig_num: 4,
                time_sig_denom: 4,
            },
            48_000.0,
        );
        let fast = resolve_visible_sample_count(
            &state,
            TransportSnapshot {
                tempo_bpm: 120.0,
                is_playing: true,
                song_pos_beats: Some(0.0),
                time_sig_num: 4,
                time_sig_denom: 4,
            },
            48_000.0,
        );

        assert!(slow > fast);
    }

    #[test]
    fn tempo_locked_falls_back_without_transport_position() {
        let state = tempo_locked_state();
        let samples = resolve_visible_sample_count(&state, TransportSnapshot::default(), 48_000.0);
        assert_eq!(samples, 48_000);
    }

    #[test]
    fn grid_subdivisions_expand_in_triplet_mode() {
        assert_eq!(
            subdivisions_for_grid(crate::params::GridSubdivision::Div8, false),
            2
        );
        assert_eq!(
            subdivisions_for_grid(crate::params::GridSubdivision::Div8, true),
            3
        );
        assert_eq!(
            subdivisions_for_grid(crate::params::GridSubdivision::Div16, true),
            6
        );
    }

    #[test]
    fn transport_runtime_roundtrip() {
        let runtime = TransportRuntime::new();
        let snapshot = TransportSnapshot {
            tempo_bpm: 132.0,
            is_playing: true,
            song_pos_beats: Some(25.25),
            time_sig_num: 7,
            time_sig_denom: 8,
        };
        runtime.update(snapshot);
        assert_eq!(runtime.snapshot(), snapshot);
    }
}
