// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! OxiGuitar's wah-wah plugin (plugin interface 1.16; mateusrdp/oxiguitar#250,
//! #293 and #346): everything about the wah but the mark's two signs on the
//! page, which are the score's own (Guitar Pro writes them too).
//!
//! - `editor` is what the user sees: the palette's Closed and Open
//!   Wah-Wah tools and the window they open, where a mark gets its curve
//!   and its Q.
//! - `form` is the window's state and rules; `shape` what a mark keeps,
//!   on its beat, as this plugin's own data.
//! - `curve` joins a track's marks, as the player plays them, into the
//!   pedal's curve through the song.
//! - `wah` is the pedal: Julius O. Smith's digital Cry Baby, our own code
//!   from its formulas, no latency.
//! - `settings` is a track's choice (on or off, the pedal's make), and
//!   `section` shows it in the Track tab.
//! - `ui/wah.slint` draws the window and the section.
//!
//! OxiGuitar knows none of this: it places the tools the manifest
//! declares, shows the plugin's views, carries what the plugin keeps on beats and
//! saves it beside the score, tells the effect its track's marked beats
//! with the times they play, and says where the score is on every call.

use std::sync::Arc;

use oxiguitar_plugin::audio::{PlayedBeat, PrepareEffect};
use oxiguitar_plugin::score2::Wah as Mark;
use oxiguitar_plugin::{
    Effect, Error, Host, Plugin, Processor, Tool, TrackView, Ui, Update, Value,
};

pub mod curve;
pub mod editor;
pub mod form;
pub mod section;
pub mod settings;
pub mod shape;
pub mod wah;

/// The plugin's id, under which it keeps its data on beats.
pub const ID: &str = "oxiguitar.wah";

/// The effect's id in the manifest.
pub const EFFECT: &str = "wah";

/// The Track tab's section's id in the manifest.
pub const SECTION: &str = "wah";

/// The plugin.
#[derive(Debug)]
pub struct WahPlugin {
    open: editor::MarkTool,
    closed: editor::MarkTool,
    editor: editor::Editor,
    section: section::Section,
}

impl Plugin for WahPlugin {
    fn start(host: Host) -> Result<WahPlugin, Error> {
        let shared = Arc::new(editor::Shared::default());
        let tool = |kind| editor::MarkTool {
            kind,
            host: host.clone(),
            shared: shared.clone(),
        };
        Ok(WahPlugin {
            open: tool(Mark::Open),
            closed: tool(Mark::Closed),
            editor: editor::Editor {
                host: host.clone(),
                shared,
            },
            section: section::Section::new(host.clone()),
        })
    }

    fn effect(&self, id: &str) -> Option<&dyn Effect> {
        (id == EFFECT).then_some(self as &dyn Effect)
    }

    fn tool(&self, id: &str) -> Option<&dyn Tool> {
        match id {
            "open" => Some(&self.open),
            "closed" => Some(&self.closed),
            _ => None,
        }
    }

    fn ui(&self) -> Option<&dyn Ui> {
        Some(self)
    }
}

/// The window and the section (`ui/wah.slint`).
impl Ui for WahPlugin {
    fn shown(&self, view: &str, track: Option<&TrackView>) -> Result<(), Error> {
        match view {
            SECTION => self.section.shown(track),
            editor::DIALOG => self
                .editor
                .host
                .set_view(view, editor::properties(&self.editor.shared)),
            _ => Ok(()),
        }
    }

