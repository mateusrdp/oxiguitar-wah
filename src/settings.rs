// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A track's pedal, kept per track beside the score (`FILE.oxg`, the track
//! dialog's section, under `oxiguitar.wah`):
//!
//! ```json
//! {"on": true, "heel": 450, "range": 2.3}
//! ```
//!
//! `on` (true when missing: a score's wah marks sound as soon as the
//! plugin is installed), `heel` the resonance with the heel down in Hz,
//! `range` how many octaves the toe takes it up. Missing or unreadable,
//! the last two are a Cry Baby's. What each wah mark does (its curve and
//! its Q) is the score's, not a setting.

use oxiguitar_plugin::serde_json::{Map, Value, json};

use crate::wah::Voice;

/// The lowest and highest heel frequency, in Hz.
pub const HEEL: (f32, f32) = (200.0, 800.0);

/// The shortest and longest sweep, in octaves.
pub const RANGE: (f32, f32) = (1.0, 3.5);

/// One track's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    /// Whether the track has the pedal.
    pub on: bool,
    /// The resonance with the heel down, in Hz, on the ten.
    pub heel: f32,
    /// The sweep in octaves, on the tenth.
    pub range: f32,
}

impl Default for Track {
    fn default() -> Track {
        Track {
            on: true,
            heel: Voice::CRY_BABY.heel as f32,
            range: Voice::CRY_BABY.range as f32,
        }
    }
}

/// A heel frequency as kept: on the ten, within [`HEEL`].
pub fn heel(value: f64) -> f32 {
    if !value.is_finite() {
        return Track::default().heel;
    }
    (((value / 10.0).round() * 10.0) as f32).clamp(HEEL.0, HEEL.1)
}

/// A sweep as kept: on the tenth of an octave, within [`RANGE`].
pub fn range(value: f64) -> f32 {
    if !value.is_finite() {
        return Track::default().range;
    }
    (((value * 10.0).round() / 10.0) as f32).clamp(RANGE.0, RANGE.1)
}

impl Track {
    /// Reads a track's settings; anything missing or of the wrong type
    /// keeps its default.
    pub fn from_map(map: &Map<String, Value>) -> Track {
        let d = Track::default();
        Track {
            on: map.get("on").and_then(Value::as_bool).unwrap_or(d.on),
            heel: map.get("heel").and_then(Value::as_f64).map_or(d.heel, heel),
            range: map
                .get("range")
                .and_then(Value::as_f64)
                .map_or(d.range, range),
        }
    }

    /// Reads them from the text the player prepares with.
    pub fn from_text(text: &str) -> Track {
        match oxiguitar_plugin::serde_json::from_str::<Value>(text) {
            Ok(Value::Object(map)) => Track::from_map(&map),
            _ => Track::default(),
        }
    }

    /// Writes them into `map`, keeping keys a later version added.
    pub fn write(&self, map: &mut Map<String, Value>) {
        map.insert("on".into(), json!(self.on));
        map.insert("heel".into(), json!(self.heel));
        // As typed (2.3), not as the nearest f32 (2.299999952316284).
        map.insert(
            "range".into(),
            json!((f64::from(self.range) * 10.0).round() / 10.0),
        );
    }

    /// The pedal's make.
    pub fn voice(&self) -> Voice {
        Voice {
            heel: f64::from(self.heel),
            range: f64::from(self.range),
        }
    }

    /// The resonance with the toe down, in Hz.
    pub fn toe(&self) -> f64 {
        f64::from(self.heel) * f64::from(self.range).exp2()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_unreadable_settings_are_a_cry_baby_that_is_on() {
        for text in ["", "{}", "not json", "[1]", r#"{"on": 3, "heel": "x"}"#] {
            assert_eq!(Track::from_text(text), Track::default(), "{text}");
        }
        let d = Track::default();
        assert!(
            d.on && d.voice()
                == Voice {
                    heel: 450.0,
                    range: f64::from(2.3f32)
                }
        );
        assert!((d.toe() - 2216.0).abs() < 2.0, "{}", d.toe());
    }

    #[test]
    fn values_are_kept_in_step_and_in_range_and_written_as_typed() {
        let t = Track::from_text(r#"{"on": false, "heel": 1234.5, "range": 1.76, "later": 1}"#);
        assert_eq!(
            t,
            Track {
                on: false,
                heel: 800.0,
                range: 1.8
            }
        );
        assert_eq!(heel(444.0), 440.0);
        assert_eq!(range(0.2), 1.0);
        let mut map = Map::new();
        map.insert("later".into(), json!(1));
        t.write(&mut map);
        assert_eq!(
            Value::Object(map).to_string(),
            r#"{"heel":800.0,"later":1,"on":false,"range":1.8}"#
        );
    }
}
