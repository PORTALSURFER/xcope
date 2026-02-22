//! Process-context to transport-snapshot conversion.

use toybox::vst3::prelude::*;

use crate::transport::{project_song_position_beats, TransportSnapshot};

use super::vst3_process_state_flag;

/// Build one transport snapshot from the current VST3 process context.
pub(super) fn transport_from_context(
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
