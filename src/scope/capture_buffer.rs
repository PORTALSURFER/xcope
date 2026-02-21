//! Lock-free scope capture ring buffer.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::constants::MAX_VISUAL_CHANNELS;

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
        let available = self.total_written.load(Ordering::Acquire) as usize;
        let frame_count = requested_frames.min(available).min(self.capacity);
        let channel_count = self
            .last_channel_count
            .load(Ordering::Relaxed)
            .clamp(0, MAX_VISUAL_CHANNELS);
        if frame_count == 0 || channel_count == 0 {
            return ScopeFrame {
                channel_count,
                samples: Vec::new(),
            };
        }

        let start_absolute = available.saturating_sub(frame_count);
        let mut samples = Vec::with_capacity(frame_count);
        for absolute_index in start_absolute..available {
            let ring_index = absolute_index % self.capacity;
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
}
