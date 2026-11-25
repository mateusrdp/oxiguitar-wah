// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Track tab's section "Wah-wah", for every class of instrument
//! (`WahSection` in `ui/wah.slint`, plugin interface 1.16): whether the
//! track has the pedal, where its sweep starts and how far it goes, and
//! Cry Baby, which sets the last two back. What the pedal does at each
//! mark (its curve and Q) is set on the mark, in the score.

use std::sync::{Mutex, PoisonError};

use oxiguitar_plugin::serde_json::{Map, Value as Json};
use oxiguitar_plugin::{Error, Host, TrackView, Value};

use crate::SECTION;
use crate::settings::{self, HEEL, RANGE, Track};

/// A frequency as shown: "450 Hz", "2.2 kHz".
fn hertz(f: f64) -> String {
    if f >= 1000.0 {
        format!("{:.1} kHz", f / 1000.0)
    } else {
        format!("{f:.0} Hz")
    }
}

/// What the section shows for a track's settings.
pub fn properties(settings: &Map<String, Json>) -> Vec<(String, Value)> {
    let t = Track::from_map(settings);
    let d = Track::default();
    let number = |n: f32| Value::Number(f64::from(n));
    vec![
        ("on".into(), Value::Bool(t.on)),
        ("heel".into(), number(t.heel)),
        ("heel-minimum".into(), number(HEEL.0)),
        ("heel-maximum".into(), number(HEEL.1)),
        ("heel-text".into(), Value::String(hertz(f64::from(t.heel)))),
        ("sweep".into(), number(t.range)),
        ("sweep-minimum".into(), number(RANGE.0)),
        ("sweep-maximum".into(), number(RANGE.1)),
        (
            "sweep-text".into(),
            Value::String(format!("{:.1} octaves, to {}", t.range, hertz(t.toe()))),
        ),
        (
            "can-reset".into(),
            Value::Bool(t.on && (t.heel != d.heel || t.range != d.range)),
        ),
    ]
}

/// What a callback changes in a track's settings; `false` when nothing.
pub fn change(settings: &mut Map<String, Json>, callback: &str, args: &[Value]) -> bool {
    let mut t = Track::from_map(settings);
    let number = || match args.first() {
        Some(Value::Number(n)) if n.is_finite() => Some(*n),
        _ => None,
    };
    match (callback, args.first()) {
        ("switched", Some(Value::Bool(on))) => t.on = *on,
        ("heel-slid", _) => match number() {
            Some(v) => t.heel = settings::heel(v),
            None => return false,
        },
        ("sweep-slid", _) => match number() {
            Some(v) => t.range = settings::range(v),
            None => return false,
        },
        ("cry-baby", _) => {
            let d = Track::default();
            (t.heel, t.range) = (d.heel, d.range);
        }
        _ => return false,
    }
    t.write(settings);
    true
}

/// The section: the settings of the track it shows.
#[derive(Debug)]
pub struct Section {
    host: Host,
    settings: Mutex<Map<String, Json>>,
}

impl Section {
    /// The section, showing no track yet.
    pub fn new(host: Host) -> Section {
        Section {
            host,
            settings: Mutex::new(Map::new()),
        }
    }

    /// Shown for a track: its settings, as they are.
    pub fn shown(&self, track: Option<&TrackView>) -> Result<(), Error> {
        let settings: Map<String, Json> = track
            .and_then(|t| oxiguitar_plugin::serde_json::from_str(&t.settings).ok())
            .unwrap_or_default();
        let properties = properties(&settings);
        *self.settings.lock().unwrap_or_else(PoisonError::into_inner) = settings;
        self.host.set_view(SECTION, properties)
    }

    /// The user changed something: the track's settings change with it.
    pub fn callback(&self, callback: &str, args: &[Value]) -> Result<(), Error> {
        let settings = {
            let mut settings = self.settings.lock().unwrap_or_else(PoisonError::into_inner);
            if !change(&mut settings, callback, args) {
                return Ok(());
            }
            settings.clone()
        };
        self.host.set_track_settings(SECTION, &settings)?;
        self.host.set_view(SECTION, properties(&settings))
    }
}

#[cfg(test)]
mod tests {
    use oxiguitar_plugin::serde_json::json;

    use super::*;

    fn get<'a>(p: &'a [(String, Value)], name: &str) -> &'a Value {
        &p.iter().find(|(n, _)| n == name).unwrap().1
    }

    fn text(p: &[(String, Value)], name: &str) -> String {
        match get(p, name) {
            Value::String(s) => s.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn it_shows_a_cry_baby_that_is_on_and_what_can_be_shown() {
        let p = properties(&Map::new());
        assert_eq!(get(&p, "on"), &Value::Bool(true));
        assert_eq!(text(&p, "heel-text"), "450 Hz");
        assert_eq!(text(&p, "sweep-text"), "2.3 octaves, to 2.2 kHz");
        // Nothing to set back yet.
        assert_eq!(get(&p, "can-reset"), &Value::Bool(false));
    }

    #[test]
    fn its_widgets_set_the_pedal_and_cry_baby_sets_it_back() {
        let mut settings = Map::new();
        settings.insert("later".into(), json!(true));
        let on = |s: &mut Map<String, Json>, id: &str, args: &[Value]| change(s, id, args);
        assert!(on(&mut settings, "heel-slid", &[Value::Number(613.0)]));
        assert!(on(&mut settings, "sweep-slid", &[Value::Number(9.0)]));
        let t = Track::from_map(&settings);
        assert_eq!((t.on, t.heel, t.range), (true, 610.0, 3.5));
        let p = properties(&settings);
        assert_eq!(text(&p, "sweep-text"), "3.5 octaves, to 6.9 kHz");
        assert_eq!(get(&p, "can-reset"), &Value::Bool(true));
        assert!(on(&mut settings, "cry-baby", &[]));
        assert_eq!(Track::from_map(&settings), Track::default());
        // Off: the sliders keep their values, and Cry Baby can't be used.
        on(&mut settings, "heel-slid", &[Value::Number(300.0)]);
        assert!(on(&mut settings, "switched", &[Value::Bool(false)]));
        let p = properties(&settings);
        assert_eq!(get(&p, "heel"), &Value::Number(300.0));
        assert_eq!(get(&p, "can-reset"), &Value::Bool(false));
        // A callback it doesn't know, or one without its value, changes
        // nothing; later keys stay.
        let before = settings.clone();
        assert!(!on(&mut settings, "other", &[]));
        assert!(!on(&mut settings, "switched", &[]));
        assert!(!on(&mut settings, "heel-slid", &[Value::Number(f64::NAN)]));
        assert_eq!(settings, before);
        assert_eq!(settings["later"], json!(true));
    }
}
