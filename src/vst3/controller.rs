//! VST3 controller and hosted GUI adapter.

use std::ffi::CStr;
use std::ptr;
use std::sync::Arc;

use toybox::vst3::prelude::Steinberg::*;
use toybox::vst3::prelude::*;

use crate::constants::ParamId;
use crate::gui::{preferred_window_size, XcopeGui};
use crate::params::{apply_param_normalized, param_count, read_param_normalized};
use crate::XcopeShared;

use super::param_meta::{param_display_string, param_steps, param_title, param_units};
use super::processor::XcopeVst3Processor;
use super::shared_registry::{release_shared_for_role, SharedRole};

pub(super) struct XcopeVst3Controller {
    shared: Arc<XcopeShared>,
}

impl XcopeVst3Controller {
    pub(super) fn new(shared: Arc<XcopeShared>) -> Self {
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
