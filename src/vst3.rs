//! VST3 processor/controller scaffold for Xcope.

#![allow(clippy::missing_docs_in_private_items)]

use std::ffi::{c_void, CStr};
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use toybox::vst3::prelude::Steinberg::*;
use toybox::vst3::prelude::*;

use crate::constants::{ParamId, MAX_VISUAL_CHANNELS, PLUGIN_NAME, STATE_MAGIC, STATE_VERSION};
use crate::gui::{preferred_window_size, XcopeGui};
use crate::params::{apply_param_normalized, param_count, read_param_normalized};
use crate::state_io::{decode_state_payload, encode_state_payload};
use crate::transport::{project_song_position_beats, TransportSnapshot};
use crate::XcopeShared;

const PROCESSOR_CID: TUID = uid(0x9AF47871, 0x00A645F3, 0x9D8A34AA, 0x7D4E7821);
const CONTROLLER_CID: TUID = uid(0x0B49357D, 0xF45A4D2D, 0xA67A66AE, 0xD7C24B7A);
const INPUT_SOURCE_BUS_COUNT: usize = MAX_VISUAL_CHANNELS;

#[cfg(target_os = "windows")]
const fn vst3_bus_flag(flag: i32) -> u32 {
    flag as u32
}

#[cfg(not(target_os = "windows"))]
const fn vst3_bus_flag(flag: u32) -> u32 {
    flag
}

#[cfg(target_os = "windows")]
const fn vst3_process_state_flag(flag: i32) -> u32 {
    flag as u32
}

#[cfg(not(target_os = "windows"))]
const fn vst3_process_state_flag(flag: u32) -> u32 {
    flag
}

#[cfg(target_os = "windows")]
const fn vst3_process_requirement_flag(flag: i32) -> u32 {
    flag as u32
}

#[cfg(not(target_os = "windows"))]
const fn vst3_process_requirement_flag(flag: u32) -> u32 {
    flag
}

fn is_supported_bus_arrangement(arrangement: SpeakerArrangement) -> bool {
    BusChannelLayout::from_arrangement(arrangement).is_some()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BusChannelLayout {
    Mono,
    Stereo,
}

impl BusChannelLayout {
    const fn to_code(self) -> u32 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
        }
    }

    const fn from_code(value: u32) -> Self {
        match value {
            1 => Self::Mono,
            _ => Self::Stereo,
        }
    }

    const fn from_arrangement(arrangement: SpeakerArrangement) -> Option<Self> {
        match arrangement {
            SpeakerArr::kMono => Some(Self::Mono),
            SpeakerArr::kStereo => Some(Self::Stereo),
            _ => None,
        }
    }

    const fn to_arrangement(self) -> SpeakerArrangement {
        match self {
            Self::Mono => SpeakerArr::kMono,
            Self::Stereo => SpeakerArr::kStereo,
        }
    }

    const fn channel_count(self) -> i32 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
        }
    }
}

#[derive(Debug)]
struct AtomicBusConfiguration {
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
    fn output_layout(&self) -> BusChannelLayout {
        BusChannelLayout::from_code(self.output_layout.load(Ordering::Relaxed))
    }

    fn input_layout(&self, index: usize) -> BusChannelLayout {
        BusChannelLayout::from_code(self.input_layouts[index].load(Ordering::Relaxed))
    }

    fn input_active_mask(&self) -> u32 {
        self.input_active_mask.load(Ordering::Relaxed)
    }

    fn set_input_active(&self, index: usize, active: bool) {
        let bit = 1u32 << index;
        if active {
            self.input_active_mask.fetch_or(bit, Ordering::Relaxed);
        } else {
            self.input_active_mask.fetch_and(!bit, Ordering::Relaxed);
        }
    }

