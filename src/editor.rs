// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The palette's two tools and the wah-wah editor they open: the window
//! `WahWindow` in `ui/wah.slint` (plugin interface 1.16).
//!
//! A tool looks at the beats the editor's selection names (the caret's
//! beat, the selected beats, or every beat of the selected bars), opens
//! the editor on the first one's mark, or on the mark it was asked for
//! when that beat has none, and leaves the rest to the window. OK writes
//! the mark and its shape on every one of those beats that has notes, as
//! one change the user can undo.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use std::fmt::Write as _;

use oxiguitar_plugin::score::BeatKind;
use oxiguitar_plugin::score2::{Beat2, BeatExt, Score2, Wah};
use oxiguitar_plugin::{Error, ErrorKind, Host, Selection2, Tool, Value};

use crate::form::{COLUMNS, Form, KINDS, ROWS, q_names};
use crate::shape::Shape;

/// The window's id in the manifest.
pub const DIALOG: &str = "editor";

/// The drawing: the grid with room for its side's words, in pixels.
const WIDTH: f32 = 404.0;
const HEIGHT: f32 = 264.0;
/// The grid's top-left corner and its cells.
const LEFT: f32 = 38.0;
const TOP: f32 = 12.0;
const CELL_W: f32 = 30.0;
const CELL_H: f32 = 24.0;

/// A beat's place in the score: track, staff, bar, voice, beat.
type Place = [usize; 5];

