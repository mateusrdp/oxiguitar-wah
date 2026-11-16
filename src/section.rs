// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The track dialog's section "Wah-wah", for every class of instrument:
//! whether the track has the pedal, where its sweep starts and how far it
//! goes, and Cry Baby, which sets the last two back. What the pedal does
//! at each mark (its curve and Q) is set on the mark, in the score.

use oxiguitar_plugin::serde_json::{Map, Value};
use oxiguitar_plugin::{Error, Row, TabContent, TabEvent, TrackInfo, TrackSection, Widget};

use crate::settings::{self, HEEL, RANGE, Track};

/// The id of the Cry Baby button.
pub const CRY_BABY: &str = "cry-baby";

/// The section.
#[derive(Debug, Default)]
pub struct Section;

fn label(text: impl Into<String>, muted: bool) -> Widget {
    Widget::Label {
        text: text.into(),
        muted,
    }
}

/// A frequency as shown: "450 Hz", "2.2 kHz".
fn hertz(f: f64) -> String {
    if f >= 1000.0 {
        format!("{:.1} kHz", f / 1000.0)
    } else {
        format!("{f:.0} Hz")
    }
}

impl TrackSection for Section {
    fn content(&self, _track: &TrackInfo, settings: &Map<String, Value>) -> TabContent {
        let t = Track::from_map(settings);
        let d = Track::default();
        let rows = vec![
            Row {
                widgets: vec![Widget::Toggle {
                    id: "on".into(),
                    text: "Play the score's wah-wah marks".into(),
                    on: t.on,
                    enabled: true,
                }],
            },
            Row {
                widgets: vec![
                    label("Heel", false),
                    Widget::Slider {
                        id: "heel".into(),
                        value: t.heel,
                        min: HEEL.0,
                        max: HEEL.1,
                        enabled: t.on,
                    },
                    label(hertz(f64::from(t.heel)), true),
                ],
            },
            Row {
                widgets: vec![
                    label("Sweep", false),
                    Widget::Slider {
                        id: "range".into(),
                        value: t.range,
                        min: RANGE.0,
                        max: RANGE.1,
                        enabled: t.on,
                    },
                    label(
                        format!("{:.1} octaves, to {}", t.range, hertz(t.toe())),
                        true,
                    ),
                ],
            },
            Row {
                widgets: vec![Widget::Button {
                    id: CRY_BABY.into(),
                    text: "Cry Baby".into(),
                    enabled: t.on && (t.heel != d.heel || t.range != d.range),
                }],
            },
        ];
        TabContent { rows }
    }

    fn on(
        &self,
        _track: &TrackInfo,
        settings: &mut Map<String, Value>,
        widget: &str,
        event: TabEvent,
    ) -> Result<(), Error> {
        let mut t = Track::from_map(settings);
        match (widget, event) {
            ("on", TabEvent::Toggled(on)) => t.on = on,
            ("heel", TabEvent::Slid(v)) => t.heel = settings::heel(f64::from(v)),
            ("range", TabEvent::Slid(v)) => t.range = settings::range(f64::from(v)),
            (CRY_BABY, TabEvent::Clicked) => {
                let d = Track::default();
                (t.heel, t.range) = (d.heel, d.range);
            }
            _ => return Ok(()),
        }
        t.write(settings);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use oxiguitar_plugin::InstrumentClass;
    use oxiguitar_plugin::serde_json::json;

    use super::*;

    fn track() -> TrackInfo {
        TrackInfo {
            index: 0,
            name: "Lead".into(),
            class: InstrumentClass::Fretted,
        }
    }

    fn texts(content: &TabContent) -> Vec<String> {
        content
            .rows
            .iter()
            .flat_map(|r| &r.widgets)
            .filter_map(|w| match w {
                Widget::Label { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn it_shows_a_cry_baby_that_is_on_and_what_can_be_shown() {
        let content = Section.content(&track(), &Map::new());
        assert_eq!(content.problems(), Vec::<String>::new());
        assert_eq!(
            texts(&content),
            ["Heel", "450 Hz", "Sweep", "2.3 octaves, to 2.2 kHz"]
        );
        assert!(matches!(
            &content.rows[0].widgets[0],
            Widget::Toggle { on: true, .. }
        ));
        // Nothing to set back yet.
        assert!(matches!(
            content.rows[3].widgets[0],
            Widget::Button { enabled: false, .. }
        ));
    }

    #[test]
    fn its_widgets_set_the_pedal_and_cry_baby_sets_it_back() {
        let mut settings = Map::new();
        settings.insert("later".into(), json!(true));
        let on = |s: &mut Map<String, Value>, id: &str, e: TabEvent| {
            Section.on(&track(), s, id, e).unwrap();
        };
        on(&mut settings, "heel", TabEvent::Slid(613.0));
        on(&mut settings, "range", TabEvent::Slid(9.0));
        let t = Track::from_map(&settings);
        assert_eq!((t.on, t.heel, t.range), (true, 610.0, 3.5));
        let content = Section.content(&track(), &settings);
        assert_eq!(content.problems(), Vec::<String>::new());
        assert_eq!(texts(&content)[3], "3.5 octaves, to 6.9 kHz");
        assert!(matches!(
            content.rows[3].widgets[0],
            Widget::Button { enabled: true, .. }
        ));
        on(&mut settings, CRY_BABY, TabEvent::Clicked);
        assert_eq!(Track::from_map(&settings), Track::default());
        // Off: the sliders are greyed out and keep their values.
        on(&mut settings, "heel", TabEvent::Slid(300.0));
        on(&mut settings, "on", TabEvent::Toggled(false));
        let content = Section.content(&track(), &settings);
        assert!(matches!(
            content.rows[1].widgets[1],
            Widget::Slider { enabled: false, value, .. } if value == 300.0
        ));
        // A widget it doesn't know changes nothing; later keys stay.
        let before = settings.clone();
        on(&mut settings, "other", TabEvent::Clicked);
        on(&mut settings, "on", TabEvent::Clicked);
        assert_eq!(settings, before);
        assert_eq!(settings["later"], json!(true));
    }
}
