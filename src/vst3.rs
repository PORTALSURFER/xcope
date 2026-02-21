//! VST3 processor/controller scaffold for Xcope.

#![allow(clippy::missing_docs_in_private_items)]

use std::ffi::{c_void, CStr};
use std::ptr;
use std::slice;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use toybox::vst3::prelude::Steinberg::*;
use toybox::vst3::prelude::*;

use crate::constants::{ParamId, MAX_VISUAL_CHANNELS, PLUGIN_NAME, STATE_MAGIC, STATE_VERSION};
use crate::gui::{preferred_window_size, XcopeGui};
use crate::params::{apply_param_normalized, param_count, read_param_normalized};
use crate::state_io::{decode_state_payload, encode_state_payload};
use crate::transport::TransportSnapshot;
use crate::XcopeShared;

const PROCESSOR_CID: TUID = uid(0x9AF47871, 0x00A645F3, 0x9D8A34AA, 0x7D4E7821);
const CONTROLLER_CID: TUID = uid(0x0B49357D, 0xF45A4D2D, 0xA67A66AE, 0xD7C24B7A);
const DEFAULT_MAIN_SPEAKER_ARRANGEMENT: SpeakerArrangement = SpeakerArr::kStereo;

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

fn is_supported_main_arrangement(arrangement: SpeakerArrangement) -> bool {
    matches!(arrangement, SpeakerArr::kMono | SpeakerArr::kStereo)
}

