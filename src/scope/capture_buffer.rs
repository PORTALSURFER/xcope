//! Lock-free scope capture ring buffer.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::constants::MAX_VISUAL_CHANNELS;

const NONE_BEATS_SENTINEL: u64 = u64::MAX;

/// One immutable snapshot of captured scope samples.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScopeFrame {
    /// Number of valid channels represented in each sample frame.
    pub channel_count: usize,
    /// Captured samples in chronological order.
    pub samples: Vec<[f32; MAX_VISUAL_CHANNELS]>,
}

impl ScopeFrame {
    /// Number of captured sample frames.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Read one sample value by channel and frame index.
    pub fn sample(&self, channel: usize, index: usize) -> f32 {
        self.samples
            .get(index)
            .map(|sample| sample[channel.min(MAX_VISUAL_CHANNELS - 1)])
            .unwrap_or(0.0)
    }
}

/// Real-time capture ring buffer storing up to `MAX_VISUAL_CHANNELS` channels.
#[derive(Debug)]
pub struct ScopeCaptureBuffer {
    capacity: usize,
    storage: Vec<AtomicU32>,
    write_cursor: AtomicUsize,
    total_written: AtomicU64,
    last_channel_count: AtomicUsize,
    transport_anchor_sample: AtomicU64,
    transport_anchor_beats_bits: AtomicU64,
    transport_anchor_epoch: AtomicU64,
}

impl ScopeCaptureBuffer {
    /// Create a new ring buffer with the given frame capacity.
    pub fn new(capacity: usize) -> Self {
        let clamped_capacity = capacity.max(1);
        let storage_len = clamped_capacity.saturating_mul(MAX_VISUAL_CHANNELS);
        Self {
            capacity: clamped_capacity,
            storage: (0..storage_len).map(|_| AtomicU32::new(0)).collect(),
            write_cursor: AtomicUsize::new(0),
            total_written: AtomicU64::new(0),
            last_channel_count: AtomicUsize::new(0),
            transport_anchor_sample: AtomicU64::new(0),
            transport_anchor_beats_bits: AtomicU64::new(NONE_BEATS_SENTINEL),
            transport_anchor_epoch: AtomicU64::new(0),
        }
    }

    /// Append one audio block to the capture ring.
    ///
    /// `channels` may include any channel count. Only the first
    /// `MAX_VISUAL_CHANNELS` channels are captured for visualization.
    pub fn write_block(&self, channels: &[&[f32]], sample_count: usize) {
        if sample_count == 0 {
            return;
        }

        let channel_count = channels.len().min(MAX_VISUAL_CHANNELS);
        if channel_count == 0 {
            return;
        }
        self.last_channel_count
            .store(channel_count, Ordering::Relaxed);

        let mut cursor = self.write_cursor.load(Ordering::Relaxed);
        for sample_index in 0..sample_count {
            for (channel_index, channel) in channels.iter().take(channel_count).enumerate() {
                let value = channel.get(sample_index).copied().unwrap_or(0.0);
                self.store_sample(cursor, channel_index, value);
            }
            for channel_index in channel_count..MAX_VISUAL_CHANNELS {
                self.store_sample(cursor, channel_index, 0.0);
            }
            cursor += 1;
            if cursor >= self.capacity {
                cursor = 0;
            }
        }

        self.write_cursor.store(cursor, Ordering::Release);
        self.total_written
            .fetch_add(sample_count as u64, Ordering::Release);
    }

    /// Append one sample frame to the capture ring.
    ///
    /// `channel_count` is clamped to `MAX_VISUAL_CHANNELS`.
    pub fn write_sample(&self, sample: [f32; MAX_VISUAL_CHANNELS], channel_count: usize) {
        let channels = channel_count.clamp(0, MAX_VISUAL_CHANNELS);
        if channels == 0 {
            return;
        }
        self.last_channel_count.store(channels, Ordering::Relaxed);
        let cursor = self.write_cursor.load(Ordering::Relaxed);
        for (channel_index, value) in sample.iter().enumerate() {
            self.store_sample(cursor, channel_index, *value);
        }
        let next = if cursor + 1 >= self.capacity {
            0
        } else {
            cursor + 1
        };
        self.write_cursor.store(next, Ordering::Release);
        self.total_written.fetch_add(1, Ordering::Release);
    }

    /// Return a chronological snapshot of the most recent sample frames.
    pub fn snapshot_recent(&self, requested_frames: usize) -> ScopeFrame {
        let end_exclusive = self.total_written.load(Ordering::Acquire);
        self.snapshot_ending_at(end_exclusive, requested_frames)
    }

    /// Return one chronological snapshot ending at `end_exclusive`.
    ///
    /// `end_exclusive` uses absolute sample indexing in the capture timeline.
    pub(crate) fn snapshot_ending_at(
        &self,
        end_exclusive: u64,
        requested_frames: usize,
    ) -> ScopeFrame {
        let available = self.total_written.load(Ordering::Acquire);
        let earliest_available = available.saturating_sub(self.capacity as u64);
        let end = end_exclusive.clamp(earliest_available, available);
        let start = end
            .saturating_sub(requested_frames.min(self.capacity) as u64)
            .max(earliest_available);
        let channel_count = self
            .last_channel_count
            .load(Ordering::Relaxed)
            .clamp(0, MAX_VISUAL_CHANNELS);
        if start >= end || channel_count == 0 {
            return ScopeFrame {
                channel_count,
                samples: Vec::new(),
            };
        }

        let frame_count = (end - start) as usize;
        let mut samples = Vec::with_capacity(frame_count);
        for absolute_index in start..end {
            let ring_index = (absolute_index as usize) % self.capacity;
            let mut frame = [0.0f32; MAX_VISUAL_CHANNELS];
            for (channel_index, value) in frame.iter_mut().enumerate() {
                *value = self.load_sample(ring_index, channel_index);
            }
            samples.push(frame);
        }

        ScopeFrame {
            channel_count,
            samples,
        }
    }

