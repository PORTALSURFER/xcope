//! VST3 unit tests focused on bus arrangement and defaults.

use toybox::vst3::prelude::SpeakerArr;

use super::bus::{is_supported_bus_arrangement, AtomicBusConfiguration, BusChannelLayout};

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
