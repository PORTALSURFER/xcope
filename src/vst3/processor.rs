//! VST3 processor implementation and audio callback handling.

use std::slice;
use std::sync::Arc;

use toybox::vst3::prelude::Steinberg::*;
use toybox::vst3::prelude::*;

use crate::constants::{MAX_VISUAL_CHANNELS, STATE_MAGIC, STATE_VERSION};
use crate::params::apply_param_normalized;
use crate::state_io::{decode_state_payload, encode_state_payload};
use crate::XcopeShared;

use super::audio_io::{
    copy_main_bus_to_output, process_first_output_bus, process_input_buses, read_bus_left_sample,
};
use super::bus::{is_supported_bus_arrangement, AtomicBusConfiguration, BusChannelLayout};
use super::shared_registry::{release_shared_for_role, SharedRole};
use super::transport_context::{resolve_end_of_block_anchor_beats, transport_from_context};
use super::{vst3_bus_flag, vst3_process_requirement_flag, CONTROLLER_CID, INPUT_SOURCE_BUS_COUNT};

pub(super) struct XcopeVst3Processor {
    shared: Arc<XcopeShared>,
    bus_configuration: AtomicBusConfiguration,
}

impl XcopeVst3Processor {
    pub(super) fn new(shared: Arc<XcopeShared>) -> Self {
        Self {
            shared,
            bus_configuration: AtomicBusConfiguration::default(),
        }
    }
}

impl Drop for XcopeVst3Processor {
    fn drop(&mut self) {
        release_shared_for_role(&self.shared, SharedRole::Processor);
    }
}

impl Class for XcopeVst3Processor {
    type Interfaces = (IComponent, IAudioProcessor, IProcessContextRequirements);
}

impl IPluginBaseTrait for XcopeVst3Processor {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IComponentTrait for XcopeVst3Processor {
    unsafe fn getControllerClassId(&self, class_id: *mut TUID) -> tresult {
        if class_id.is_null() {
            return kInvalidArgument;
        }
        unsafe { *class_id = CONTROLLER_CID };
        kResultOk
    }

    unsafe fn setIoMode(&self, _mode: IoMode) -> tresult {
        kResultOk
    }

    unsafe fn getBusCount(&self, media_type: MediaType, dir: BusDirection) -> i32 {
        match media_type as MediaTypes {
            MediaTypes_::kAudio => match dir as BusDirections {
                BusDirections_::kInput => INPUT_SOURCE_BUS_COUNT as i32,
                BusDirections_::kOutput => 1,
                _ => 0,
            },
            _ => 0,
        }
    }