/// What the tools and the editor share.
#[derive(Debug, Default)]
pub struct Shared {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    /// The beats the open editor is for, and its state.
    editing: Option<(Vec<Place>, Form)>,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The beats a selection names, in the score as it is.
fn places(score: &Score2, selection: &Selection2) -> Vec<Place> {
    let index = |i: u32| i as usize;
    let mut out = Vec::new();
    match *selection {
        Selection2::Caret {
            track,
            staff,
            voice,
            bar,
            beat,
            ..
        } => out.push([
            index(track),
            index(staff),
            index(bar),
            index(voice),
            index(beat),
        ]),
        Selection2::Beats {
            track,
            staff,
            voice,
            from,
            to,
        } => {
            let (t, s, v) = (index(track), index(staff), index(voice));
            for bar in index(from.bar)..=index(to.bar) {
                let beats = beat_at(score, [t, s, bar, v, 0])
                    .map(|_| score.tracks[t].staves[s].bars[bar].voices[v].beats.len())
                    .unwrap_or(0);
                let first = if bar == index(from.bar) {
                    index(from.beat)
                } else {
                    0
                };
                let last = if bar == index(to.bar) {
                    index(to.beat)
                } else {
                    beats.saturating_sub(1)
                };
                out.extend(
                    (first..=last)
                        .filter(|b| *b < beats)
                        .map(|b| [t, s, bar, v, b]),
                );
            }
        }
        Selection2::Bars {
            first_track,
            last_track,
            first_bar,
            last_bar,
        } => {
            for t in index(first_track)..=index(last_track) {
                let Some(track) = score.tracks.get(t) else {
                    continue;
                };
                for (s, staff) in track.staves.iter().enumerate() {
                    for bar in index(first_bar)..=index(last_bar) {
                        let Some(b) = staff.bars.get(bar) else {
                            continue;
                        };
                        for (v, voice) in b.voices.iter().enumerate() {
                            out.extend((0..voice.beats.len()).map(|i| [t, s, bar, v, i]));
                        }
                    }
                }
            }
        }
    }
    out.retain(|p| beat_at(score, *p).is_some_and(takes_a_mark));
    out
}

fn beat_at(score: &Score2, [t, s, bar, v, b]: Place) -> Option<&Beat2> {
    score
        .tracks
        .get(t)?
        .staves
        .get(s)?
        .bars
        .get(bar)?
        .voices
        .get(v)?
        .beats
        .get(b)
}

fn beat_at_mut(score: &mut Score2, [t, s, bar, v, b]: Place) -> Option<&mut Beat2> {
    score
        .tracks
        .get_mut(t)?
        .staves
        .get_mut(s)?
        .bars
        .get_mut(bar)?
        .voices
        .get_mut(v)?
        .beats
        .get_mut(b)
}

/// A wah-wah mark goes on a beat that sounds: one with notes, not a rest
/// nor a grace beat.
fn takes_a_mark(beat: &Beat2) -> bool {
    beat.kind == BeatKind::Notes && beat.grace.is_none()
}

/// A beat's mark and the shape this plugin kept on it.
pub fn mark_of(beat: &Beat2) -> Option<(Wah, Option<Shape>)> {
    let kind = beat.ext.iter().find_map(|e| match e {
        BeatExt::Wah(kind) => Some(*kind),
        _ => None,
    })?;
    let shape = beat.ext.iter().find_map(|e| match e {
        BeatExt::Plugin { plugin, data } if plugin == crate::ID => Shape::from_text(data),
        _ => None,
    });
    Some((kind, shape))
}

/// Gives a beat a mark and its shape, or none; whatever else the beat has
/// stays.
pub fn set_mark(beat: &mut Beat2, mark: Option<&(Wah, Option<Shape>)>) {
    beat.ext.retain(|e| {
        !matches!(e, BeatExt::Wah(_))
            && !matches!(e, BeatExt::Plugin { plugin, .. } if plugin == crate::ID)
    });
    if let Some((kind, shape)) = mark {
        beat.ext.push(BeatExt::Wah(*kind));
        if let Some(shape) = shape {
            beat.ext.push(BeatExt::Plugin {
                plugin: crate::ID.into(),
                data: shape.to_text(),
            });
        }
    }
}

/// A palette tool: Closed Wah-Wah or Open Wah-Wah.
#[derive(Debug)]
pub struct MarkTool {
    /// The mark it offers on a beat that has none.
    pub kind: Wah,
    /// The host.
    pub host: Host,
    /// What it shares with the editor.
    pub shared: Arc<Shared>,
}

impl Tool for MarkTool {
    fn run(&self) -> Result<String, Error> {
        let Some((_, score)) = self.host.score2()? else {
            return Ok("No score is open.".into());
        };
        // Asked for as the tool runs: where the caret is now.
        let selection = self.host.selection()?;
        let places = selection
            .as_ref()
            .map(|s| places(&score, s))
            .unwrap_or_default();
        let Some(first) = places.first().and_then(|p| beat_at(&score, *p)) else {
            return Ok(
                "Put the caret on a note first: a wah-wah mark goes on a beat with notes.".into(),
            );
        };
        let form = Form::open(mark_of(first), self.kind);
        self.shared.state().editing = Some((places, form));
        let shown = self
            .host
            .set_view(DIALOG, properties(&self.shared))
            .and_then(|()| self.host.show_view(DIALOG));
        if let Err(e) = shown {
            self.shared.state().editing = None;
            return Err(e);
        }
        Ok(String::new())
    }
}

/// The wah-wah editor: the window.
#[derive(Debug)]
pub struct Editor {
    /// The host.
    pub host: Host,
    /// What it shares with the tools.
    pub shared: Arc<Shared>,
}

fn x(col: f32) -> f32 {
    LEFT + col * CELL_W
}

fn y(row: f32) -> f32 {
    TOP + (ROWS as f32 - row) * CELL_H
}

fn spot(x: f32, y: f32) -> Value {
    Value::Struct(vec![
        ("x".into(), Value::Number(f64::from(x))),
        ("y".into(), Value::Number(f64::from(y))),
    ])
}

/// The grid's lines as SVG path commands: the thin ones, the thick ones
/// (every third column, the heel, the middle and the toe).
fn grid() -> (String, String) {
    let (mut thin, mut thick) = (String::new(), String::new());
    let (right, bottom) = (x(COLUMNS as f32), y(0.0));
    for col in 0..=COLUMNS {
        let to = if col % 3 == 0 { &mut thick } else { &mut thin };
        let at = x(col as f32);
        let _ = write!(to, "M {at} {TOP} L {at} {bottom} ");
    }
    for row in 0..=ROWS {
        let to = if row % 5 == 0 { &mut thick } else { &mut thin };
        let at = y(row as f32);
        let _ = write!(to, "M {LEFT} {at} L {right} {at} ");
    }
    (thin.trim_end().into(), thick.trim_end().into())
}

/// What the window shows: the form being edited, or nothing to edit.
pub fn properties(shared: &Shared) -> Vec<(String, Value)> {
    let state = shared.state();
    let (count, form) = match &state.editing {
        Some((places, form)) => (places.len(), form.clone()),
        None => (0, Form::open(None, Wah::Open)),
    };
    let strings = |v: Vec<String>| Value::List(v.into_iter().map(Value::String).collect());
    let at = |p: &crate::form::Point| (x(p.col as f32), y(p.row as f32));
    let mut curve = String::new();
    for (i, p) in form.points.iter().enumerate() {
        let (px, py) = at(p);
        let _ = write!(curve, "{} {px} {py} ", if i == 0 { "M" } else { "L" });
    }
    let (thin, thick) = grid();
    vec![
        (
            "kinds".into(),
            strings(KINDS.iter().map(|k| (*k).to_owned()).collect()),
        ),
        ("kind".into(), Value::Number(form.kind as f64)),
        ("qs".into(), strings(q_names())),
        ("q".into(), Value::Number(form.q as f64)),
        ("can-pick-q".into(), Value::Bool(form.kind != 0)),
        ("drawing-width".into(), Value::Number(f64::from(WIDTH))),
        ("drawing-height".into(), Value::Number(f64::from(HEIGHT))),
        ("thin".into(), Value::String(thin)),
        ("thick".into(), Value::String(thick)),
        ("curve".into(), Value::String(curve.trim_end().into())),
        (
            "points".into(),
            Value::List(
                form.points
                    .iter()
                    .map(|p| {
                        let (px, py) = at(p);
                        spot(px, py)
                    })
                    .collect(),
            ),
        ),
        ("toe".into(), spot(LEFT - 6.0, y(ROWS as f32))),
        ("heel".into(), spot(LEFT - 6.0, y(0.0))),
        (
            "beats".into(),
            Value::String(if count > 1 {
                format!("For the {count} selected beats.")
            } else {
                String::new()
            }),
        ),
    ]
}

fn number(args: &[Value], i: usize) -> Option<f32> {
    match args.get(i) {
        Some(Value::Number(n)) if n.is_finite() => Some(*n as f32),
        _ => None,
    }
}

impl Editor {
    /// A callback of the window but OK and Cancel: the form follows the
    /// choices and the pointer. `false` when nothing changed that shows.
    pub fn on(&self, callback: &str, args: &[Value]) -> bool {
        let mut state = self.shared.state();
        let Some((_, form)) = &mut state.editing else {
            return false;
        };
        let index = number(args, 0).filter(|i| *i >= 0.0).map(|i| i as usize);
        let grid = || {
            let (px, py) = (number(args, 0)?, number(args, 1)?);
            Some(((px - LEFT) / CELL_W, ROWS as f32 - (py - TOP) / CELL_H))
        };
        let before = form.clone();
        match callback {
            "kind-chosen" => index.into_iter().for_each(|i| form.pick_kind(i)),
            "q-chosen" => index.into_iter().for_each(|i| form.pick_q(i)),
            "pressed" => {
                if let Some((col, row)) = grid() {
                    form.press(col, row);
                }
                return false;
            }
            "dragged" => {
                if let Some((col, row)) = grid() {
                    form.drag(col, row);
                }
            }
            "released" => {
                form.release();
                return false;
            }
            _ => return false,
        }
        *form != before
    }

