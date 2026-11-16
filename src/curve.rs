// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The pedal through the song: every wah-wah mark of the track as the
//! player plays it (plugin interface 1.8: an effect that follows the score
//! is prepared with its track's marked beats and their times), joined into
//! one curve the pedal reads on the audio thread. The pedal is off
//! wherever no mark's beat is playing.

use oxiguitar_plugin::audio::PlayedBeat;
use oxiguitar_plugin::score2::BeatExt;

use crate::shape::Shape;

/// A point of the curve: at `time` seconds of the score the pedal is at
/// `pedal` (0 to 1) with a mark's `q`; `on` false ends a run, and the pedal
/// is off from there to the next point.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    time: f64,
    pedal: f64,
    q: f64,
    on: bool,
}

/// The pedal's curve over the score's time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Curve {
    points: Vec<Point>,
}

impl Curve {
    /// The curve of a track's played beats: those with a wah-wah mark,
    /// each with the shape this plugin kept on it (`plugin` is its id), or
    /// its kind's own. Where two voices' marks play at once, the pedal
    /// follows the one that started first.
    pub fn of(beats: &[PlayedBeat], plugin: &str) -> Curve {
        let mut marks: Vec<(f64, f64, Shape)> = beats
            .iter()
            .filter(|b| b.end > b.start)
            .filter_map(|b| {
                let kind = b.ext.iter().find_map(|e| match e {
                    BeatExt::Wah(kind) => Some(*kind),
                    _ => None,
                })?;
                let kept = b.ext.iter().find_map(|e| match e {
                    BeatExt::Plugin { plugin: id, data } if id == plugin => Shape::from_text(data),
                    _ => None,
                });
                Some((b.start, b.end, kept.unwrap_or_else(|| Shape::of(kind))))
            })
            .collect();
        marks.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut points: Vec<Point> = Vec::new();
        let mut run_end: Option<f64> = None;
        for (start, end, shape) in marks {
            let (Some(&(_, first)), Some(&(_, last))) = (shape.points.first(), shape.points.last())
            else {
                continue;
            };
            match run_end {
                Some(e) if start < e - 1e-9 => continue,
                // A gap: the pedal is off between the two.
                Some(e) if start > e + 1e-9 => points.push(Point {
                    time: e,
                    pedal: 0.0,
                    q: shape.q,
                    on: false,
                }),
                _ => {}
            }
            let at = |time: f64, pedal: f64| Point {
                time,
                pedal: pedal / 100.0,
                q: shape.q,
                on: true,
            };
            points.push(at(start, first));
            points.extend(
                shape
                    .points
                    .iter()
                    .map(|&(place, pedal)| at(start + (end - start) * place, pedal)),
            );
            points.push(at(end, last));
            run_end = Some(end);
        }
        if let (Some(e), Some(last)) = (run_end, points.last().copied()) {
            points.push(Point {
                time: e,
                on: false,
                ..last
            });
        }
        points.dedup();
        Curve { points }
    }

    /// Whether the track has no wah-wah mark at all.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The pedal (0 to 1) and its mark's Q at `time`, or `None` where the
    /// pedal is off. Nothing is allocated: this runs on the audio thread.
    pub fn at(&self, time: f64) -> Option<(f64, f64)> {
        let next = self.points.partition_point(|p| p.time <= time);
        let a = self.points.get(next.checked_sub(1)?)?;
        if !a.on {
            return None;
        }
        match self.points.get(next) {
            Some(b) if b.on && b.time > a.time => {
                let f = (time - a.time) / (b.time - a.time);
                Some((a.pedal + (b.pedal - a.pedal) * f, a.q))
            }
            Some(_) => Some((a.pedal, a.q)),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use oxiguitar_plugin::score2::Wah;

    use super::*;

    fn beat(start: f64, end: f64, ext: Vec<BeatExt>) -> PlayedBeat {
        PlayedBeat {
            start,
            end,
            staff: 0,
            bar: 0,
            voice: 0,
            beat: 0,
            ext,
        }
    }

    #[test]
    fn marks_become_one_curve_and_the_pedal_is_off_between_them() {
        let ours = BeatExt::Plugin {
            plugin: "oxiguitar.wah".into(),
            data: r#"{"q": 6.5, "points": [[0.25, 30], [0.75, 80]]}"#.into(),
        };
        let theirs = BeatExt::Plugin {
            plugin: "someone.else".into(),
            data: r#"{"q": 1, "points": [[0, 100]]}"#.into(),
        };
        let curve = Curve::of(
            &[
                // An open mark, then a closed one at once: the pedal rocks
                // on. Other marks and other plugins' data are not ours.
                beat(0.0, 0.5, vec![BeatExt::Slur, BeatExt::Wah(Wah::Open)]),
                beat(0.5, 1.0, vec![BeatExt::Wah(Wah::Closed), theirs]),
                beat(1.0, 1.5, vec![BeatExt::Slur]),
                // A shape of the user's own, on the last beat.
                beat(1.5, 2.0, vec![BeatExt::Wah(Wah::Open), ours]),
            ],
            "oxiguitar.wah",
        );
        let pedal = |t: f64| curve.at(t).map(|(p, _)| (p * 1000.0).round() / 1000.0);
        assert_eq!(pedal(-0.1), None);
        assert_eq!(pedal(0.0), Some(0.0));
        assert_eq!(pedal(0.125), Some(0.5));
        assert_eq!(pedal(0.25), Some(1.0));
        assert_eq!(pedal(0.4), Some(1.0));
        assert_eq!(pedal(0.625), Some(0.5));
        assert_eq!(pedal(0.9), Some(0.0));
        // Off from the closed mark's end to the last beat.
        assert_eq!(pedal(1.0), None);
        assert_eq!(pedal(1.4), None);
        // The user's curve holds its ends to the beat's.
        assert_eq!(pedal(1.5), Some(0.3));
        assert_eq!(pedal(1.6), Some(0.3));
        assert_eq!(pedal(1.75), Some(0.55));
        assert_eq!(pedal(1.95), Some(0.8));
        assert_eq!(pedal(2.0), None);
        assert_eq!(curve.at(0.1).map(|(_, q)| q), Some(4.0));
        assert_eq!(curve.at(1.7).map(|(_, q)| q), Some(6.5));
    }

    #[test]
    fn without_marks_there_is_no_curve_and_overlapping_marks_follow_the_first() {
        assert!(Curve::of(&[beat(0.0, 1.0, vec![BeatExt::Slur])], "oxiguitar.wah").is_empty());
        let two = Curve::of(
            &[
                beat(0.0, 1.0, vec![BeatExt::Wah(Wah::Open)]),
                beat(0.5, 1.0, vec![BeatExt::Wah(Wah::Closed)]),
            ],
            "oxiguitar.wah",
        );
        assert_eq!(two.at(0.75).map(|(p, _)| p), Some(1.0));
        assert_eq!(two.at(1.0), None);
    }
}