    /// Return total number of captured samples ever written.
    pub(crate) fn total_written_samples(&self) -> u64 {
        self.total_written.load(Ordering::Acquire)
    }

    /// Update the transport anchor used for deterministic scope window lookup.
    ///
    /// The anchor pairs the current absolute sample index with host song-position
    /// beats captured from the process context.
    #[cfg(any(feature = "vst3", test))]
    pub(crate) fn set_transport_anchor(&self, song_pos_beats: Option<f64>) {
        self.transport_anchor_epoch.fetch_add(1, Ordering::AcqRel);
        let sample = self.total_written.load(Ordering::Acquire);
        self.transport_anchor_sample
            .store(sample, Ordering::Release);
        let beats_bits = song_pos_beats
            .filter(|value| value.is_finite())
            .map(f64::to_bits)
            .unwrap_or(NONE_BEATS_SENTINEL);
        self.transport_anchor_beats_bits
            .store(beats_bits, Ordering::Release);
        self.transport_anchor_epoch.fetch_add(1, Ordering::Release);
    }

    /// Return the latest transport anchor as `(sample_index, song_pos_beats)`.
    pub(crate) fn transport_anchor(&self) -> Option<(u64, f64)> {
        for _ in 0..8 {
            let start_epoch = self.transport_anchor_epoch.load(Ordering::Acquire);
            if start_epoch & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }

            let sample = self.transport_anchor_sample.load(Ordering::Acquire);
            let beats_bits = self.transport_anchor_beats_bits.load(Ordering::Acquire);
            let end_epoch = self.transport_anchor_epoch.load(Ordering::Acquire);

            if start_epoch != end_epoch || end_epoch & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            if beats_bits == NONE_BEATS_SENTINEL {
                return None;
            }
            return Some((sample, f64::from_bits(beats_bits)));
        }
        None
    }

    fn storage_index(&self, frame_index: usize, channel_index: usize) -> usize {
        frame_index
            .saturating_mul(MAX_VISUAL_CHANNELS)
            .saturating_add(channel_index)
    }

    fn store_sample(&self, frame_index: usize, channel_index: usize, value: f32) {
        let index = self.storage_index(frame_index, channel_index);
        self.storage[index].store(value.to_bits(), Ordering::Relaxed);
    }

    fn load_sample(&self, frame_index: usize, channel_index: usize) -> f32 {
        let index = self.storage_index(frame_index, channel_index);
        f32::from_bits(self.storage[index].load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_returns_recent_frames_in_chronological_order() {
        let buffer = ScopeCaptureBuffer::new(8);
        let ch0 = [0.0, 1.0, 2.0, 3.0, 4.0];
        let ch1 = [10.0, 11.0, 12.0, 13.0, 14.0];
        buffer.write_block(&[&ch0, &ch1], 5);

        let snapshot = buffer.snapshot_recent(3);
        assert_eq!(snapshot.channel_count, 2);
        assert_eq!(snapshot.sample_count(), 3);
        assert_eq!(snapshot.sample(0, 0), 2.0);
        assert_eq!(snapshot.sample(1, 0), 12.0);
        assert_eq!(snapshot.sample(0, 2), 4.0);
        assert_eq!(snapshot.sample(1, 2), 14.0);
    }

    #[test]
    fn snapshot_caps_to_ring_capacity_after_wrap() {
        let buffer = ScopeCaptureBuffer::new(4);
        let ch0 = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        buffer.write_block(&[&ch0], 6);

        let snapshot = buffer.snapshot_recent(10);
        assert_eq!(snapshot.sample_count(), 4);
        assert_eq!(snapshot.sample(0, 0), 2.0);
        assert_eq!(snapshot.sample(0, 3), 5.0);
    }

    #[test]
    fn snapshot_ending_at_reads_absolute_window() {
        let buffer = ScopeCaptureBuffer::new(8);
        let ch0 = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        buffer.write_block(&[&ch0], 6);

        let snapshot = buffer.snapshot_ending_at(5, 3);
        assert_eq!(snapshot.sample_count(), 3);
        assert_eq!(snapshot.sample(0, 0), 2.0);
        assert_eq!(snapshot.sample(0, 1), 3.0);
        assert_eq!(snapshot.sample(0, 2), 4.0);
    }

    #[test]
    fn transport_anchor_roundtrip_reports_latest_sample_and_beats() {
        let buffer = ScopeCaptureBuffer::new(8);
        let ch0 = [0.0, 1.0, 2.0, 3.0];
        buffer.write_block(&[&ch0], 4);
        buffer.set_transport_anchor(Some(12.5));

        let anchor = buffer.transport_anchor().expect("anchor should exist");
        assert_eq!(anchor.0, 4);
        assert!((anchor.1 - 12.5).abs() < 1.0e-9);
    }

    #[test]
    fn transport_anchor_none_clears_anchor_state() {
        let buffer = ScopeCaptureBuffer::new(8);
        let ch0 = [0.0, 1.0, 2.0, 3.0];
        buffer.write_block(&[&ch0], 4);
        buffer.set_transport_anchor(None);

        assert!(buffer.transport_anchor().is_none());
    }
}