    fn callback(&self, view: &str, callback: &str, args: Vec<Value>) -> Result<(), Error> {
        match (view, callback) {
            (SECTION, _) => self.section.callback(callback, &args),
            (editor::DIALOG, "ok" | "cancel") => {
                self.editor.finish(callback == "ok")?;
                self.editor.host.close_view(view)
            }
            (editor::DIALOG, _) => {
                if self.editor.on(callback, &args) {
                    self.editor
                        .host
                        .set_view(view, editor::properties(&self.editor.shared))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    /// The window closed by the user (its close button, Escape) forgets
    /// the mark, as Cancel does.
    fn closed(&self, view: &str, _accepted: bool) -> Result<(), Error> {
        match view {
            editor::DIALOG => self.editor.finish(false),
            _ => Ok(()),
        }
    }
}

impl Effect for WahPlugin {
    /// Without the score's marks there is nothing to play: the track
    /// passes untouched.
    fn prepare(&self, prepare: &PrepareEffect) -> Result<Box<dyn Processor>, Error> {
        self.prepare_following(prepare, &[])
    }

    fn prepare_following(
        &self,
        prepare: &PrepareEffect,
        beats: &[PlayedBeat],
    ) -> Result<Box<dyn Processor>, Error> {
        let track = settings::Track::from_text(&prepare.track.settings);
        Ok(Box::new(Wah {
            on: track.on,
            curve: curve::Curve::of(beats, ID),
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
    curve: curve::Curve,
    pedal: wah::Pedal,
}

impl Processor for Wah {
    /// Told nothing of where the score is: the pedal is off, and only
    /// goes out if it was in.
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.pedal.process(left, right, |_| None);
    }

    fn process_at(&mut self, left: &mut [f32], right: &mut [f32], time: f64, step: f64) {
        // The score isn't playing, the track's pedal is off, or it has no
        // mark: as above.
        if step == 0.0 || !self.on || self.curve.is_empty() {
            return self.process(left, right);
        }
        let curve = &self.curve;
        self.pedal
            .process(left, right, |frame| curve.at(time + frame as f64 * step));
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
    use oxiguitar_plugin::audio::PreparedTrack;
    use oxiguitar_plugin::score2::BeatExt;
    use oxiguitar_plugin::{InstrumentClass, TrackInfo};

    use super::*;

    const MANIFEST: &str = include_str!("../manifest.json");

    #[test]
    fn the_manifest_is_valid_for_the_plugin() {
        let problems = oxiguitar_plugin::check::<WahPlugin>(MANIFEST, env!("CARGO_PKG_VERSION"));
        assert!(problems.is_empty(), "{}", problems.join("; "));
    }

    fn plugin() -> WahPlugin {
        WahPlugin::start(Host::detached()).unwrap()
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

    /// One open mark, played from 0.25 s to 0.5 s.
    fn marks() -> Vec<PlayedBeat> {
        vec![PlayedBeat {
            start: 0.25,
            end: 0.5,
            staff: 0,
            bar: 0,
            voice: 0,
            beat: 0,
            ext: vec![BeatExt::Wah(Mark::Open)],
        }]
    }

    /// A second of a saw-like sound through the plugin, in blocks of 512,
    /// with the score playing or not.
    fn run(p: &mut Box<dyn Processor>, playing: bool) -> (Vec<f32>, Vec<f32>) {
        let sound: Vec<f32> = (0..48_000)
            .map(|i| ((i * 37) % 101) as f32 / 101.0 - 0.5)
            .collect();
        let (mut l, mut r) = (sound.clone(), sound);
        let step = 1.0 / 48_000.0;
        for (k, (a, b)) in l.chunks_mut(512).zip(r.chunks_mut(512)).enumerate() {
            let time = (k * 512) as f64 * step;
            p.process_at(a, b, time, if playing { step } else { 0.0 });
        }
        (l, r)
    }

    #[test]
    fn the_pedal_changes_the_track_only_while_a_mark_s_beat_plays() {
        let wah = plugin();
        let mut p = wah.prepare_following(&prepared("{}"), &marks()).unwrap();
        assert_eq!(p.latency(), 0);
        let (dry, _) = run(&mut p, false);
        let (l, r) = run(&mut p, true);
        assert_eq!(l, r);
        // Untouched before the mark; changed under it; untouched again
        // once the pedal has gone out.
        assert_eq!(l[..12_000], dry[..12_000]);
        assert_ne!(l[13_000..23_000], dry[13_000..23_000]);
        assert_eq!(l[25_000..], dry[25_000..]);
        // A track without marks, or prepared without the score, passes
        // untouched.
        let mut none = wah.prepare(&prepared("{}")).unwrap();
        assert_eq!(run(&mut none, true).0, dry);
    }

    #[test]
    fn a_track_whose_pedal_is_off_passes_untouched_and_follows_the_section_live() {
        let wah = plugin();
        let mut p = wah
            .prepare_following(&prepared(r#"{"on": false}"#), &marks())
            .unwrap();
        let (dry, _) = run(&mut p, false);
        assert_eq!(run(&mut p, true).0, dry);
        // Switched on while the song plays, with another make.
        let mut u = wah
            .update(&prepared(r#"{"on": true, "heel": 300}"#), "{}")
            .unwrap()
            .unwrap();
        p.apply(&mut u);
        let wet = run(&mut p, true).0;
        assert_ne!(wet, dry);
        let mut cry_baby = wah.prepare_following(&prepared("{}"), &marks()).unwrap();
        assert_ne!(run(&mut cry_baby, true).0, wet);
    }

    #[test]
    fn the_tools_and_the_views_are_the_manifest_s() {
        let wah = plugin();
        assert!(wah.tool("open").is_some() && wah.tool("closed").is_some());
        assert!(wah.tool("other").is_none());
        assert!(wah.ui().is_some());
        // With no score open a tool says so and opens nothing.
        assert_eq!(
            wah.tool("open").unwrap().run().unwrap(),
            "No score is open."
        );
    }

    fn component(name: &str) -> oxiguitar_plugin_test::UiComponent {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/wah.slint");
        let compiled = oxiguitar_plugin_test::compile_ui_file(&file).unwrap();
        compiled
            .components
            .into_iter()
            .find(|c| c.name == name)
            .unwrap()
    }

    /// The file compiles as OxiGuitar compiles it, with every property and
    /// callback the window and the section use.
    #[test]
    fn the_slint_file_has_what_the_views_set_and_hear() {
        let window = component("WahWindow");
        let shared = editor::Shared::default();
        for (name, _) in editor::properties(&shared) {
            assert!(window.properties.contains(&name), "{name}");
        }
        for name in [
            "kind-chosen",
            "q-chosen",
            "pressed",
            "dragged",
            "released",
            "ok",
            "cancel",
        ] {
            assert!(window.callbacks.iter().any(|c| c == name), "{name}");
        }
        let section = component("WahSection");
        for (name, _) in section::properties(&Default::default()) {
            assert!(section.properties.contains(&name), "{name}");
        }
        for name in ["switched", "heel-slid", "sweep-slid", "cry-baby"] {
            assert!(section.callbacks.iter().any(|c| c == name), "{name}");
        }
    }
}
