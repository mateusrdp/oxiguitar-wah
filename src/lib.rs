// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! OxiGuitar's wah-wah plugin (plugin interface 1.8; mateusrdp/oxiguitar#250,
//! its docs/design/wah.md): the effect `wah`, a wah pedal on any track,
//! rocked by the score's wah marks.
//!
//! OxiGuitar hands an effect its track's control curves with every call
//! (interface 1.8). The wah pedal's are [`control::WAH`], the pedal's
//! position from heel (0) to toe (1) wherever a wah mark's beat is
//! playing, and [`control::WAH_Q`], that mark's Q. Where the pedal has no
//! value it is off and the track passes untouched.
//!
//! - `wah` is the pedal: Julius O. Smith's digital Cry Baby, our own code
//!   from its formulas, no latency.
//! - `settings` is a track's choice: on or off, and the pedal's make
//!   (where its sweep starts, how far it goes).
//! - `section` shows it in the track dialog, as the section "Wah-wah".
//!
//! A change of the section while the song plays reaches the running
//! pedal (interface 1.5).

use oxiguitar_plugin::audio::{Controls, PrepareEffect, control};
use oxiguitar_plugin::{Effect, Error, Host, Plugin, Processor, TrackSection, Update};

pub mod section;
pub mod settings;
pub mod wah;

/// The effect's id in the manifest.
pub const EFFECT: &str = "wah";

/// The track dialog's section's id in the manifest.
pub const SECTION: &str = "wah";

/// The plugin.
#[derive(Debug)]
pub struct WahPlugin;

impl Plugin for WahPlugin {
    fn start(_host: Host) -> Result<WahPlugin, Error> {
        Ok(WahPlugin)
    }

    fn effect(&self, id: &str) -> Option<&dyn Effect> {
        (id == EFFECT).then_some(self as &dyn Effect)
    }

    fn track_section(&self, id: &str) -> Option<&dyn TrackSection> {
        (id == SECTION).then_some(&section::Section as &dyn TrackSection)
    }
}

impl Effect for WahPlugin {
    fn prepare(&self, prepare: &PrepareEffect) -> Result<Box<dyn Processor>, Error> {
        let track = settings::Track::from_text(&prepare.track.settings);
        Ok(Box::new(Wah {
            on: track.on,
            pedal: wah::Pedal::new(track.voice(), f64::from(prepare.rate)),
        }))
    }

    /// The track's new settings while the song plays: the pedal on or
    /// off, and its make, which it glides to.
    fn update(&self, prepare: &PrepareEffect, _previous: &str) -> Result<Option<Update>, Error> {
        Ok(Some(Update::new(settings::Track::from_text(
            &prepare.track.settings,
        ))))
    }
}

/// A track through its pedal.
struct Wah {
    on: bool,
    pedal: wah::Pedal,
}

impl Processor for Wah {
    /// Without the score (nothing plays): the pedal is off, and only goes
    /// out if it was in.
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.pedal.process(left, right, |_| None);
    }

    fn process_with(&mut self, left: &mut [f32], right: &mut [f32], controls: &Controls<'_>) {
        let on = self.on;
        self.pedal.process(left, right, |frame| {
            let pedal = controls.value_at(control::WAH, frame).filter(|_| on)?;
            let q = controls.value_at(control::WAH_Q, frame).unwrap_or(wah::Q);
            Some((pedal, q))
        });
    }

    fn apply(&mut self, update: &mut Update) {
        if let Some(track) = update.get::<settings::Track>() {
            self.on = track.on;
            self.pedal.set_voice(track.voice());
        }
    }
}

oxiguitar_plugin::export_plugin!(WahPlugin);

#[cfg(test)]
mod tests {
    use oxiguitar_plugin::audio::{ControlPoint, PreparedTrack};
    use oxiguitar_plugin::{InstrumentClass, TrackInfo};

    use super::*;

    const MANIFEST: &str = include_str!("../manifest.json");

    #[test]
    fn the_manifest_is_valid_for_the_plugin() {
        let problems = oxiguitar_plugin::check::<WahPlugin>(MANIFEST, env!("CARGO_PKG_VERSION"));
        assert!(problems.is_empty(), "{}", problems.join("; "));
    }

    fn prepared(settings: &str) -> PrepareEffect {
        PrepareEffect {
            effect: EFFECT.into(),
            rate: 48_000,
            frames: 512,
            track: PreparedTrack {
                info: TrackInfo {
                    index: 0,
                    name: "Lead".into(),
                    class: InstrumentClass::Fretted,
                },
                program: 29,
                percussion: false,
                tuning: vec![64, 59, 55, 50, 45, 40],
                capo: 0,
                settings: settings.into(),
            },
        }
    }

    /// A second of the pedal going from heel to toe from 0.25 s to 0.5 s,
    /// then off.
    fn marks() -> Vec<ControlPoint> {
        let point = |control, time, value| ControlPoint {
            time,
            value,
            control,
        };
        vec![
            point(control::WAH, 0.25, 0.0),
            point(control::WAH, 0.5, 1.0),
            point(control::WAH, 0.5, f32::NAN),
            point(control::WAH_Q, 0.25, 4.0),
            point(control::WAH_Q, 0.5, f32::NAN),
        ]
    }

    /// A second of a saw-like sound through the plugin, in blocks of 512.
    fn run(p: &mut Box<dyn Processor>, points: &[ControlPoint]) -> (Vec<f32>, Vec<f32>) {
        let sound: Vec<f32> = (0..48_000)
            .map(|i| ((i * 37) % 101) as f32 / 101.0 - 0.5)
            .collect();
        let (mut l, mut r) = (sound.clone(), sound);
        let step = 1.0 / 48_000.0;
        for (k, (a, b)) in l.chunks_mut(512).zip(r.chunks_mut(512)).enumerate() {
            let controls = Controls::new(points, (k * 512) as f64 * step, step);
            p.process_with(a, b, &controls);
        }
        (l, r)
    }

    #[test]
    fn the_pedal_changes_the_track_only_where_the_score_has_it_on() {
        let mut p = WahPlugin.prepare(&prepared("{}")).unwrap();
        assert_eq!(p.latency(), 0);
        let (dry, _) = run(&mut p, &[]);
        let (l, r) = run(&mut p, &marks());
        assert_eq!(l, r);
        // Untouched before the mark; changed under it; untouched again
        // once it has gone out.
        assert_eq!(l[..12_000], dry[..12_000]);
        assert_ne!(l[13_000..23_000], dry[13_000..23_000]);
        assert_eq!(l[25_000..], dry[25_000..]);
    }

    #[test]
    fn a_track_whose_pedal_is_off_passes_untouched_and_follows_the_section_live() {
        let mut p = WahPlugin.prepare(&prepared(r#"{"on": false}"#)).unwrap();
        let (dry, _) = run(&mut p, &[]);
        assert_eq!(run(&mut p, &marks()).0, dry);
        // Switched on while the song plays, with another make.
        let mut u = WahPlugin
            .update(&prepared(r#"{"on": true, "heel": 300}"#), "{}")
            .unwrap()
            .unwrap();
        p.apply(&mut u);
        let wet = run(&mut p, &marks()).0;
        assert_ne!(wet, dry);
        let mut cry_baby = WahPlugin.prepare(&prepared("{}")).unwrap();
        assert_ne!(run(&mut cry_baby, &marks()).0, wet);
    }
}
