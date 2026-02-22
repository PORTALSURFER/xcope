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
    let samples_per_beat = samples_per_beat(sample_rate_hz, tempo_bpm)?;
    let extrapolated = extrapolate_anchor_beats(previous_anchor, anchor_sample, samples_per_beat);

    let ctx = unsafe { process_context.as_ref() }?;
    let flags = ctx.state;
    let pos_valid = (flags
        & vst3_process_state_flag(ProcessContext_::StatesAndFlags_::kProjectTimeMusicValid))
        != 0;
    if !pos_valid || !ctx.projectTimeMusic.is_finite() {
        return extrapolated.or_else(|| previous_anchor.map(|(_, beats)| beats));
    }

    let block_beats = (num_samples.max(0) as f64) / samples_per_beat;
    let default_host_end = host_end_of_block_default(ctx.projectTimeMusic, block_beats);
    let Some(expected) = extrapolated else {
        return Some(default_host_end);
    };
    let candidate_direct = ctx.projectTimeMusic;
    let candidate_projected = ctx.projectTimeMusic + block_beats;
    let direct_error = (candidate_direct - expected).abs();
    let projected_error = (candidate_projected - expected).abs();
    let candidate = if direct_error <= projected_error {
        candidate_direct
    } else {
        candidate_projected
    };
    let continuity_threshold = (block_beats.abs() * 2.0).max(1.0e-4);
    if (candidate - expected).abs() <= continuity_threshold {
        Some(candidate)
    } else {
        // Host timeline likely jumped (seek/loop). Re-anchor to host reference.
        Some(default_host_end)
    }
}

fn samples_per_beat(sample_rate_hz: f32, tempo_bpm: f32) -> Option<f64> {
    let tempo = tempo_bpm.max(1.0) as f64;
    let samples_per_beat = (sample_rate_hz.max(1.0) as f64 * 60.0) / tempo;
    if !samples_per_beat.is_finite() || samples_per_beat <= 0.0 {
        None
    } else {
        Some(samples_per_beat)
    }
}

fn extrapolate_anchor_beats(
    previous_anchor: Option<(u64, f64)>,
    anchor_sample: u64,
    samples_per_beat: f64,
) -> Option<f64> {
    let (previous_sample, previous_beats) = previous_anchor?;
    let delta_samples = anchor_sample.saturating_sub(previous_sample) as f64;
    Some(previous_beats + (delta_samples / samples_per_beat))
}