    fn write_arrangements(
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

#[derive(Copy, Clone)]
enum SharedRole {
    Processor,
    Controller,
}

struct SharedRegistryEntry {
    shared: Weak<XcopeShared>,
    processor_claimed: bool,
    controller_claimed: bool,
}

fn shared_registry() -> &'static Mutex<Vec<SharedRegistryEntry>> {
    static REGISTRY: OnceLock<Mutex<Vec<SharedRegistryEntry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

fn acquire_shared_for_role(role: SharedRole) -> Arc<XcopeShared> {
    let mut registry = match shared_registry().lock() {
        Ok(registry) => registry,
        Err(_) => return Arc::new(XcopeShared::new()),
    };
    registry.retain(|entry| entry.shared.upgrade().is_some());

    for entry in registry.iter_mut() {
        let Some(shared) = entry.shared.upgrade() else {
            continue;
        };
        match role {
            SharedRole::Processor if !entry.processor_claimed => {
                entry.processor_claimed = true;
                return shared;
            }
            SharedRole::Controller if !entry.controller_claimed => {
                entry.controller_claimed = true;
                return shared;
            }
            _ => {}
        }
    }

    let shared = Arc::new(XcopeShared::new());
    registry.push(SharedRegistryEntry {
        shared: Arc::downgrade(&shared),
        processor_claimed: matches!(role, SharedRole::Processor),
        controller_claimed: matches!(role, SharedRole::Controller),
    });
    shared
}

fn release_shared_for_role(shared: &Arc<XcopeShared>, role: SharedRole) {
    let mut registry = match shared_registry().lock() {
        Ok(registry) => registry,
        Err(_) => return,
    };

    registry.retain(|entry| entry.shared.upgrade().is_some());
    for entry in registry.iter_mut() {
        let Some(candidate) = entry.shared.upgrade() else {
            continue;
        };
        if !Arc::ptr_eq(&candidate, shared) {
            continue;
        }
        match role {
            SharedRole::Processor => entry.processor_claimed = false,
            SharedRole::Controller => entry.controller_claimed = false,
        }
    }
}

struct XcopeVst3Processor {
    shared: Arc<XcopeShared>,
    bus_configuration: AtomicBusConfiguration,
}

impl XcopeVst3Processor {
    fn new(shared: Arc<XcopeShared>) -> Self {
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
        if data.numSamples <= 0 || data.symbolicSampleSize != SymbolicSampleSizes_::kSample32 as i32
        {
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

struct XcopeVst3Controller {
    shared: Arc<XcopeShared>,
}

impl XcopeVst3Controller {
    fn new(shared: Arc<XcopeShared>) -> Self {
        Self { shared }
    }
}
impl Drop for XcopeVst3Controller {
    fn drop(&mut self) {
        release_shared_for_role(&self.shared, SharedRole::Controller);
    }
}
impl Class for XcopeVst3Controller {
    type Interfaces = (IEditController,);
}
impl IPluginBaseTrait for XcopeVst3Controller {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }
    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IEditControllerTrait for XcopeVst3Controller {
    unsafe fn setComponentState(&self, state: *mut IBStream) -> tresult {
        unsafe { XcopeVst3Processor::new(self.shared.clone()).setState(state) }
    }
    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        unsafe { self.setComponentState(state) }
    }
    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        unsafe { XcopeVst3Processor::new(self.shared.clone()).getState(state) }
    }
    unsafe fn getParameterCount(&self) -> int32 {
        param_count() as i32
    }

    unsafe fn getParameterInfo(&self, index: int32, info: *mut ParameterInfo) -> tresult {
        if info.is_null() || index < 0 || index as u32 >= param_count() {
            return kInvalidArgument;
        }
        let param_id = index as u32 + 1;
        let info = unsafe { &mut *info };
        info.id = param_id;
        copy_wstring(param_title(param_id), &mut info.title);
        copy_wstring(param_title(param_id), &mut info.shortTitle);
        copy_wstring(param_units(param_id), &mut info.units);
        info.stepCount = param_steps(param_id);
        info.defaultNormalizedValue =
            read_param_normalized(&XcopeShared::new().params, param_id).unwrap_or(0.0);
        info.unitId = 0;
        info.flags = ParameterInfo_::ParameterFlags_::kCanAutomate;
        kResultOk
    }

    unsafe fn getParamStringByValue(
        &self,
        id: ParamID,
        value_normalized: ParamValue,
        string: *mut String128,
    ) -> tresult {
        if string.is_null() {
            return kInvalidArgument;
        }
        copy_wstring(&param_display_string(id, value_normalized), unsafe {
            &mut *string
        });
        kResultOk
    }

    unsafe fn getParamValueByString(
        &self,
        id: ParamID,
        _string: *mut TChar,
        value_normalized: *mut ParamValue,
    ) -> tresult {
        if value_normalized.is_null() || ParamId::from_raw(id).is_none() {
            return kInvalidArgument;
        }
        unsafe {
            *value_normalized =
                read_param_normalized(self.shared.params.as_ref(), id).unwrap_or(0.0)
        };
        kResultOk
    }

    unsafe fn normalizedParamToPlain(
        &self,
        _id: ParamID,
        value_normalized: ParamValue,
    ) -> ParamValue {
        value_normalized
    }
    unsafe fn plainParamToNormalized(&self, _id: ParamID, plain_value: ParamValue) -> ParamValue {
        plain_value.clamp(0.0, 1.0)
    }
    unsafe fn getParamNormalized(&self, id: ParamID) -> ParamValue {
        read_param_normalized(self.shared.params.as_ref(), id).unwrap_or(0.0)
    }
    unsafe fn setParamNormalized(&self, id: ParamID, value: ParamValue) -> tresult {
        if apply_param_normalized(self.shared.params.as_ref(), id, value) {
            kResultOk
        } else {
            kInvalidArgument
        }
    }
    unsafe fn setComponentHandler(&self, _handler: *mut IComponentHandler) -> tresult {
        kResultOk
    }

    unsafe fn createView(&self, name: FIDString) -> *mut IPlugView {
        if name.is_null() {
            return ptr::null_mut();
        }
        let requested = unsafe { CStr::from_ptr(name) };
        let editor = unsafe { CStr::from_ptr(ViewType::kEditor) };
        if requested.to_bytes() != editor.to_bytes() {
            return ptr::null_mut();
        }

        let (width, height) = preferred_window_size();
        let adapter = XcopeVst3GuiAdapter::new(self.shared.clone());
        let Some(view) =
            ComWrapper::new(HostedVst3View::new(adapter, width, height)).to_com_ptr::<IPlugView>()
        else {
            return ptr::null_mut();
        };
        ComPtr::into_raw(view)
    }
}

struct XcopeVst3GuiAdapter {
    shared: Arc<XcopeShared>,
    gui: XcopeGui,
}
impl XcopeVst3GuiAdapter {
    fn new(shared: Arc<XcopeShared>) -> Self {
        Self {
            shared,
            gui: XcopeGui::default(),
        }
    }
}

impl Vst3HostedGui for XcopeVst3GuiAdapter {
    fn set_parent_raw(&mut self, parent: toybox::raw_window_handle::RawWindowHandle) {
        self.gui.set_parent_raw(parent);
    }
    fn open(&mut self) -> bool {
        self.gui.open(self.shared.clone()).is_ok()
    }
    fn close(&mut self) {
        self.gui.close();
    }
    fn last_size(&self) -> Option<(u32, u32)> {
        self.gui.last_size()
    }
    fn request_resize(&self, width: u32, height: u32) {
        self.gui.request_resize(width, height);
    }
}

#[derive(Default)]
struct XcopeVst3Factory;
impl Class for XcopeVst3Factory {
    type Interfaces = (IPluginFactory,);
}

impl IPluginFactoryTrait for XcopeVst3Factory {
    unsafe fn getFactoryInfo(&self, info: *mut PFactoryInfo) -> tresult {
        if info.is_null() {
            return kInvalidArgument;
        }
        let info = unsafe { &mut *info };
        copy_cstring("portalsurfer", &mut info.vendor);
        copy_cstring("https://github.com/PORTALSURFER/xcope", &mut info.url);
        copy_cstring("support@localhost", &mut info.email);
        info.flags = PFactoryInfo_::FactoryFlags_::kUnicode as i32;
        kResultOk
    }

    unsafe fn countClasses(&self) -> i32 {
        2
    }

    unsafe fn getClassInfo(&self, index: i32, info: *mut PClassInfo) -> tresult {
        if info.is_null() {
            return kInvalidArgument;
        }
        let info = unsafe { &mut *info };
        match index {
            0 => {
                write_class_info_many(
                    info,
                    PROCESSOR_CID,
                    CATEGORY_AUDIO_MODULE_CLASS,
                    PLUGIN_NAME,
                );
                kResultOk
            }
            1 => {
                write_class_info_many(
                    info,
                    CONTROLLER_CID,
                    CATEGORY_COMPONENT_CONTROLLER_CLASS,
                    PLUGIN_NAME,
                );
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }

    unsafe fn createInstance(
        &self,
        cid: FIDString,
        iid: FIDString,
        obj: *mut *mut c_void,
    ) -> tresult {
        if cid.is_null() || iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let class_id = unsafe { *(cid as *const TUID) };
        let instance = match class_id {
            PROCESSOR_CID => ComWrapper::new(XcopeVst3Processor::new(acquire_shared_for_role(
                SharedRole::Processor,
            )))
            .to_com_ptr::<FUnknown>(),
            CONTROLLER_CID => ComWrapper::new(XcopeVst3Controller::new(acquire_shared_for_role(
                SharedRole::Controller,
            )))
            .to_com_ptr::<FUnknown>(),
            _ => None,
        };
        let Some(instance) = instance else {
            return kInvalidArgument;
        };
        let ptr = instance.as_ptr();
        unsafe { ((*(*ptr).vtbl).queryInterface)(ptr, iid as *mut TUID, obj) }
    }
}

toybox::vst3_plugin_entry!(XcopeVst3Factory);

fn transport_from_context(
    process_context: *mut ProcessContext,
    num_samples: i32,
    sample_rate_hz: f32,
    previous: TransportSnapshot,
) -> TransportSnapshot {
    let Some(ctx) = (unsafe { process_context.as_ref() }) else {
        return previous;
    };
    let flags = ctx.state;
    let tempo_valid =
        (flags & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kTempoValid)) != 0;
    let pos_valid = (flags
        & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kProjectTimeMusicValid))
        != 0;
    let is_playing =
        (flags & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kPlaying)) != 0;
    let song_pos_beats = if pos_valid {
        let base = ctx.projectTimeMusic;
        let tempo = if tempo_valid {
            ctx.tempo as f32
        } else {
            previous.tempo_bpm
        };
        Some(project_song_position_beats(
            base,
            tempo,
            is_playing && tempo_valid,
            num_samples,
            sample_rate_hz,
        ))
    } else {
        previous.song_pos_beats
    };
    TransportSnapshot {
        tempo_bpm: if tempo_valid {
            ctx.tempo as f32
        } else {
            previous.tempo_bpm
        },
        is_playing,
        song_pos_beats,
        time_sig_num: if (flags
            & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kTimeSigValid))
            != 0
        {
            ctx.timeSigNumerator as u16
        } else {
            previous.time_sig_num
        },
        time_sig_denom: if (flags
            & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kTimeSigValid))
            != 0
        {
            ctx.timeSigDenominator as u16
        } else {
            previous.time_sig_denom
        },
    }
}