    unsafe fn getBusInfo(
        &self,
        media_type: MediaType,
        dir: BusDirection,
        index: i32,
        bus: *mut BusInfo,
    ) -> tresult {
        if bus.is_null() || index < 0 {
            return kInvalidArgument;
        }
        if media_type as MediaTypes != MediaTypes_::kAudio {
            return kInvalidArgument;
        }

        let index = index as usize;
        let bus = unsafe { &mut *bus };
        bus.mediaType = MediaTypes_::kAudio as MediaType;
        bus.direction = dir;

        match dir as BusDirections {
            BusDirections_::kInput if index < INPUT_SOURCE_BUS_COUNT => {
                let label = match index {
                    0 => "Main Input",
                    _ => "Sidechain Input",
                };
                bus.channelCount = self.bus_configuration.input_layout(index).channel_count();
                copy_wstring(label, &mut bus.name);
                bus.busType = if index == 0 {
                    BusTypes_::kMain as BusType
                } else {
                    BusTypes_::kAux as BusType
                };
                bus.flags = if index == 0 {
                    vst3_bus_flag(BusInfo_::BusFlags_::kDefaultActive)
                } else {
                    0
                };
                kResultOk
            }
            BusDirections_::kOutput if index == 0 => {
                bus.channelCount = self.bus_configuration.output_layout().channel_count();
                copy_wstring("Output", &mut bus.name);
                bus.busType = BusTypes_::kMain as BusType;
                bus.flags = vst3_bus_flag(BusInfo_::BusFlags_::kDefaultActive);
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }

    unsafe fn getRoutingInfo(
        &self,
        _in_info: *mut RoutingInfo,
        _out_info: *mut RoutingInfo,
    ) -> tresult {
        kNotImplemented
    }

    unsafe fn activateBus(
        &self,
        media_type: MediaType,
        dir: BusDirection,
        index: i32,
        state: TBool,
    ) -> tresult {
        if media_type as MediaTypes != MediaTypes_::kAudio || index < 0 {
            return kInvalidArgument;
        }
        if matches!(dir as BusDirections, BusDirections_::kInput) {
            let index = index as usize;
            if index >= INPUT_SOURCE_BUS_COUNT {
                return kInvalidArgument;
            }
            self.bus_configuration.set_input_active(index, state != 0);
            return kResultOk;
        }
        if matches!(dir as BusDirections, BusDirections_::kOutput) && index == 0 {
            return kResultOk;
        }
        if matches!(dir as BusDirections, BusDirections_::kOutput) {
            return kInvalidArgument;
        }
        kResultOk
    }

    unsafe fn setActive(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        let payload = match unsafe { read_versioned_payload(state, STATE_MAGIC, &[STATE_VERSION]) }
        {
            Ok(payload) => payload,
            Err(_) => return kResultFalse,
        };
        match decode_state_payload(self.shared.params.as_ref(), &payload.payload) {
            Ok(()) => kResultOk,
            Err(_) => kResultFalse,
        }
    }

    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        let payload = encode_state_payload(self.shared.params.as_ref());
        match unsafe { write_versioned_payload(state, STATE_MAGIC, STATE_VERSION, &payload) } {
            Ok(()) => kResultOk,
            Err(_) => kResultFalse,
        }
    }
}

impl IAudioProcessorTrait for XcopeVst3Processor {
    unsafe fn setBusArrangements(
        &self,
        inputs: *mut SpeakerArrangement,
        num_ins: i32,
        outputs: *mut SpeakerArrangement,
        num_outs: i32,
    ) -> tresult {
        if num_ins <= 0 || num_ins > INPUT_SOURCE_BUS_COUNT as i32 || num_outs != 1 {
            return kResultFalse;
        }
        if inputs.is_null() || outputs.is_null() {
            return kInvalidArgument;
        }

        let output_arrangement = unsafe { *outputs };
        if !is_supported_bus_arrangement(output_arrangement) {
            return kResultFalse;
        }
        let Some(output_layout) = BusChannelLayout::from_arrangement(output_arrangement) else {
            return kResultFalse;
        };

        let input_arrangements = unsafe { slice::from_raw_parts(inputs, num_ins as usize) };
        if !is_supported_bus_arrangement(input_arrangements[0])
            || input_arrangements[0] != output_arrangement
        {
            return kResultFalse;
        }
        for arrangement in input_arrangements.iter().copied() {
            if !is_supported_bus_arrangement(arrangement) {
                return kResultFalse;
            }
        }

        let mut input_layouts = [BusChannelLayout::Stereo; INPUT_SOURCE_BUS_COUNT];
        for (index, arrangement) in input_arrangements.iter().copied().enumerate() {
            let Some(layout) = BusChannelLayout::from_arrangement(arrangement) else {
                return kResultFalse;
            };
            input_layouts[index] = layout;
        }
        self.bus_configuration
            .write_arrangements(output_layout, &input_layouts[..input_arrangements.len()]);
        kResultTrue
    }