    /// OK (`ok`) writes the mark on the beats the window is for, as one
    /// change; Cancel, or the window closed, forgets it.
    pub fn finish(&self, ok: bool) -> Result<(), Error> {
        let Some((places, form)) = self.shared.state().editing.take() else {
            return Ok(());
        };
        if !ok || !form.changed {
            return Ok(());
        }
        let Some((revision, mut score)) = self.host.score2()? else {
            return Ok(());
        };
        let mark = form.mark();
        let mut any = false;
        for place in places {
            if let Some(beat) = beat_at_mut(&mut score, place).filter(|b| takes_a_mark(b)) {
                set_mark(beat, mark.as_ref());
                any = true;
            }
        }
        if !any {
            return Err(Error::new(
                ErrorKind::Failed,
                "the beats the editor was opened for are no longer there",
            ));
        }
        self.host.apply(revision, "Wah-Wah", score)
    }
}

#[cfg(test)]
mod tests {
    use oxiguitar_plugin::Position;
    use oxiguitar_plugin::score2::{Bar2, Staff2, Track2, Voice2};

    use super::*;

    /// A quarter-note beat of this kind, with nothing on it.
    fn beat(kind: BeatKind) -> Beat2 {
        Beat2 {
            kind,
            duration: oxiguitar_plugin::score::Duration {
                value: oxiguitar_plugin::score::NoteValue::Quarter,
                dots: 0,
                tuplets: Vec::new(),
            },
            notes: Vec::new(),
            grace: None,
            effects: oxiguitar_plugin::score::BeatEffects::default(),
            chord: None,
            text: String::new(),
            ext: Vec::new(),
        }
    }