fn param_title(param_id: u32) -> &'static str {
    match ParamId::from_raw(param_id) {
        Some(ParamId::ScopeMode) => "Mode",
        Some(ParamId::TimeWindow) => "Window",
        Some(ParamId::GridSubdivision) => "Grid",
        Some(ParamId::GridTriplet) => "Triplet",
        Some(ParamId::DisplayMode) => "Display",
        Some(ParamId::Freeze) => "Freeze",
        Some(ParamId::ZoomX) => "Zoom X",
        Some(ParamId::ZoomY) => "Zoom Y",
        Some(ParamId::Channel1Visible) => "Ch1",
        Some(ParamId::Channel2Visible) => "Ch2",
        Some(ParamId::Channel1Color) => "Ch1 Color",
        Some(ParamId::Channel2Color) => "Ch2 Color",
        None => "Unknown",
    }
}

fn param_units(param_id: u32) -> &'static str {
    match ParamId::from_raw(param_id) {
        Some(ParamId::ZoomX | ParamId::ZoomY) => "x",
        _ => "",
    }
}

fn param_steps(param_id: u32) -> i32 {
    match ParamId::from_raw(param_id) {
        Some(
            ParamId::ScopeMode
            | ParamId::DisplayMode
            | ParamId::GridTriplet
            | ParamId::Freeze
            | ParamId::Channel1Visible
            | ParamId::Channel2Visible,
        ) => 1,
        Some(ParamId::TimeWindow) => 3,
        Some(ParamId::GridSubdivision) => 2,
        Some(ParamId::Channel1Color | ParamId::Channel2Color) => 7,
        _ => 0,
    }
}