    unsafe fn getBusArrangement(
        &self,
        dir: BusDirection,
        index: i32,
        arr: *mut SpeakerArrangement,
    ) -> tresult {
        if arr.is_null() || index < 0 {
            return kInvalidArgument;
        }
        let index = index as usize;
        match dir as BusDirections {
            BusDirections_::kInput if index < INPUT_SOURCE_BUS_COUNT => {
                unsafe { *arr = self.bus_configuration.input_layout(index).to_arrangement() };
                kResultOk
            }
            BusDirections_::kOutput if index == 0 => {
                unsafe { *arr = self.bus_configuration.output_layout().to_arrangement() };
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }

    unsafe fn canProcessSampleSize(&self, symbolic_sample_size: i32) -> tresult {
        match symbolic_sample_size as SymbolicSampleSizes {
            SymbolicSampleSizes_::kSample32 => kResultOk,
            SymbolicSampleSizes_::kSample64 => kNotImplemented,
            _ => kInvalidArgument,
        }
    }

    unsafe fn getLatencySamples(&self) -> u32 {
        0
    }

    unsafe fn setupProcessing(&self, setup: *mut ProcessSetup) -> tresult {
        if let Some(setup) = unsafe { setup.as_ref() } {
            self.shared.set_sample_rate_hz(setup.sampleRate as f32);
        }
        kResultOk
    }

    unsafe fn setProcessing(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let Some(data) = (unsafe { data.as_ref() }) else {
            return kInvalidArgument;
        };

        unsafe {
            for_each_param_point(
                data.inputParameterChanges,
                |param_id, _sample_offset, value| {
                    let _ = apply_param_normalized(self.shared.params.as_ref(), param_id, value);
                },
            );
        }

        let previous_transport = self.shared.transport.snapshot();
        self.shared.transport.update(transport_from_context(
            data.processContext,
            data.numSamples,
            self.shared.sample_rate_hz(),
            previous_transport,
        ));
        let transport_snapshot = self.shared.transport.snapshot();

        if data.numSamples <= 0 || data.symbolicSampleSize != SymbolicSampleSizes_::kSample32 as i32
        {
            let anchor_sample = self.shared.scope_buffer.total_written_samples();
            let previous_anchor = self.shared.scope_buffer.transport_anchor();
            let anchor_beats = resolve_end_of_block_anchor_beats(
                data.processContext,
                data.numSamples,
                self.shared.sample_rate_hz(),
                transport_snapshot.tempo_bpm,
                previous_anchor,
                anchor_sample,
            );
            self.shared.scope_buffer.set_transport_anchor(anchor_beats);
            return process_ok();
        }

        let sample_count = data.numSamples as usize;
        let active_mask = self.bus_configuration.input_active_mask();
        let input_buses = unsafe { process_input_buses(data) };
        let output_bus = unsafe { process_first_output_bus(data) };

        if let Some(output_bus) = output_bus {
            let main_input_bus = input_buses.and_then(|buses| buses.first());
            unsafe { copy_main_bus_to_output(main_input_bus, output_bus, sample_count) };
        }

        for sample_index in 0..sample_count {
            let mut capture_sample = [0.0f32; MAX_VISUAL_CHANNELS];
            let mut active_sources = 0usize;
            for (source_index, slot) in capture_sample
                .iter_mut()
                .enumerate()
                .take(MAX_VISUAL_CHANNELS)
            {
                if (active_mask & (1u32 << source_index)) == 0 {
                    continue;
                }
                let Some(buses) = input_buses else {
                    continue;
                };
                let Some(input_bus) = buses.get(source_index) else {
                    continue;
                };
                if let Some(value) = unsafe { read_bus_left_sample(input_bus, sample_index) } {
                    *slot = value;
                    active_sources = active_sources.max(source_index + 1);
                }
            }
            self.shared
                .scope_buffer
                .write_sample(capture_sample, active_sources.max(1));
        }
        let anchor_sample = self.shared.scope_buffer.total_written_samples();
        let previous_anchor = self.shared.scope_buffer.transport_anchor();
        let anchor_beats = resolve_end_of_block_anchor_beats(
            data.processContext,
            data.numSamples,
            self.shared.sample_rate_hz(),
            transport_snapshot.tempo_bpm,
            previous_anchor,
            anchor_sample,
        );
        self.shared.scope_buffer.set_transport_anchor(anchor_beats);

        process_ok()
    }

    unsafe fn getTailSamples(&self) -> u32 {
        0
    }
}

impl IProcessContextRequirementsTrait for XcopeVst3Processor {
    unsafe fn getProcessContextRequirements(&self) -> u32 {
        vst3_process_requirement_flag(IProcessContextRequirements_::Flags_::kNeedTempo)
            | vst3_process_requirement_flag(
                IProcessContextRequirements_::Flags_::kNeedProjectTimeMusic,
            )
            | vst3_process_requirement_flag(
                IProcessContextRequirements_::Flags_::kNeedTransportState,
            )
    }
}
