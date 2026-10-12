//! Original generated exercises and edge cases. No downloaded musical material.

/// A short two-part original exercise: melody, repeated notes, a chord and bass.
pub fn demo_midi() -> Vec<u8> {
    let conductor = track(vec![
        (0, vec![0xff, 0x51, 3, 0x07, 0xa1, 0x20]),
        (0, vec![0xff, 0x58, 4, 4, 2, 24, 8]),
        (3840, vec![0xff, 0x2f, 0]),
    ]);
    let mut melody = vec![(0, text(3, b"Original melody"))];
    for pitch in [60, 62, 64, 67, 67, 65] {
        melody.push((0, vec![0x90, pitch, 88]));
        melody.push((400, vec![0x80, pitch, 0]));
        melody.push((80, vec![0xff, 1, 0]));
    }
    melody.extend([
        (0, vec![0x90, 60, 88]),
        (0, vec![0x90, 64, 88]),
        (880, vec![0x80, 60, 0]),
        (0, vec![0x80, 64, 0]),
        (80, vec![0xff, 0x2f, 0]),
    ]);
    let mut bass = vec![(0, text(3, b"Original accompaniment"))];
    for pitch in [48, 53, 55, 48] {
        bass.push((0, vec![0x91, pitch, 64]));
        bass.push((880, vec![0x81, pitch, 0]));
        bass.push((80, vec![0xff, 1, 0]));
    }
    bass.push((0, vec![0xff, 0x2f, 0]));
    smf(1, 480, &[conductor, track(melody), track(bass)])
}

/// Distinct source bytes, useful for a bounded generated library benchmark.
#[cfg(test)]
#[allow(dead_code)] // Shared fixture module is also compiled by the binary unit harness.
pub fn single_note(serial: u32) -> Vec<u8> {
    smf(
        0,
        480,
        &[track(vec![
            (0, text(1, format!("Original fixture {serial}").as_bytes())),
            (0, vec![0x90, 60 + (serial % 12) as u8, 90]),
            (480, vec![0x80, 60 + (serial % 12) as u8, 0]),
            (0, vec![0xff, 0x2f, 0]),
        ])],
    )
}

pub fn tempo_sustain() -> Vec<u8> {
    smf(
        0,
        480,
        &[track(vec![
            (0, vec![0xff, 0x51, 3, 0x07, 0xa1, 0x20]),
            (0, vec![0x90, 60, 90]),
            (120, vec![0xb0, 64, 127]),
            (120, vec![0x90, 60, 0]),
            (240, vec![0xff, 0x51, 3, 0x0f, 0x42, 0x40]),
            (0, vec![0x90, 64, 80]),
            (480, vec![0x80, 64, 0]),
            (0, vec![0xb0, 64, 0]),
            (0, vec![0xff, 0x2f, 0]),
        ])],
    )
}

#[cfg(test)]
#[allow(dead_code)]
pub fn overlap_and_missing_release() -> Vec<u8> {
    smf(
        0,
        480,
        &[track(vec![
            (0, vec![0x90, 60, 90]),
            (120, vec![0x90, 60, 80]),
            (120, vec![0x80, 60, 0]),
            (120, vec![0x80, 60, 0]),
            (120, vec![0x90, 67, 70]),
            (0, vec![0xff, 0x2f, 0]),
        ])],
    )
}

pub fn smf(format: u16, division: u16, tracks: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"MThd\0\0\0\x06".to_vec();
    bytes.extend_from_slice(&format.to_be_bytes());
    bytes.extend_from_slice(&(tracks.len() as u16).to_be_bytes());
    bytes.extend_from_slice(&division.to_be_bytes());
    for track in tracks {
        bytes.extend_from_slice(b"MTrk");
        bytes.extend_from_slice(&(track.len() as u32).to_be_bytes());
        bytes.extend_from_slice(track);
    }
    bytes
}

pub fn track(events: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (delta, event) in events {
        variable(delta, &mut bytes);
        bytes.extend(event);
    }
    bytes
}

fn text(kind: u8, value: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xff, kind];
    variable(value.len() as u32, &mut bytes);
    bytes.extend_from_slice(value);
    bytes
}

fn variable(mut value: u32, output: &mut Vec<u8>) {
    let mut bytes = [0u8; 4];
    let mut index = 3;
    bytes[index] = (value & 0x7f) as u8;
    value >>= 7;
    while value > 0 && index > 0 {
        index -= 1;
        bytes[index] = (value as u8 & 0x7f) | 0x80;
        value >>= 7;
    }
    output.extend_from_slice(&bytes[index..]);
}
