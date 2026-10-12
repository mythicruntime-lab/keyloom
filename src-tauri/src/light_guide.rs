//! Stateful MIDI guide output. This controller never owns or opens a device.

/// Tracks pitches that may still be lit, including ambiguous failed deliveries.
/// A group identity must change for every new target occurrence, even repeated
/// chords. Call `cleanup` before dropping or replacing the connected output.
#[derive(Debug, Default)]
pub struct GuideNotes {
    active: u128,
    uncertain: u128,
    group: Option<u64>,
}

impl GuideNotes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sorted pitches that are confirmed or possibly active. A failed Note On
    /// remains here until a successful Note Off establishes that it is clear.
    pub fn lit_notes(&self) -> Vec<u8> {
        (0..=127)
            .filter(|pitch| self.active & (1_u128 << pitch) != 0)
            .collect()
    }

    /// Sends required Note Offs before Note Ons, on MIDI channel 1. Targets are
    /// deduplicated; unchanged successful output is silent. A new group identity
    /// rearticulates shared pitches. Stops at the first failed send so the caller
    /// can report/disconnect the failing device and later retry or clean up.
    pub fn update<F>(&mut self, target: Option<(u64, &[u8])>, mut send: F) -> Result<(), String>
    where
        F: FnMut([u8; 3]) -> Result<(), String>,
    {
        let mut wanted = 0_u128;
        let identity = target.map(|(identity, _)| identity);
        if let Some((_, pitches)) = target {
            for &pitch in pitches {
                if pitch > 127 {
                    return Err(format!("Guide pitch {pitch} is outside MIDI range 0..127"));
                }
                wanted |= 1_u128 << pitch;
            }
        }

        let to_clear = if self.group != identity {
            self.active
        } else {
            (self.active & !wanted) | self.uncertain
        };
        for pitch in 0..=127 {
            let bit = 1_u128 << pitch;
            if to_clear & bit != 0 {
                self.note_off(pitch, &mut send)?;
            }
        }

        // All previous-group notes are now off. Any notes sent below belong to
        // this group, including a partially delivered chord after an error.
        self.group = identity;
        for pitch in 0..=127 {
            let bit = 1_u128 << pitch;
            if wanted & bit != 0 && self.active & bit == 0 {
                // A failed write can still have reached the instrument. Record
                // the possibility before calling out, then confirm on success.
                self.active |= bit;
                self.uncertain |= bit;
                send([0x90, pitch, 64])
                    .map_err(|error| format!("Guide Note On {pitch} failed: {error}"))?;
                self.uncertain &= !bit;
            }
        }
        Ok(())
    }

    /// Attempts every tracked Note Off even if another send fails. Successful
    /// pitches clear immediately; failures remain tracked for a later retry.
    pub fn cleanup<F>(&mut self, mut send: F) -> Result<(), String>
    where
        F: FnMut([u8; 3]) -> Result<(), String>,
    {
        let mut errors = Vec::new();
        for pitch in 0..=127 {
            if self.active & (1_u128 << pitch) != 0 {
                if let Err(error) = self.note_off(pitch, &mut send) {
                    errors.push(error);
                }
            }
        }
        if self.active == 0 {
            self.group = None;
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    fn note_off<F>(&mut self, pitch: u8, send: &mut F) -> Result<(), String>
    where
        F: FnMut([u8; 3]) -> Result<(), String>,
    {
        let bit = 1_u128 << pitch;
        self.uncertain |= bit;
        send([0x80, pitch, 0])
            .map_err(|error| format!("Guide Note Off {pitch} failed: {error}"))?;
        self.active &= !bit;
        self.uncertain &= !bit;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::GuideNotes;

    fn update(guide: &mut GuideNotes, target: Option<(u64, &[u8])>) -> Vec<[u8; 3]> {
        let mut messages = Vec::new();
        guide
            .update(target, |message| {
                messages.push(message);
                Ok(())
            })
            .unwrap();
        messages
    }

    #[test]
    fn deduplicates_sorts_and_uses_channel_one() {
        let mut guide = GuideNotes::new();
        assert_eq!(
            update(&mut guide, Some((1, &[127, 60, 0, 60]))),
            vec![[0x90, 0, 64], [0x90, 60, 64], [0x90, 127, 64]]
        );
        assert_eq!(guide.lit_notes(), vec![0, 60, 127]);
        assert!(update(&mut guide, Some((1, &[60, 0, 127, 127]))).is_empty());
    }

    #[test]
    fn group_transition_turns_every_old_pitch_off_before_any_on() {
        let mut guide = GuideNotes::new();
        update(&mut guide, Some((10, &[60, 64])));
        assert_eq!(
            update(&mut guide, Some((11, &[64, 67]))),
            vec![[0x80, 60, 0], [0x80, 64, 0], [0x90, 64, 64], [0x90, 67, 64]]
        );
        assert_eq!(guide.lit_notes(), vec![64, 67]);
    }

    #[test]
    fn repeated_chord_rearticulates_when_identity_changes() {
        let mut guide = GuideNotes::new();
        update(&mut guide, Some((1, &[60, 67])));
        assert_eq!(
            update(&mut guide, Some((2, &[60, 67]))),
            vec![[0x80, 60, 0], [0x80, 67, 0], [0x90, 60, 64], [0x90, 67, 64]]
        );
        assert!(update(&mut guide, Some((2, &[60, 67]))).is_empty());
    }

    #[test]
    fn same_group_pitch_changes_preserve_shared_notes() {
        let mut guide = GuideNotes::new();
        update(&mut guide, Some((1, &[60, 64])));
        assert_eq!(
            update(&mut guide, Some((1, &[64, 67]))),
            vec![[0x80, 60, 0], [0x90, 67, 64]]
        );
    }

    #[test]
    fn none_clears_notes_and_repeated_none_is_silent() {
        let mut guide = GuideNotes::default();
        update(&mut guide, Some((1, &[60, 64])));
        assert_eq!(update(&mut guide, None), vec![[0x80, 60, 0], [0x80, 64, 0]]);
        assert!(guide.lit_notes().is_empty());
        assert!(update(&mut guide, None).is_empty());
        assert_eq!(update(&mut guide, Some((1, &[60]))), vec![[0x90, 60, 64]]);
    }

    #[test]
    fn failed_note_on_is_tracked_and_retry_clears_ambiguity_first() {
        let mut guide = GuideNotes::new();
        let mut attempted = Vec::new();
        let error = guide
            .update(Some((1, &[60, 64, 67])), |message| {
                attempted.push(message);
                if message[1] == 64 {
                    Err("disconnected".into())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert!(error.contains("Note On 64"));
        assert_eq!(attempted, vec![[0x90, 60, 64], [0x90, 64, 64]]);
        assert_eq!(guide.lit_notes(), vec![60, 64]);
        assert_eq!(
            update(&mut guide, Some((1, &[60, 64, 67]))),
            vec![[0x80, 64, 0], [0x90, 64, 64], [0x90, 67, 64]]
        );
        assert!(update(&mut guide, Some((1, &[60, 64, 67]))).is_empty());
    }

    #[test]
    fn failed_note_off_stops_update_and_keeps_only_remaining_notes() {
        let mut guide = GuideNotes::new();
        update(&mut guide, Some((1, &[60, 64, 67])));
        let mut attempted = Vec::new();
        guide
            .update(Some((2, &[64, 72])), |message| {
                attempted.push(message);
                if message[1] == 64 {
                    Err("port failed".into())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(attempted, vec![[0x80, 60, 0], [0x80, 64, 0]]);
        assert_eq!(guide.lit_notes(), vec![64, 67]);
        assert_eq!(
            update(&mut guide, Some((2, &[64, 72]))),
            vec![[0x80, 64, 0], [0x80, 67, 0], [0x90, 64, 64], [0x90, 72, 64]]
        );
    }

    #[test]
    fn cleanup_attempts_all_notes_and_retries_only_failed_offs() {
        let mut guide = GuideNotes::new();
        update(&mut guide, Some((1, &[60, 64, 67])));
        let mut attempted = Vec::new();
        let error = guide
            .cleanup(|message| {
                attempted.push(message);
                if message[1] != 64 {
                    Err("port failed".into())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert!(error.contains("Note Off 60"));
        assert!(error.contains("Note Off 67"));
        assert_eq!(attempted, vec![[0x80, 60, 0], [0x80, 64, 0], [0x80, 67, 0]]);
        assert_eq!(guide.lit_notes(), vec![60, 67]);
        attempted.clear();
        guide
            .cleanup(|message| {
                attempted.push(message);
                Ok(())
            })
            .unwrap();
        assert_eq!(attempted, vec![[0x80, 60, 0], [0x80, 67, 0]]);
        assert!(guide.lit_notes().is_empty());
        guide.cleanup(|_| panic!("already cleared")).unwrap();
    }

    #[test]
    fn cleanup_includes_an_ambiguous_failed_note_on() {
        let mut guide = GuideNotes::new();
        guide
            .update(Some((1, &[72])), |_| Err("delivery unknown".into()))
            .unwrap_err();
        let mut attempted = Vec::new();
        guide
            .cleanup(|message| {
                attempted.push(message);
                Ok(())
            })
            .unwrap();
        assert_eq!(attempted, vec![[0x80, 72, 0]]);
        assert!(guide.lit_notes().is_empty());
    }

    #[test]
    fn invalid_target_is_rejected_before_any_output_or_state_change() {
        let mut guide = GuideNotes::new();
        update(&mut guide, Some((1, &[60])));
        guide
            .update(Some((2, &[64, 128])), |_| panic!("invalid target sent"))
            .unwrap_err();
        assert_eq!(guide.lit_notes(), vec![60]);
        assert!(update(&mut guide, Some((1, &[60]))).is_empty());
    }
}
