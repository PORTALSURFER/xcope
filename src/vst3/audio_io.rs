//! Low-level audio-buffer access helpers used by the VST3 process callback.

use std::ptr;
use std::slice;

use toybox::vst3::prelude::*;

/// Borrow all input buses from process data when present.
pub(super) unsafe fn process_input_buses(data: &ProcessData) -> Option<&[AudioBusBuffers]> {
    if data.numInputs <= 0 || data.inputs.is_null() {
        return None;
    }
    let bus_count = usize::try_from(data.numInputs).ok()?;
    Some(unsafe { slice::from_raw_parts(data.inputs, bus_count) })
}

/// Borrow the first output bus from process data when present.
pub(super) unsafe fn process_first_output_bus(data: &ProcessData) -> Option<&AudioBusBuffers> {
    if data.numOutputs <= 0 || data.outputs.is_null() {
        return None;
    }
    let bus_count = usize::try_from(data.numOutputs).ok()?;
    let output_buses = unsafe { slice::from_raw_parts(data.outputs, bus_count) };
    output_buses.first()
}

unsafe fn bus_channel_buffers_32(bus: &AudioBusBuffers) -> Option<&[*mut f32]> {
    if bus.numChannels <= 0 {
        return None;
    }
    let channel_count = usize::try_from(bus.numChannels).ok()?;
    let channels_ptr = unsafe { bus.__field0.channelBuffers32 };
    if channels_ptr.is_null() {
        return None;
    }
    Some(unsafe { slice::from_raw_parts(channels_ptr, channel_count) })
}

/// Read one sample from the left channel of one input bus.
pub(super) unsafe fn read_bus_left_sample(
    bus: &AudioBusBuffers,
    sample_index: usize,
) -> Option<f32> {
    let channels = unsafe { bus_channel_buffers_32(bus) }?;
    let left = channels.first().copied()?;
    if left.is_null() {
        return None;
    }
    Some(unsafe { *left.add(sample_index) })
}

/// Copy the main input bus to the first output bus as passthrough audio.
pub(super) unsafe fn copy_main_bus_to_output(
    input_bus: Option<&AudioBusBuffers>,
    output_bus: &AudioBusBuffers,
    sample_count: usize,
) {
    let Some(output_channels) = (unsafe { bus_channel_buffers_32(output_bus) }) else {
        return;
    };
    let input_channels = input_bus.and_then(|bus| unsafe { bus_channel_buffers_32(bus) });
    let copy_channels = input_channels
        .map(|channels| channels.len().min(output_channels.len()))
        .unwrap_or(0);

    for (channel_index, output_channel) in output_channels
        .iter()
        .copied()
        .enumerate()
        .take(copy_channels)
    {
        let input_channel = input_channels
            .and_then(|channels| channels.get(channel_index).copied())
            .unwrap_or(ptr::null_mut());
        if output_channel.is_null() {
            continue;
        }
        if input_channel.is_null() {
            for sample_index in 0..sample_count {
                unsafe {
                    *output_channel.add(sample_index) = 0.0;
                }
            }
            continue;
        }
        for sample_index in 0..sample_count {
            unsafe {
                *output_channel.add(sample_index) = *input_channel.add(sample_index);
            }
        }
    }

    for output_channel in output_channels.iter().skip(copy_channels).copied() {
        if output_channel.is_null() {
            continue;
        }
        for sample_index in 0..sample_count {
            unsafe {
                *output_channel.add(sample_index) = 0.0;
            }
        }
    }
}
