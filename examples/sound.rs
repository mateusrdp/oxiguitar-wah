// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Writes the README's example sound: `cargo run --example sound -- FILE.wav`.
//!
//! Two bars of eighth-note chords on a plucked string of our own (a
//! Karplus-Strong string, so the sound needs no instrument plugin): the
//! first bar dry, the second with wah-wah marks, open and closed in turn,
//! through the pedal as OxiGuitar would run it. Mono, 16 bits, 44.1 kHz.

use oxiguitar_plugin::audio::PlayedBeat;
use oxiguitar_plugin::score2::{BeatExt, Wah};
use oxiguitar_wah_plugin::curve::Curve;
use oxiguitar_wah_plugin::wah::{Pedal, Voice};

const RATE: f64 = 44_100.0;
/// Seconds an eighth note lasts at 96 quarter notes a minute.
const EIGHTH: f64 = 60.0 / 96.0 / 2.0;

/// One plucked string at `frequency`, `seconds` long.
fn pluck(frequency: f64, seconds: f64, seed: &mut u32) -> Vec<f32> {
    let period = (RATE / frequency).round() as usize;
    let mut line: Vec<f32> = (0..period)
        .map(|_| {
            // A small generator of our own: the pluck is noise.
            *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (*seed >> 8) as f32 / (1 << 23) as f32 - 1.0
        })
        .collect();
    let mut out = Vec::new();
    for i in 0..(seconds * RATE) as usize {
        let at = i % period;
        let next = (at + 1) % period;
        out.push(line[at]);
        line[at] = 0.498 * (line[at] + line[next]);
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: sound FILE.wav")?;
    // An E9 chord's top four strings, strummed sixteen times.
    let chord = [146.83, 207.65, 261.63, 329.63];
    let mut seed = 7;
    let mut sound = vec![0.0f32; (EIGHTH * 16.0 * RATE) as usize + RATE as usize];
    let mut beats = Vec::new();
    for strum in 0..16 {
        let start = strum as f64 * EIGHTH;
        for (string, frequency) in chord.iter().enumerate() {
            // The strings a few milliseconds apart, as a pick crosses them.
            let from = ((start + string as f64 * 0.006) * RATE) as usize;
            for (i, s) in pluck(*frequency, EIGHTH * 1.6, &mut seed)
                .iter()
                .enumerate()
            {
                // Let go of at the next strum.
                let fade = (1.0 - i as f64 / (EIGHTH * 1.6 * RATE)).min(1.0) as f32;
                if let Some(out) = sound.get_mut(from + i) {
                    *out += 0.2 * s * fade;
                }
            }
        }
        // The second bar's strums carry marks: open, closed, open...
        let ext = match strum {
            0..8 => Vec::new(),
            s if s % 2 == 0 => vec![BeatExt::Wah(Wah::Open)],
            _ => vec![BeatExt::Wah(Wah::Closed)],
        };
        beats.push(PlayedBeat {
            start,
            end: start + EIGHTH,
            staff: 0,
            bar: strum / 8,
            voice: 0,
            beat: strum % 8,
            ext,
        });
    }
    let curve = Curve::of(&beats, oxiguitar_wah_plugin::ID);
    let mut pedal = Pedal::new(Voice::CRY_BABY, RATE);
    let mut right = sound.clone();
    for (block, (l, r)) in sound.chunks_mut(256).zip(right.chunks_mut(256)).enumerate() {
        let time = (block * 256) as f64 / RATE;
        pedal.process(l, r, |frame| curve.at(time + frame as f64 / RATE));
    }

    let data: Vec<u8> = sound
        .iter()
        .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32_767.0) as i16).to_le_bytes())
        .collect();
    let mut file = Vec::new();
    file.extend(b"RIFF");
    file.extend((36 + data.len() as u32).to_le_bytes());
    file.extend(b"WAVEfmt ");
    file.extend(16u32.to_le_bytes());
    file.extend(1u16.to_le_bytes()); // PCM
    file.extend(1u16.to_le_bytes()); // mono
    file.extend((RATE as u32).to_le_bytes());
    file.extend((RATE as u32 * 2).to_le_bytes());
    file.extend(2u16.to_le_bytes());
    file.extend(16u16.to_le_bytes());
    file.extend(b"data");
    file.extend((data.len() as u32).to_le_bytes());
    file.extend(data);
    std::fs::write(path, file)?;
    Ok(())
}
