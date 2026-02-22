//! Bus-layout helpers and runtime bus activation state.

use std::sync::atomic::{AtomicU32, Ordering};

use toybox::vst3::prelude::Steinberg::*;

use super::INPUT_SOURCE_BUS_COUNT;

/// Return `true` when the arrangement maps to supported mono/stereo layouts.
pub(super) fn is_supported_bus_arrangement(arrangement: SpeakerArrangement) -> bool {
    BusChannelLayout::from_arrangement(arrangement).is_some()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BusChannelLayout {
    Mono,
    Stereo,
}

impl BusChannelLayout {
    pub(super) const fn to_code(self) -> u32 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
        }
    }

    pub(super) const fn from_code(value: u32) -> Self {
        match value {
            1 => Self::Mono,
            _ => Self::Stereo,
        }
    }

    pub(super) const fn from_arrangement(arrangement: SpeakerArrangement) -> Option<Self> {
        match arrangement {
            SpeakerArr::kMono => Some(Self::Mono),
            SpeakerArr::kStereo => Some(Self::Stereo),
            _ => None,
        }
    }

    pub(super) const fn to_arrangement(self) -> SpeakerArrangement {
        match self {
            Self::Mono => SpeakerArr::kMono,
            Self::Stereo => SpeakerArr::kStereo,
        }
    }

    pub(super) const fn channel_count(self) -> i32 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
        }
    }
}

/// Atomically shared input/output bus arrangement and active-mask configuration.
#[derive(Debug)]
pub(super) struct AtomicBusConfiguration {
    output_layout: AtomicU32,
    input_layouts: [AtomicU32; INPUT_SOURCE_BUS_COUNT],
    input_active_mask: AtomicU32,
}

impl Default for AtomicBusConfiguration {
    fn default() -> Self {
        Self {
            output_layout: AtomicU32::new(BusChannelLayout::Stereo.to_code()),
            input_layouts: std::array::from_fn(|_| {
                AtomicU32::new(BusChannelLayout::Stereo.to_code())
            }),
            input_active_mask: AtomicU32::new(1),
        }
    }
}

impl AtomicBusConfiguration {
    pub(super) fn output_layout(&self) -> BusChannelLayout {
        BusChannelLayout::from_code(self.output_layout.load(Ordering::Relaxed))
    }

    pub(super) fn input_layout(&self, index: usize) -> BusChannelLayout {
        BusChannelLayout::from_code(self.input_layouts[index].load(Ordering::Relaxed))
    }

    pub(super) fn input_active_mask(&self) -> u32 {
        self.input_active_mask.load(Ordering::Relaxed)
    }

    pub(super) fn set_input_active(&self, index: usize, active: bool) {
        let bit = 1u32 << index;
        if active {
            self.input_active_mask.fetch_or(bit, Ordering::Relaxed);
        } else {
            self.input_active_mask.fetch_and(!bit, Ordering::Relaxed);
        }
    }

    pub(super) fn write_arrangements(
        &self,
        output_layout: BusChannelLayout,
        input_layouts: &[BusChannelLayout],
    ) {
        self.output_layout
            .store(output_layout.to_code(), Ordering::Relaxed);
        for index in 0..INPUT_SOURCE_BUS_COUNT {
            let layout = input_layouts
                .get(index)
                .copied()
                .unwrap_or(BusChannelLayout::Stereo);
            self.input_layouts[index].store(layout.to_code(), Ordering::Relaxed);
        }

        let active_count = input_layouts.len().min(INPUT_SOURCE_BUS_COUNT);
        let mask = if active_count == 0 {
            0
        } else {
            (1u32 << active_count) - 1
        };
        self.input_active_mask.store(mask, Ordering::Relaxed);
    }
}
