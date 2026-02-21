//! Waveform render-sampling helpers.
//!
//! This module provides deterministic min/max decimation primitives used to
//! project many input samples onto a fixed number of horizontal pixel columns.

use crate::scope::ScopeFrame;

/// One per-column amplitude envelope.
///
/// `min` and `max` capture the full vertical extent for all source samples that
/// map to one render column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnExtrema {
    /// Minimum sample value observed in the column source range.
    pub min: f32,
    /// Maximum sample value observed in the column source range.
    pub max: f32,
}

/// Compute min/max envelopes for one dense sample slice.
///
/// The returned vector has exactly `columns` entries when both `samples` and
/// `columns` are non-empty. When either is empty, an empty vector is returned.
///
/// Mapping is stable and deterministic:
/// - each column maps to a contiguous sample range
/// - every sample contributes to at least one column
/// - dense spikes are preserved through min/max capture
pub fn decimate_min_max(samples: &[f32], columns: usize) -> Vec<ColumnExtrema> {
    if samples.is_empty() || columns == 0 {
        return Vec::new();
    }

    let sample_count = samples.len();
    let mut result = Vec::with_capacity(columns);
    for column in 0..columns {
        let start = column * sample_count / columns;
        let mut end = (column + 1) * sample_count / columns;
        if end <= start {
            end = (start + 1).min(sample_count);
        }
        let mut min_value = f32::INFINITY;
        let mut max_value = f32::NEG_INFINITY;
        for sample in &samples[start..end] {
            min_value = min_value.min(*sample);
            max_value = max_value.max(*sample);
        }
        if !min_value.is_finite() || !max_value.is_finite() {
            result.push(ColumnExtrema { min: 0.0, max: 0.0 });
        } else {
            result.push(ColumnExtrema {
                min: min_value,
                max: max_value,
            });
        }
    }
    result
}

/// Compute min/max envelopes for one channel within a captured scope frame.
///
/// Channel indices outside the captured channel range produce an empty result.
pub fn decimate_frame_channel(
    frame: &ScopeFrame,
    channel_index: usize,
    columns: usize,
) -> Vec<ColumnExtrema> {
    if columns == 0 || frame.sample_count() == 0 || channel_index >= frame.channel_count {
        return Vec::new();
    }
    let samples: Vec<f32> = frame
        .samples
        .iter()
        .map(|sample| sample[channel_index])
        .collect();
    decimate_min_max(&samples, columns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::MAX_VISUAL_CHANNELS;

    #[test]
    fn decimation_returns_empty_for_empty_input_or_zero_columns() {
        assert!(decimate_min_max(&[], 16).is_empty());
        assert!(decimate_min_max(&[0.1, 0.2], 0).is_empty());
    }

    #[test]
    fn decimation_is_deterministic_for_same_input() {
        let samples = [-0.4, 0.1, 0.8, -0.2, 0.3, 0.6, -0.7, 0.2];
        let a = decimate_min_max(&samples, 4);
        let b = decimate_min_max(&samples, 4);
        assert_eq!(a, b);
    }

    #[test]
    fn decimation_preserves_dense_spike_in_column_extrema() {
        let mut samples = vec![0.0f32; 256];
        samples[129] = 1.0;
        let envelopes = decimate_min_max(&samples, 32);
        assert_eq!(envelopes.len(), 32);
        assert!(envelopes.iter().any(|column| column.max >= 1.0));
    }

    #[test]
    fn frame_channel_decimation_uses_requested_channel() {
        let frame = ScopeFrame {
            channel_count: 2,
            samples: vec![
                [0.1, -0.9, 0.0, 0.0],
                [0.2, 0.5, 0.0, 0.0],
                [0.3, 0.8, 0.0, 0.0],
                [0.4, -0.1, 0.0, 0.0],
            ],
        };
        let left = decimate_frame_channel(&frame, 0, 2);
        let right = decimate_frame_channel(&frame, 1, 2);
        assert_eq!(left.len(), 2);
        assert_eq!(right.len(), 2);
        assert_ne!(left, right);
    }

    #[test]
    fn frame_channel_decimation_rejects_out_of_range_channel() {
        let frame = ScopeFrame {
            channel_count: 1,
            samples: vec![[0.25; MAX_VISUAL_CHANNELS]],
        };
        assert!(decimate_frame_channel(&frame, 1, 8).is_empty());
    }
}
