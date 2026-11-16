// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! How a wah-wah mark is played: where the pedal is across the mark's
//! beat, and how sharp the filter is. The score has the mark itself (open
//! or closed, as Guitar Pro writes it); the shape is this plugin's own, kept
//! on the beat (plugin interface 1.8: OxiGuitar carries it with the beat
//! and saves it in the song's `.oxg` file), as
//!
//! ```json
//! {"q": 6, "points": [[0, 0], [0.5, 100], [1, 50]]}
//! ```
//!
//! each point a place in the beat (0 its start, 1 its end) and the pedal
//! there, from 0 (heel down, closed) to 100 (toe down, open). A mark with
//! no shape plays its kind's own ([`Shape::of`]).

use oxiguitar_plugin::score2::Wah;
use oxiguitar_plugin::serde_json::{self, Value, json};

/// The Q of a mark that has none of its own: a Cry Baby's with its pedal
/// half way.
pub const Q: f64 = 4.0;

/// The least and the most Q a mark can have.
pub const Q_RANGE: (f64, f64) = (1.0, 16.0);

/// A mark's shape.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// The pedal's curve in time order: (place in the beat, 0 to 1; the
    /// pedal, 0 to 100). Straight lines between the points; the first and
    /// last hold to the beat's ends.
    pub points: Vec<(f64, f64)>,
    /// The filter's Q with the pedal half way.
    pub q: f64,
}

impl Shape {
    /// A kind's own shape: the pedal rocks to its place over the beat's
    /// first half and stays, from the heel for an open mark ("wah") and
    /// from the toe for a closed one, so marks that alternate rock the
    /// pedal without a jump.
    pub fn of(kind: Wah) -> Shape {
        let (from, to) = match kind {
            Wah::Open => (0.0, 100.0),
            Wah::Closed => (100.0, 0.0),
        };
        Shape {
            points: vec![(0.0, from), (0.5, to), (1.0, to)],
            q: Q,
        }
    }

    /// The shape kept on a beat; `None` for what isn't one.
    pub fn from_text(text: &str) -> Option<Shape> {
        let value: Value = serde_json::from_str(text).ok()?;
        let mut points: Vec<(f64, f64)> = value
            .get("points")?
            .as_array()?
            .iter()
            .filter_map(|p| {
                let p = p.as_array()?;
                let (at, pedal) = (p.first()?.as_f64()?, p.get(1)?.as_f64()?);
                (at.is_finite() && pedal.is_finite() && (0.0..=1.0).contains(&at))
                    .then(|| (at, pedal.clamp(0.0, 100.0)))
            })
            .collect();
        if points.is_empty() {
            return None;
        }
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        let q = value.get("q").and_then(Value::as_f64).unwrap_or(Q);
        Some(Shape {
            points,
            q: if q.is_finite() {
                q.clamp(Q_RANGE.0, Q_RANGE.1)
            } else {
                Q
            },
        })
    }

    /// The shape as kept on a beat.
    pub fn to_text(&self) -> String {
        let points: Vec<Value> = self.points.iter().map(|&(at, p)| json!([at, p])).collect();
        json!({ "q": self.q, "points": points }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_s_own_shape_rocks_over_the_first_half_and_holds() {
        assert_eq!(
            Shape::of(Wah::Open).points,
            [(0.0, 0.0), (0.5, 100.0), (1.0, 100.0)]
        );
        assert_eq!(
            Shape::of(Wah::Closed).points,
            [(0.0, 100.0), (0.5, 0.0), (1.0, 0.0)]
        );
        assert_eq!(Shape::of(Wah::Open).q, 4.0);
    }

    #[test]
    fn a_kept_shape_is_read_back_in_order_and_in_range() {
        let shape = Shape {
            points: vec![(0.0, 20.0), (0.25, 100.0), (1.0, 40.0)],
            q: 6.5,
        };
        assert_eq!(Shape::from_text(&shape.to_text()), Some(shape));
        let odd =
            r#"{"q": 99, "points": [[1, 250], [0, -5], [2, 50], ["x", 1], [0.5, 50]], "later": 1}"#;
        assert_eq!(
            Shape::from_text(odd),
            Some(Shape {
                points: vec![(0.0, 0.0), (0.5, 50.0), (1.0, 100.0)],
                q: 16.0
            })
        );
        // No Q: the Cry Baby's. No points, or not a shape: none.
        assert_eq!(Shape::from_text(r#"{"points": [[0, 1]]}"#).unwrap().q, Q);
        for none in [
            "",
            "{}",
            "[1]",
            r#"{"points": []}"#,
            r#"{"points": [[3, 1]]}"#,
        ] {
            assert_eq!(Shape::from_text(none), None, "{none}");
        }
    }
}
