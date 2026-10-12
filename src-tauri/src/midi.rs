use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::SyncSender,
    Arc,
};

#[derive(Clone, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Copy)]
pub struct Message {
    pub status: u8,
    pub a: u8,
    pub b: u8,
}
pub fn devices() -> Result<Vec<Device>, String> {
    let input = MidiInput::new("Keyloom device list").map_err(|e| e.to_string())?;
    let names: Vec<String> = input
        .ports()
        .iter()
        .map(|p| {
            input
                .port_name(p)
                .unwrap_or_else(|_| "Unnamed MIDI input".into())
        })
        .collect();
    Ok(names
        .iter()
        .enumerate()
        .map(|(i, name)| Device {
            id: if names.iter().filter(|n| *n == name).count() == 1 {
                name.clone()
            } else {
                format!("{name} [port {i}; ambiguous]")
            },
            name: name.clone(),
        })
        .collect())
}
pub fn connect(
    id: &str,
    tx: SyncSender<Message>,
    dropped: Arc<AtomicU64>,
) -> Result<MidiInputConnection<()>, String> {
    let listing = devices()?;
    let index = listing
        .iter()
        .position(|d| d.id == id)
        .ok_or("Selected input is unavailable; Refresh Devices and select it again")?;
    let mut input = MidiInput::new("Keyloom player input").map_err(|e| e.to_string())?;
    input.ignore(Ignore::None);
    let ports = input.ports();
    let port = ports
        .get(index)
        .ok_or("MIDI port changed during selection; refresh and retry")?;
    if input.port_name(port).map_err(|e| e.to_string())? != listing[index].name {
        return Err("MIDI ports changed during selection; refresh and retry".into());
    }
    input
        .connect(
            port,
            "Keyloom input",
            move |_, bytes, _| {
                if bytes.len() < 3 {
                    return;
                }
                let status = bytes[0];
                if !matches!(status & 0xf0, 0x80 | 0x90 | 0xb0) {
                    return;
                }
                if tx
                    .try_send(Message {
                        status,
                        a: bytes[1] & 0x7f,
                        b: bytes[2] & 0x7f,
                    })
                    .is_err()
                {
                    dropped.fetch_add(1, Ordering::Relaxed);
                }
            },
            (),
        )
        .map_err(|e| e.to_string())
}

/// Output is a separate WinMM connection, with no internal route to the input
/// callback/matcher. External MIDI echo/Thru must be disabled: the MIDI wire
/// carries no provenance that distinguishes returned output from real keys.
/// Imported MIDI is never forwarded.
pub fn output_devices() -> Result<Vec<Device>, String> {
    let output = MidiOutput::new("Keyloom guide device list").map_err(|e| e.to_string())?;
    let names: Vec<String> = output
        .ports()
        .iter()
        .map(|p| {
            output
                .port_name(p)
                .unwrap_or_else(|_| "Unnamed MIDI output".into())
        })
        .collect();
    Ok(names
        .iter()
        .enumerate()
        .map(|(i, name)| Device {
            id: if names.iter().filter(|n| *n == name).count() == 1 {
                name.clone()
            } else {
                format!("{name} [port {i}; ambiguous]")
            },
            name: name.clone(),
        })
        .collect())
}

pub fn connect_output(id: &str) -> Result<MidiOutputConnection, String> {
    let listing = output_devices()?;
    let index = listing
        .iter()
        .position(|d| d.id == id)
        .ok_or("Selected output is unavailable; refresh devices and select it again")?;
    let output = MidiOutput::new("Keyloom light guide").map_err(|e| e.to_string())?;
    let ports = output.ports();
    let port = ports
        .get(index)
        .ok_or("MIDI output changed; refresh and retry")?;
    if output.port_name(port).map_err(|e| e.to_string())? != listing[index].name {
        return Err("MIDI output changed during selection; refresh and retry".into());
    }
    output
        .connect(port, "Keyloom channel 1 light guide")
        .map_err(|e| e.to_string())
}