fn channel_count_for_arrangement(arrangement: SpeakerArrangement) -> i32 {
    match arrangement {
        SpeakerArr::kMono => 1,
        SpeakerArr::kStereo => 2,
        _ => 2,
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
    main_arrangement: Mutex<SpeakerArrangement>,
}

impl XcopeVst3Processor {
    fn new(shared: Arc<XcopeShared>) -> Self {
        Self {
            shared,
            main_arrangement: Mutex::new(DEFAULT_MAIN_SPEAKER_ARRANGEMENT),
        }
    }

    fn read_main_arrangement(&self) -> SpeakerArrangement {
        self.main_arrangement
            .lock()
            .map(|guard| *guard)
            .unwrap_or(DEFAULT_MAIN_SPEAKER_ARRANGEMENT)
    }

    fn write_main_arrangement(&self, arrangement: SpeakerArrangement) {
        if let Ok(mut guard) = self.main_arrangement.lock() {
            *guard = arrangement;
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
                BusDirections_::kInput | BusDirections_::kOutput => 1,
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
        if bus.is_null() || index != 0 {
            return kInvalidArgument;
        }
        if media_type as MediaTypes != MediaTypes_::kAudio {
            return kInvalidArgument;
        }
        let label = match dir as BusDirections {
            BusDirections_::kInput => "Input",
            BusDirections_::kOutput => "Output",
            _ => return kInvalidArgument,
        };
        let bus = unsafe { &mut *bus };
        bus.mediaType = MediaTypes_::kAudio as MediaType;
        bus.direction = dir;
        bus.channelCount = channel_count_for_arrangement(self.read_main_arrangement());
        copy_wstring(label, &mut bus.name);
        bus.busType = BusTypes_::kMain as BusType;
        bus.flags = vst3_bus_flag(BusInfo_::BusFlags_::kDefaultActive);
        kResultOk
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
        _media_type: MediaType,
        _dir: BusDirection,
        _index: i32,
        _state: TBool,
    ) -> tresult {
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
        if num_ins != 1 || num_outs != 1 {
            return kResultFalse;
        }
        if inputs.is_null() || outputs.is_null() {
            return kInvalidArgument;
        }
        let in_arr = unsafe { *inputs };
        let out_arr = unsafe { *outputs };
        if in_arr != out_arr || !is_supported_main_arrangement(in_arr) {
            return kResultFalse;
        }
        self.write_main_arrangement(in_arr);
        kResultTrue
    }

    unsafe fn getBusArrangement(
        &self,
        dir: BusDirection,
        index: i32,
        arr: *mut SpeakerArrangement,
    ) -> tresult {
        if arr.is_null() || index != 0 {
            return kInvalidArgument;
        }
        match dir as BusDirections {
            BusDirections_::kInput | BusDirections_::kOutput => {
                unsafe { *arr = self.read_main_arrangement() };
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

        self.shared
            .transport
            .update(transport_from_context(data.processContext));
        if data.numSamples <= 0 || data.symbolicSampleSize != SymbolicSampleSizes_::kSample32 as i32
        {
            return process_ok();
        }
        let Some((in_channels, out_channels)) = (unsafe { first_bus_channel_lists(data) }) else {
            return process_ok();
        };
        let channel_count = in_channels.len().min(out_channels.len());
        if channel_count == 0 {
            return process_ok();
        }
        let sample_count = data.numSamples as usize;
        let capture_channels = channel_count.min(MAX_VISUAL_CHANNELS);

        for sample_index in 0..sample_count {
            let mut capture_sample = [0.0f32; MAX_VISUAL_CHANNELS];
            for channel_index in 0..channel_count {
                if in_channels[channel_index].is_null() || out_channels[channel_index].is_null() {
                    continue;
                }
                let value = unsafe { *in_channels[channel_index].add(sample_index) };
                unsafe {
                    *out_channels[channel_index].add(sample_index) = value;
                }
                if channel_index < MAX_VISUAL_CHANNELS {
                    capture_sample[channel_index] = value;
                }
            }
            self.shared
                .scope_buffer
                .write_sample(capture_sample, capture_channels);
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

fn transport_from_context(process_context: *mut ProcessContext) -> TransportSnapshot {
    let Some(ctx) = (unsafe { process_context.as_ref() }) else {
        return TransportSnapshot::default();
    };
    let flags = ctx.state;
    let tempo_valid =
        (flags & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kTempoValid)) != 0;
    let pos_valid = (flags
        & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kProjectTimeMusicValid))
        != 0;
    TransportSnapshot {
        tempo_bpm: if tempo_valid { ctx.tempo as f32 } else { 120.0 },
        is_playing: (flags & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kPlaying))
            != 0,
        song_pos_beats: pos_valid.then_some(ctx.projectTimeMusic),
        time_sig_num: if (flags
            & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kTimeSigValid))
            != 0
        {
            ctx.timeSigNumerator as u16
        } else {
            4
        },
        time_sig_denom: if (flags
            & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kTimeSigValid))
            != 0
        {
            ctx.timeSigDenominator as u16
        } else {
            4
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
        Some(ParamId::Channel3Visible) => "Ch3",
        Some(ParamId::Channel4Visible) => "Ch4",
        Some(ParamId::Channel1Color) => "Ch1 Color",
        Some(ParamId::Channel2Color) => "Ch2 Color",
        Some(ParamId::Channel3Color) => "Ch3 Color",
        Some(ParamId::Channel4Color) => "Ch4 Color",
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
            | ParamId::Channel2Visible
            | ParamId::Channel3Visible
            | ParamId::Channel4Visible,
        ) => 1,
        Some(ParamId::TimeWindow) => 3,
        Some(ParamId::GridSubdivision) => 2,
        Some(
            ParamId::Channel1Color
            | ParamId::Channel2Color
            | ParamId::Channel3Color
            | ParamId::Channel4Color,
        ) => 7,
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

unsafe fn first_bus_channel_lists(data: &ProcessData) -> Option<(&[*mut f32], &[*mut f32])> {
    if data.numInputs <= 0
        || data.numOutputs <= 0
        || data.inputs.is_null()
        || data.outputs.is_null()
    {
        return None;
    }
    let input_bus_count = usize::try_from(data.numInputs).ok()?;
    let output_bus_count = usize::try_from(data.numOutputs).ok()?;
    let input_buses = unsafe { slice::from_raw_parts(data.inputs, input_bus_count) };
    let output_buses = unsafe { slice::from_raw_parts(data.outputs, output_bus_count) };
    let input_bus = input_buses.first()?;
    let output_bus = output_buses.first()?;

    let channel_count = usize::try_from(input_bus.numChannels.min(output_bus.numChannels)).ok()?;
    if channel_count == 0 {
        return None;
    }
    let input_channels_ptr = unsafe { input_bus.__field0.channelBuffers32 };
    let output_channels_ptr = unsafe { output_bus.__field0.channelBuffers32 };
    if input_channels_ptr.is_null() || output_channels_ptr.is_null() {
        return None;
    }
    Some((
        unsafe { slice::from_raw_parts(input_channels_ptr, channel_count) },
        unsafe { slice::from_raw_parts(output_channels_ptr, channel_count) },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_arrangement_support_is_stereo_or_mono_only() {
        assert!(is_supported_main_arrangement(SpeakerArr::kMono));
        assert!(is_supported_main_arrangement(SpeakerArr::kStereo));
        assert!(!is_supported_main_arrangement(SpeakerArr::k31Cine));
    }

    #[test]
    fn main_arrangement_channel_count_tracks_supported_layout() {
        assert_eq!(channel_count_for_arrangement(SpeakerArr::kMono), 1);
        assert_eq!(channel_count_for_arrangement(SpeakerArr::kStereo), 2);
    }
}
