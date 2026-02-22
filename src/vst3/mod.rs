//! VST3 processor/controller scaffold for Xcope.

#![allow(clippy::missing_docs_in_private_items)]

use toybox::vst3::prelude::Steinberg::*;
use toybox::vst3::prelude::*;

use crate::constants::MAX_VISUAL_CHANNELS;

mod audio_io;
mod bus;
mod controller;
mod factory;
mod param_meta;
mod processor;
mod shared_registry;
mod transport_context;

#[cfg(test)]
mod tests;

pub(super) const PROCESSOR_CID: TUID = uid(0x9AF47871, 0x00A645F3, 0x9D8A34AA, 0x7D4E7821);
pub(super) const CONTROLLER_CID: TUID = uid(0x0B49357D, 0xF45A4D2D, 0xA67A66AE, 0xD7C24B7A);
pub(super) const INPUT_SOURCE_BUS_COUNT: usize = MAX_VISUAL_CHANNELS;

#[cfg(target_os = "windows")]
pub(super) const fn vst3_bus_flag(flag: i32) -> u32 {
    flag as u32
}

#[cfg(not(target_os = "windows"))]
pub(super) const fn vst3_bus_flag(flag: u32) -> u32 {
    flag
}

#[cfg(target_os = "windows")]
pub(super) const fn vst3_process_state_flag(flag: i32) -> u32 {
    flag as u32
}

#[cfg(not(target_os = "windows"))]
pub(super) const fn vst3_process_state_flag(flag: u32) -> u32 {
    flag
}

#[cfg(target_os = "windows")]
pub(super) const fn vst3_process_requirement_flag(flag: i32) -> u32 {
    flag as u32
}

#[cfg(not(target_os = "windows"))]
pub(super) const fn vst3_process_requirement_flag(flag: u32) -> u32 {
    flag
}