    /// One track, one staff, two bars of one voice: three beats with
    /// notes (the second a grace beat) and a rest, then two with notes.
    fn score() -> Score2 {
        let mut grace = beat(BeatKind::Notes);
        grace.grace = Some(oxiguitar_plugin::score::GraceKind::BeforeBeat);
        let bar = |beats: Vec<Beat2>| Bar2 {
            clef: oxiguitar_plugin::score::Clef::Treble,
            voices: vec![Voice2 {
                beats,
                ..Voice2::default()
            }],
            ext: Vec::new(),
        };
        Score2 {
            tracks: vec![Track2 {
                staves: vec![Staff2 {
                    bars: vec![
                        bar(vec![
                            beat(BeatKind::Notes),
                            grace,
                            beat(BeatKind::Notes),
                            beat(BeatKind::Rest),
                        ]),
                        bar(vec![beat(BeatKind::Notes), beat(BeatKind::Notes)]),
                    ],
                    ..Staff2::default()
                }],
                ..Track2::default()
            }],
            ..Score2::default()
        }
    }

    #[test]
    fn a_selection_names_the_beats_that_take_a_mark() {
        let score = score();
        let caret = |bar, beat| Selection2::Caret {
            track: 0,
            staff: 0,
            voice: 0,
            bar,
            beat,
            string: 1,
        };
        assert_eq!(places(&score, &caret(0, 2)), [[0, 0, 0, 0, 2]]);
        // A rest, a grace beat and a beat that isn't there take none.
        assert!(places(&score, &caret(0, 3)).is_empty());
        assert!(places(&score, &caret(0, 1)).is_empty());
        assert!(places(&score, &caret(7, 0)).is_empty());
        let run = Selection2::Beats {
            track: 0,
            staff: 0,
            voice: 0,
            from: Position { bar: 0, beat: 2 },
            to: Position { bar: 1, beat: 0 },
        };
        assert_eq!(places(&score, &run), [[0, 0, 0, 0, 2], [0, 0, 1, 0, 0]]);
        let bars = Selection2::Bars {
            first_track: 0,
            last_track: 3,
            first_bar: 0,
            last_bar: 1,
        };
        assert_eq!(places(&score, &bars).len(), 4);
    }