fn host_end_of_block_default(project_time_music: f64, block_beats: f64) -> f64 {
    // Keep VST3 start-of-block projection as the robust default when we cannot
    // disambiguate host reference conventions yet.
    project_time_music + block_beats
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
    fn anchor_beats_extrapolates_from_previous_when_position_invalid() {
        let mut ctx = context_with_music(0.0, false);
        let beats =
            resolve_end_of_block_anchor_beats(&mut ctx, 48, 48.0, 60.0, Some((100, 20.0)), 148)
                .expect("anchor beats should extrapolate from previous");
        assert!((beats - 21.0).abs() < 1.0e-9);
    }

    #[test]
    fn anchor_beats_stays_phase_locked_under_mixed_host_reference_points() {
        let sample_rate_hz = 48_000.0;
        let tempo_bpm = 120.0;
        let block_samples = 480;
        let samples_per_beat = (sample_rate_hz * 60.0) / tempo_bpm;
        let block_beats = block_samples as f64 / samples_per_beat as f64;

        let mut previous_anchor: Option<(u64, f64)> = None;
        let mut anchor_sample = 0u64;

        for index in 0..128u64 {
            anchor_sample += block_samples as u64;
            let expected_end = (index + 1) as f64 * block_beats;
            let host_reports_end = index % 2 == 1;
            let reported = if host_reports_end {
                expected_end
            } else {
                expected_end - block_beats
            };
            let jitter = match index % 3 {
                0 => 1.0e-7,
                1 => -1.0e-7,
                _ => 0.0,
            };

            let mut ctx = context_with_music(reported + jitter, true);
            let resolved = resolve_end_of_block_anchor_beats(
                &mut ctx,
                block_samples,
                sample_rate_hz,
                tempo_bpm,
                previous_anchor,
                anchor_sample,
            )
            .expect("anchor beats should resolve");

            assert!(
                (resolved - expected_end).abs() < 1.0e-4,
                "block {index}: expected {expected_end}, got {resolved}"
            );

            if let Some((previous_sample, previous_beats)) = previous_anchor {
                let delta_samples = (anchor_sample - previous_sample) as f64;
                let delta_beats = delta_samples / samples_per_beat as f64;
                assert!(
                    (resolved - (previous_beats + delta_beats)).abs() < 1.0e-4,
                    "block {index}: progression mismatch"
                );
            }

            previous_anchor = Some((anchor_sample, resolved));
        }
    }

    #[test]
    fn anchor_beats_recovers_after_temporarily_missing_position_flags() {
        let sample_rate_hz = 48_000.0;
        let tempo_bpm = 120.0;
        let block_samples = 480;
        let samples_per_beat = (sample_rate_hz * 60.0) / tempo_bpm;
        let block_beats = block_samples as f64 / samples_per_beat as f64;

        let mut previous_anchor: Option<(u64, f64)> = None;
        let mut anchor_sample = 0u64;
        let mut expected_end = 0.0f64;

        // Warm-up with valid position.
        for _ in 0..8 {
            anchor_sample += block_samples as u64;
            expected_end += block_beats;
            let mut ctx = context_with_music(expected_end - block_beats, true);
            let resolved = resolve_end_of_block_anchor_beats(
                &mut ctx,
                block_samples,
                sample_rate_hz,
                tempo_bpm,
                previous_anchor,
                anchor_sample,
            )
            .expect("anchor beats should resolve");
            previous_anchor = Some((anchor_sample, resolved));
        }

        // Simulate host blocks where position is temporarily unavailable.
        for _ in 0..4 {
            anchor_sample += block_samples as u64;
            expected_end += block_beats;
            let mut ctx = context_with_music(0.0, false);
            let resolved = resolve_end_of_block_anchor_beats(
                &mut ctx,
                block_samples,
                sample_rate_hz,
                tempo_bpm,
                previous_anchor,
                anchor_sample,
            )
            .expect("fallback to previous anchor should resolve");
            assert!(
                (resolved - expected_end).abs() < 1.0e-4,
                "expected extrapolated {expected_end}, got {resolved}"
            );
            previous_anchor = Some((anchor_sample, resolved));
        }

        // When valid position returns, anchor should quickly re-lock.
        for _ in 0..8 {
            anchor_sample += block_samples as u64;
            expected_end += block_beats;
            let mut ctx = context_with_music(expected_end - block_beats, true);
            let resolved = resolve_end_of_block_anchor_beats(
                &mut ctx,
                block_samples,
                sample_rate_hz,
                tempo_bpm,
                previous_anchor,
                anchor_sample,
            )
            .expect("anchor beats should resolve");
            assert!(
                (resolved - expected_end).abs() < block_beats * 1.1,
                "expected relock near {expected_end}, got {resolved}"
            );
            previous_anchor = Some((anchor_sample, resolved));
        }
    }

    #[test]
    fn anchor_beats_stays_continuous_when_position_flags_flicker() {
        let sample_rate_hz = 48_000.0;
        let tempo_bpm = 120.0;
        let block_samples = 480;
        let samples_per_beat = (sample_rate_hz * 60.0) / tempo_bpm;
        let block_beats = block_samples as f64 / samples_per_beat as f64;
        let mut previous_anchor: Option<(u64, f64)> = None;
        let mut anchor_sample = 0u64;
        let mut expected_end = 0.0f64;

        for index in 0..128 {
            anchor_sample += block_samples as u64;
            expected_end += block_beats;
            let pos_valid = index % 2 == 0;
            let reported = expected_end - block_beats;
            let mut ctx = context_with_music(reported, pos_valid);
            let resolved = resolve_end_of_block_anchor_beats(
                &mut ctx,
                block_samples,
                sample_rate_hz,
                tempo_bpm,
                previous_anchor,
                anchor_sample,
            )
            .expect("anchor beats should resolve");
            assert!(
                (resolved - expected_end).abs() < 1.0e-4,
                "block {index}: expected {expected_end}, got {resolved}"
            );
            previous_anchor = Some((anchor_sample, resolved));
        }
    }
}
