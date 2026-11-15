// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The wah-wah editor's state and rules, apart from what draws it so they
//! are tested alone: the mark's kind, its Q, and the pedal's curve on a
//! grid of the beat across (twelve columns) and the pedal up the side (ten
//! rows from heel to toe), as OxiGuitar's bend editor has a bend's.
//!
//! The curve has the bend's four places: where the pedal starts, the
//! middle's two places (one position, held from the first to the second),
//! and where it ends.

use oxiguitar_plugin::score2::Wah;

use crate::shape::{Q, Shape};

/// The grid's columns: twelfths of the beat.
pub const COLUMNS: i32 = 12;

/// The grid's rows above the heel: tenths of the pedal's way.
pub const ROWS: i32 = 10;

/// The kinds offered, "No wah-wah" first.
pub const KINDS: [&str; 3] = ["No wah-wah", "Open wah-wah", "Closed wah-wah"];

/// The Qs offered.
pub const QS: [f64; 9] = [1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0];

/// The Qs' names, the Cry Baby's own marked.
pub fn q_names() -> Vec<String> {
    QS.iter()
        .map(|&q| {
            if q == Q {
                format!("Q {q} (Cry Baby)")
            } else {
                format!("Q {q}")
            }
        })
        .collect()
}

/// A point of the curve on the grid: a column (0 to 12) and a row (0, the
/// heel, to 10, the toe).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    /// The column.
    pub col: i32,
    /// The row.
    pub row: i32,
}

/// The editor's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Form {
    /// The mark: an index into [`KINDS`].
    pub kind: usize,
    /// The Q picked: an index into [`QS`].
    pub q: usize,
    /// Start, the middle's two places, end; empty for no mark.
    pub points: Vec<Point>,
    /// The point being dragged.
    dragging: Option<usize>,
    /// Something was changed, so OK applies it.
    pub changed: bool,
}

fn kind_of(index: usize) -> Option<Wah> {
    match index {
        1 => Some(Wah::Open),
        2 => Some(Wah::Closed),
        _ => None,
    }
}

impl Form {
    /// The editor opened on a beat's mark (`here`, with its shape where it
    /// has one of its own). On a beat without a mark it opens on the kind
    /// `asked` for, ready to be applied.
    pub fn open(here: Option<(Wah, Option<Shape>)>, asked: Wah) -> Form {
        let (wah, shape, changed) = match here {
            Some((wah, shape)) => (wah, shape.unwrap_or_else(|| Shape::of(wah)), false),
            None => (asked, Shape::of(asked), true),
        };
        Form {
            kind: match wah {
                Wah::Open => 1,
                Wah::Closed => 2,
            },
            q: nearest_q(shape.q),
            points: grid_points(&shape.points),
            dragging: None,
            changed,
        }
    }

    /// A kind picked: its own curve, or none for "No wah-wah". The Q
    /// stays.
    pub fn pick_kind(&mut self, kind: usize) {
        if kind >= KINDS.len() {
            return;
        }
        self.kind = kind;
        self.points = kind_of(kind)
            .map(|w| grid_points(&Shape::of(w).points))
            .unwrap_or_default();
        self.changed = true;
    }

    /// A Q picked.
    pub fn pick_q(&mut self, q: usize) {
        if q < QS.len() && q != self.q {
            self.q = q;
            self.changed = true;
        }
    }

    /// A press on the grid at a column and row (fractions allowed): the
    /// nearest point is picked up, if any is within a cell and a half.
    pub fn press(&mut self, col: f32, row: f32) {
        let near = self
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let d = (p.col as f32 - col).powi(2) + (p.row as f32 - row).powi(2);
                (i, d)
            })
            // The later of two points in one place, so a held middle's
            // second place comes off its first.
            .min_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
            .filter(|(_, d)| *d <= 2.25);
        self.dragging = near.map(|(i, _)| i);
    }

    /// The pointer moved while a point is held: the point moves there,
    /// kept between its neighbours in time; the middle's two places share
    /// one position.
    pub fn drag(&mut self, col: f32, row: f32) {
        let Some(i) = self.dragging else {
            return;
        };
        let low = i
            .checked_sub(1)
            .and_then(|j| self.points.get(j))
            .map_or(0, |p| p.col);
        let high = self.points.get(i + 1).map_or(COLUMNS, |p| p.col);
        let to = Point {
            col: (col.round() as i32).clamp(low, high),
            row: (row.round() as i32).clamp(0, ROWS),
        };
        let Some(p) = self.points.get_mut(i) else {
            return;
        };
        if *p == to {
            return;
        }
        *p = to;
        if (i == 1 || i == 2)
            && let Some(q) = self.points.get_mut(3 - i)
        {
            q.row = to.row;
        }
        self.changed = true;
    }

    /// The pointer let go.
    pub fn release(&mut self) {
        self.dragging = None;
    }

    /// The mark the editor makes: its kind, and its shape when that isn't
    /// the kind's own; `None` for no mark.
    pub fn mark(&self) -> Option<(Wah, Option<Shape>)> {
        let wah = kind_of(self.kind)?;
        let mut points: Vec<(f64, f64)> = Vec::new();
        for p in &self.points {
            let at = (
                f64::from(p.col) / f64::from(COLUMNS),
                f64::from(p.row.clamp(0, ROWS) * 100 / ROWS),
            );
            if points.last() != Some(&at) {
                points.push(at);
            }
        }
        let shape = Shape {
            points,
            q: QS.get(self.q).copied().unwrap_or(Q),
        };
        Some((wah, (shape != Shape::of(wah)).then_some(shape)))
    }
}

