# Yamaha verification

## Owner-reported results — 11 October 2026

The owner reports that **Wait for me works well on the Yamaha** and that **reconnecting works well**. The owner does not have a pedal, so physical pedal testing is not applicable to this setup. These are owner-reported playing results, separate from the automated evidence below; the individual edge cases have not all been confirmed.

## Remaining detailed checks

Native port enumeration, two open/close/reopen cycles of USB MIDI Interface, and WASAPI callback operation passed in the installed and portable builds. Simulated keys and callback samples are not physical-keyboard or audible-latency evidence.

- Connect the Yamaha MIDI OUT to the adapter MIDI IN. In Keyloom select the observed USB interface explicitly; confirm the live monitor labels events **physical**, with expected channel, note, velocity and note-off. Test velocity-zero note-on release. Physical pedal CC64 is not applicable unless a pedal is added later.
- Calibrate the real lowest/highest keys. Confirm out-of-range targets are reported without transposition. Confirm C4 note 60/note-number labels agree with the keyboard convention you prefer.
- Use an imported arrangement. Start with Yamaha-speaker monitoring, then optionally PC basic-tone monitoring with keyboard local sound disabled manually to avoid doubling. Confirm sound, releases, chords and Panic by listening.
- Wait mode: single notes, repeated notes (release/re-press), two-note chord within 150 ms, wrong extra notes, held chord transition. Confirm accompaniment and falling notes freeze together. Try left/right part practice and the explicitly approximate split for the combined track.
- Timed/Listen: speed 25/75/150%, seek across sustained notes, pause/resume, count-in/metronome, A–B loop, and stop. Confirm no stuck notes or pitch changes.
- Unplug during practice. Within the polling interval the session should pause and report loss. Refresh, reconnect the same interface, and resume. Try restart with the interface absent and with duplicate port names if available.
- Resize, fullscreen, and the owner's 4K display/scaling. Check readability and actual playing responsiveness while the machine has its usual workload. No overlay/driver settings were changed; report any reproducible overlay conflict rather than attributing immunity to the new stack.
- Measure true key-to-sound latency externally if needed. Internal timestamps/RAF cadence exclude USB, driver and speaker latency. Listen for dropouts during a longer session.

Mark detailed checks passed/failed with the keyboard model, interface model, key range and date. The owner-reported results above do not imply that every edge case has passed.