fn param_display_string(param_id: u32, normalized: f64) -> String {
    match ParamId::from_raw(param_id) {
        Some(ParamId::ZoomX | ParamId::ZoomY) => format!("{:.2}x", normalized),
        Some(_) => format!("{normalized:.3}"),
        None => "-".to_string(),
    }
}

unsafe fn process_input_buses(data: &ProcessData) -> Option<&[AudioBusBuffers]> {
    if data.numInputs <= 0 || data.inputs.is_null() {
        return None;
    }
    let bus_count = usize::try_from(data.numInputs).ok()?;
    Some(unsafe { slice::from_raw_parts(data.inputs, bus_count) })
}

unsafe fn process_first_output_bus(data: &ProcessData) -> Option<&AudioBusBuffers> {
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

unsafe fn read_bus_left_sample(bus: &AudioBusBuffers, sample_index: usize) -> Option<f32> {
    let channels = unsafe { bus_channel_buffers_32(bus) }?;
    let left = channels.first().copied()?;
    if left.is_null() {
        return None;
    }
    Some(unsafe { *left.add(sample_index) })
}

unsafe fn copy_main_bus_to_output(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_arrangement_support_is_stereo_or_mono_only() {
        assert!(is_supported_bus_arrangement(SpeakerArr::kMono));
        assert!(is_supported_bus_arrangement(SpeakerArr::kStereo));
        assert!(!is_supported_bus_arrangement(SpeakerArr::k31Cine));
    }

    #[test]
    fn arrangement_channel_count_tracks_supported_layout() {
        assert_eq!(
            BusChannelLayout::from_arrangement(SpeakerArr::kMono)
                .expect("mono arrangement should map to channel layout")
                .channel_count(),
            1
        );
        assert_eq!(
            BusChannelLayout::from_arrangement(SpeakerArr::kStereo)
                .expect("stereo arrangement should map to channel layout")
                .channel_count(),
            2
        );
    }

    #[test]
    fn default_bus_configuration_enables_only_main_input() {
        let config = AtomicBusConfiguration::default();
        assert_eq!(config.output_layout(), BusChannelLayout::Stereo);
        assert_eq!(config.input_layout(0), BusChannelLayout::Stereo);
        assert_eq!(config.input_layout(1), BusChannelLayout::Stereo);
        assert_eq!(config.input_active_mask(), 1);
    }
}
