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

/// Resolve song-position beats aligned to the current end-of-block sample.
///
/// The host may report `projectTimeMusic` at different block reference points.
/// This helper derives two plausible end-of-block candidates and picks the one
/// that best matches the previously observed anchor progression.
pub(super) fn resolve_end_of_block_anchor_beats(
    process_context: *mut ProcessContext,
    num_samples: i32,
    sample_rate_hz: f32,
    tempo_bpm: f32,
    previous_anchor: Option<(u64, f64)>,
    anchor_sample: u64,
) -> Option<f64> {
    let ctx = unsafe { process_context.as_ref() }?;
    let flags = ctx.state;
    let pos_valid = (flags
        & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kProjectTimeMusicValid))
        != 0;
    if !pos_valid || !ctx.projectTimeMusic.is_finite() {
        return previous_anchor.map(|(_, beats)| beats);
    }

    let tempo = tempo_bpm.max(1.0) as f64;
    let samples_per_beat = (sample_rate_hz.max(1.0) as f64 * 60.0) / tempo;
    if !samples_per_beat.is_finite() || samples_per_beat <= 0.0 {
        return Some(ctx.projectTimeMusic);
    }

    let block_beats = (num_samples.max(0) as f64) / samples_per_beat;
    let candidate_direct = ctx.projectTimeMusic;
    let candidate_projected = ctx.projectTimeMusic + block_beats;

    if let Some((previous_sample, previous_beats)) = previous_anchor {
        let delta_samples = anchor_sample.saturating_sub(previous_sample) as f64;
        let expected = previous_beats + (delta_samples / samples_per_beat);
        let direct_error = (candidate_direct - expected).abs();
        let projected_error = (candidate_projected - expected).abs();
        if direct_error <= projected_error {
            Some(candidate_direct)
        } else {
            Some(candidate_projected)
        }
    } else {
        // Default to the VST3 reference-point assumption (block start).
        Some(candidate_projected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context_with_music(project_time_music: f64, pos_valid: bool) -> ProcessContext {
        let mut ctx: ProcessContext = unsafe { std::mem::zeroed() };
        if pos_valid {
            ctx.state =
                vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kProjectTimeMusicValid);
        }
        ctx.projectTimeMusic = project_time_music;
        ctx
    }

    #[test]
    fn anchor_beats_defaults_to_projected_end_when_no_history() {
        let mut ctx = context_with_music(10.0, true);
        let beats = resolve_end_of_block_anchor_beats(&mut ctx, 48, 48.0, 60.0, None, 0)
            .expect("anchor beats should resolve");
        assert!((beats - 11.0).abs() < 1.0e-9);
    }

    #[test]
    fn anchor_beats_prefers_candidate_closest_to_previous_progression() {
        let mut ctx = context_with_music(21.0, true);
        let beats =
            resolve_end_of_block_anchor_beats(&mut ctx, 48, 48.0, 60.0, Some((100, 20.0)), 148)
                .expect("anchor beats should resolve");
        assert!((beats - 21.0).abs() < 1.0e-9);
    }

    #[test]
    fn anchor_beats_falls_back_to_previous_when_position_invalid() {
        let mut ctx = context_with_music(0.0, false);
        let beats =
            resolve_end_of_block_anchor_beats(&mut ctx, 48, 48.0, 60.0, Some((100, 20.0)), 148)
                .expect("anchor beats should reuse previous");
        assert!((beats - 20.0).abs() < 1.0e-9);
    }
}