    #[test]
    fn a_mark_and_its_shape_are_written_on_the_beat_and_read_back() {
        let mut beat = beat(BeatKind::Notes);
        beat.ext = vec![
            BeatExt::Slur,
            BeatExt::Plugin {
                plugin: "someone.else".into(),
                data: "{}".into(),
            },
        ];
        assert_eq!(mark_of(&beat), None);
        let shape = Shape {
            points: vec![(0.0, 10.0), (1.0, 90.0)],
            q: 6.0,
        };
        set_mark(&mut beat, Some(&(Wah::Closed, Some(shape.clone()))));
        assert_eq!(mark_of(&beat), Some((Wah::Closed, Some(shape))));
        // Another mark replaces it; the plain mark keeps no shape.
        set_mark(&mut beat, Some(&(Wah::Open, None)));
        assert_eq!(mark_of(&beat), Some((Wah::Open, None)));
        assert_eq!(beat.ext.len(), 3);
        // No mark: what isn't ours stays.
        set_mark(&mut beat, None);
        assert_eq!(mark_of(&beat), None);
        assert_eq!(beat.ext.len(), 2);
        assert!(matches!(beat.ext[0], BeatExt::Slur));
    }

    fn get<'a>(p: &'a [(String, Value)], name: &str) -> &'a Value {
        &p.iter().find(|(n, _)| n == name).unwrap().1
    }

    #[test]
    fn the_editor_shows_the_form_and_follows_its_choices_and_the_pointer() {
        let shared = Arc::new(Shared::default());
        let editor = Editor {
            host: Host::detached(),
            shared: shared.clone(),
        };
        // Not opened by a tool: nothing changes, and it can be shown.
        assert!(!editor.on("q-chosen", &[Value::Number(5.0)]));
        assert_eq!(
            get(&properties(&shared), "beats"),
            &Value::String(String::new())
        );
        shared.state().editing = Some((
            vec![[0, 0, 0, 0, 0], [0, 0, 0, 0, 2]],
            Form::open(None, Wah::Open),
        ));
        let p = properties(&shared);
        assert_eq!(get(&p, "kind"), &Value::Number(1.0));
        assert_eq!(get(&p, "can-pick-q"), &Value::Bool(true));
        assert_eq!(
            get(&p, "beats"),
            &Value::String("For the 2 selected beats.".into())
        );
        // The curve from the heel to the toe, and its four points.
        assert_eq!(
            get(&p, "curve"),
            &Value::String("M 38 252 L 218 12 L 218 12 L 398 12".into())
        );
        assert!(matches!(get(&p, "points"), Value::List(l) if l.len() == 4));
        assert!(matches!(get(&p, "thick"), Value::String(t) if t.starts_with("M 38 12 L 38 252")));
        // The end of the curve dragged from the toe to half way.
        let at = |col: f32, row: f32| {
            [
                Value::Number(f64::from(x(col))),
                Value::Number(f64::from(y(row))),
            ]
        };
        assert!(!editor.on("pressed", &at(12.0, 10.0)));
        assert!(editor.on("dragged", &at(12.0, 5.0)));
        assert!(!editor.on("dragged", &at(12.0, 5.1)));
        assert!(!editor.on("released", &[]));
        assert!(editor.on("q-chosen", &[Value::Number(5.0)]));
        let made = shared.state().editing.as_ref().unwrap().1.mark().unwrap();
        let shape = made.1.unwrap();
        assert_eq!(shape.q, 6.0);
        assert_eq!(shape.points.last(), Some(&(1.0, 50.0)));
        // No wah-wah: no curve, and no Q to pick.
        assert!(editor.on("kind-chosen", &[Value::Number(0.0)]));
        let p = properties(&shared);
        assert_eq!(get(&p, "curve"), &Value::String(String::new()));
        assert_eq!(get(&p, "can-pick-q"), &Value::Bool(false));
        // Cancel forgets it and changes nothing.
        editor.finish(false).unwrap();
        assert!(shared.state().editing.is_none());
    }
}