/// The Q offered that is nearest to `q`.
fn nearest_q(q: f64) -> usize {
    QS.iter()
        .enumerate()
        .min_by(|a, b| (a.1 - q).abs().total_cmp(&(b.1 - q).abs()))
        .map_or(0, |(i, _)| i)
}

/// A curve on the grid, in the editor's four places: one that doesn't
/// have that shape keeps its first point, the one furthest from it and
/// its last.
fn grid_points(points: &[(f64, f64)]) -> Vec<Point> {
    let at = |&(place, pedal): &(f64, f64)| Point {
        col: (place * f64::from(COLUMNS)).round() as i32,
        row: (pedal.clamp(0.0, 100.0) * f64::from(ROWS) / 100.0).round() as i32,
    };
    let four: Vec<(f64, f64)> = match points {
        [] => return Vec::new(),
        [a] => vec![*a; 4],
        [a, b] => vec![*a, *b, *b, *b],
        [a, b, c] => vec![*a, *b, *b, *c],
        [a, b, c, d] if b.1 == c.1 => vec![*a, *b, *c, *d],
        [first, middle @ .., last] => {
            let far = middle
                .iter()
                .max_by(|a, b| (a.1 - first.1).abs().total_cmp(&(b.1 - first.1).abs()))
                .copied()
                .unwrap_or(*first);
            vec![*first, far, far, *last]
        }
    };
    let mut out: Vec<Point> = four.iter().map(at).collect();
    // Snapped columns stay in order.
    for i in 1..out.len() {
        let before = out.get(i - 1).map_or(0, |p| p.col);
        if let Some(p) = out.get_mut(i) {
            p.col = p.col.clamp(before, COLUMNS);
        }
    }
    if let [_, m1, m2, _] = out.as_mut_slice() {
        m2.row = m1.row;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(form: &Form) -> Vec<(i32, i32)> {
        form.points.iter().map(|p| (p.col, p.row)).collect()
    }

    #[test]
    fn a_new_mark_opens_on_the_kind_asked_for_ready_to_apply() {
        let form = Form::open(None, Wah::Open);
        assert_eq!(KINDS[form.kind], "Open wah-wah");
        assert!(form.changed);
        assert_eq!(q_names()[form.q], "Q 4 (Cry Baby)");
        // From the heel to the toe over the beat's first half, then held.
        assert_eq!(cells(&form), [(0, 0), (6, 10), (6, 10), (12, 10)]);
        // The kind's own shape is not kept: the mark alone says it.
        assert_eq!(form.mark(), Some((Wah::Open, None)));
    }

    #[test]
    fn a_mark_s_own_shape_is_shown_and_left_alone_until_changed() {
        let shape = Shape {
            points: vec![(0.0, 30.0), (0.25, 100.0), (1.0, 0.0)],
            q: 8.0,
        };
        let mut form = Form::open(Some((Wah::Closed, Some(shape.clone()))), Wah::Open);
        assert_eq!(KINDS[form.kind], "Closed wah-wah");
        assert!(!form.changed);
        assert_eq!(q_names()[form.q], "Q 8");
        assert_eq!(form.mark(), Some((Wah::Closed, Some(shape))));
        // The end dragged up to half way; the start can't pass the middle.
        form.press(12.0, 0.0);
        form.drag(12.3, 5.2);
        form.release();
        form.press(0.0, 3.0);
        form.drag(7.0, 3.0);
        form.release();
        assert!(form.changed);
        let made = form.mark().unwrap().1.unwrap();
        assert_eq!(made.points.first().unwrap().0, 0.25);
        assert_eq!(made.points.last().unwrap().1, 50.0);
        // The middle's two places move as one.
        form.press(3.0, 10.0);
        form.drag(3.0, 6.0);
        assert!(form.points[1].row == 6 && form.points[2].row == 6);
        // A press far from every point picks none up.
        form.release();
        form.press(9.0, 9.0);
        let before = form.clone();
        form.drag(9.0, 2.0);
        assert_eq!(form.points, before.points);
    }

    #[test]
    fn picking_a_kind_gives_its_curve_and_no_wah_gives_none() {
        let mut form = Form::open(Some((Wah::Open, None)), Wah::Open);
        form.pick_q(6);
        form.pick_kind(2);
        assert_eq!(cells(&form), [(0, 10), (6, 0), (6, 0), (12, 0)]);
        let (wah, shape) = form.mark().unwrap();
        assert_eq!((wah, shape.unwrap().q), (Wah::Closed, 8.0));
        form.pick_kind(0);
        assert!(form.points.is_empty() && form.mark().is_none());
        form.pick_kind(9);
        assert_eq!(form.kind, 0);
    }
}
