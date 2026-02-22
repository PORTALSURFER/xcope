//! Xcope: tempo-aware, multi-channel oscilloscope scaffold built on Toybox.

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub mod constants;
pub mod gui;
#[cfg(test)]
mod host_validation;
pub mod params;
pub mod scope;
pub mod state_io;
pub mod transport;
#[cfg(feature = "vst3")]
mod vst3;

use constants::{CAPTURE_BUFFER_CAPACITY, MAX_SCOPE_WINDOW_SAMPLES};
use params::XcopeParams;
use scope::ScopeCaptureBuffer;
use transport::TransportRuntime;

/// Shared runtime resources used by processor/controller/UI.
#[derive(Debug)]
pub struct XcopeShared {
    /// Shared parameter store.
    pub params: Arc<XcopeParams>,
    /// Shared transport mirror.
    pub transport: Arc<TransportRuntime>,
    /// Shared scope capture ring buffer.
    pub scope_buffer: Arc<ScopeCaptureBuffer>,
    sample_rate_bits: AtomicU32,
}

impl XcopeShared {
    /// Create default shared runtime resources.
    pub fn new() -> Self {
        Self {
            params: Arc::new(XcopeParams::new()),
            transport: Arc::new(TransportRuntime::new()),
            scope_buffer: Arc::new(ScopeCaptureBuffer::new(
                CAPTURE_BUFFER_CAPACITY.min(MAX_SCOPE_WINDOW_SAMPLES),
            )),
            sample_rate_bits: AtomicU32::new(48_000.0f32.to_bits()),
        }
    }

    /// Set active sample rate in hertz.
    pub fn set_sample_rate_hz(&self, sample_rate_hz: f32) {
        let clamped = if sample_rate_hz.is_finite() && sample_rate_hz > 1.0 {
            sample_rate_hz
        } else {
            48_000.0
        };
        self.sample_rate_bits
            .store(clamped.to_bits(), Ordering::Relaxed);
    }

    /// Read active sample rate in hertz.
    pub fn sample_rate_hz(&self) -> f32 {
        f32::from_bits(self.sample_rate_bits.load(Ordering::Relaxed)).max(1.0)
    }
}

impl Default for XcopeShared {
    fn default() -> Self {
        Self::new()
    }
}
