//! VST3 plugin factory and class registration entrypoint.

use std::ffi::c_void;

use toybox::vst3::prelude::Steinberg::*;
use toybox::vst3::prelude::*;

use crate::constants::PLUGIN_NAME;

use super::controller::XcopeVst3Controller;
use super::processor::XcopeVst3Processor;
use super::shared_registry::{acquire_shared_for_role, SharedRole};
use super::{CONTROLLER_CID, PROCESSOR_CID};

#[derive(Default)]
pub(super) struct XcopeVst3Factory;

impl Class for XcopeVst3Factory {
    type Interfaces = (IPluginFactory,);
}

impl IPluginFactoryTrait for XcopeVst3Factory {
    unsafe fn getFactoryInfo(&self, info: *mut PFactoryInfo) -> tresult {
        if info.is_null() {
            return kInvalidArgument;
        }
        let info = unsafe { &mut *info };
        copy_cstring("PORTALSURFER", &mut info.vendor);
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
