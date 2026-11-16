// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The pedal: Julius O. Smith's digital Cry Baby ("Digital CryBaby" in
//! *Making Virtual Electric Guitars and Associated Effects Using Faust*,
//! CCRMA, Stanford, <https://ccrma.stanford.edu/realsimple/faust_strings/>),
//! our own code from its formulas. He measured a Dunlop Cry Baby's
//! response at several pedal angles and fitted one resonance to each: a
//! second-order section with a zero at 0 Hz,
//!
//! ```text
//! H(z) = g (1 − z⁻¹) / (1 + a1 z⁻¹ + a2 z⁻²)
//! a1 = −2 R cos θ,  a2 = R²,  R = 1 − π f / (Q · rate),  θ = 2π f / rate
//! ```
//!
//! whose frequency, Q and gain all follow the pedal `w` (0 heel down, 1
//! toe down):
//!
//! ```text
//! f = 450 · 2^(2.3 w) Hz      Q = 2^(2 (1 − w) + 1)      g = 0.1 · 4^w
//! ```
//!
//! so the heel is a narrow peak at 450 Hz (Q 8) and the toe a wide one at
//! 2.2 kHz (Q 2). Here the 450 Hz and the 2.3 octaves are the track's
//! settings ([`Voice`]), and a wah mark's Q, which the score gives for the
//! pedal half way (4 on the Cry Baby), scales the pedal's own: Q = q/4 ·
//! 2^(2 (1 − w) + 1), with the gain scaled the other way so the peak
//! stays as loud. Smith's gain is for 44.1 kHz; it is scaled to the rate,
//! as the peak's height is g · Q · rate / (2π f).
//!
//! The coefficients are worked out every [`BLOCK`] frames and glide there
//! frame by frame (one pole each, as in Smith's code: a mix of two stable
//! filters' coefficients is stable), so a rocking pedal makes no zipper
//! noise. Where the pedal is off the track passes untouched; the pedal
//! comes in and goes out over [`FADE`] seconds. No latency.

use std::f64::consts::PI;

/// Frames between two workings-out of the coefficients.
pub const BLOCK: usize = 16;

/// Seconds the pedal takes to come in or go out.
pub const FADE: f64 = 0.01;

/// The time the coefficients take to get about two thirds of the way to
/// new ones, in seconds: Smith's 0.999 a frame at 44.1 kHz.
pub const GLIDE: f64 = 1000.0 / 44_100.0;

/// The Q a wah mark has when the score gives it none: the Cry Baby's with
/// its pedal half way.
pub const Q: f64 = 4.0;

/// The resonance's highest frequency, as a share of the sample rate.
const HIGHEST: f64 = 0.45;

/// The pedal's make: where its sweep starts and how far it goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Voice {
    /// The resonance with the heel down, in Hz.
    pub heel: f64,
    /// How far the toe takes it, in octaves.
    pub range: f64,
}

impl Voice {
    /// A Dunlop Cry Baby, as Smith measured it: 450 Hz to 2.2 kHz.
    pub const CRY_BABY: Voice = Voice {
        heel: 450.0,
        range: 2.3,
    };
}

/// The resonance for one pedal position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coefficients {
    /// The gain.
    pub g: f64,
    /// The denominator's z⁻¹.
    pub a1: f64,
    /// The denominator's z⁻².
    pub a2: f64,
}

impl Coefficients {
    /// The resonance with the pedal at `pedal` (0 to 1) and a mark's Q
    /// `q`, at `rate` frames a second.
    pub fn at(voice: Voice, pedal: f64, q: f64, rate: f64) -> Coefficients {
        let w = pedal.clamp(0.0, 1.0);
        let scale = q.clamp(0.25, 64.0) / Q;
        let f = (voice.heel * (voice.range * w).exp2()).min(HIGHEST * rate);
        let sharpness = scale * (2.0 * (1.0 - w) + 1.0).exp2();
        let r = (1.0 - PI * f / (sharpness * rate)).max(0.0);
        let theta = 2.0 * PI * f / rate;
        Coefficients {
            g: 0.1 * 4f64.powf(w) / scale * (44_100.0 / rate),
            a1: -2.0 * r * theta.cos(),
            a2: r * r,
        }
    }

    /// How much louder (or softer) a steady tone of `frequency` Hz comes
    /// out, for tests and for anyone drawing the response.
    pub fn gain_at(&self, frequency: f64, rate: f64) -> f64 {
        let w = 2.0 * PI * frequency / rate;
        let (c1, s1) = (w.cos(), w.sin());
        let (c2, s2) = ((2.0 * w).cos(), (2.0 * w).sin());
        let num = ((1.0 - c1).powi(2) + s1 * s1).sqrt();
        let den = ((1.0 + self.a1 * c1 + self.a2 * c2).powi(2)
            + (self.a1 * s1 + self.a2 * s2).powi(2))
        .sqrt();
        self.g * num / den
    }
}

/// One channel's memory: the last input and the last two outputs.
#[derive(Clone, Copy, Debug, Default)]
struct Memory {
    x1: f64,
    y1: f64,
    y2: f64,
}

/// The pedal on one track, both channels.
#[derive(Clone, Debug)]
pub struct Pedal {
    voice: Voice,
    rate: f64,
    /// The coefficients now, and the ones they glide to.
    now: Coefficients,
    target: Coefficients,
    /// What a frame keeps of the way to the target.
    keep: f64,
    /// How much of the pedal is in the sound, 0 to 1, and what a frame
    /// adds or takes.
    mix: f64,
    fade: f64,
    memory: [Memory; 2],
}

impl Pedal {
    /// A pedal of this make at `rate` frames a second, off.
    pub fn new(voice: Voice, rate: f64) -> Pedal {
        let rest = Coefficients::at(voice, 0.0, Q, rate);
        Pedal {
            voice,
            rate,
            now: rest,
            target: rest,
            keep: (-1.0 / (GLIDE * rate)).exp(),
            mix: 0.0,
            fade: 1.0 / (FADE * rate).max(1.0),
            memory: [Memory::default(); 2],
        }
    }

    /// Another make, from the next block on: the pedal glides there.
    pub fn set_voice(&mut self, voice: Voice) {
        self.voice = voice;
    }

    /// Runs `left` and `right` through the pedal in place. `position`
    /// says where the pedal is at a frame of the two (0 to 1) and its
    /// mark's Q, or `None` where it is off; it is asked once every
    /// [`BLOCK`] frames.
    pub fn process(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        mut position: impl FnMut(usize) -> Option<(f64, f64)>,
    ) {
        let frames = left.len().min(right.len());
        let mut on = false;
        for frame in 0..frames {
            if frame % BLOCK == 0 {
                let at = position(frame);
                on = at.is_some();
                if let Some((pedal, q)) = at {
                    self.target = Coefficients::at(self.voice, pedal, q, self.rate);
                    if self.mix == 0.0 {
                        // Coming in from nothing: there is no sound of the
                        // pedal's to glide from.
                        self.now = self.target;
                        self.memory = [Memory::default(); 2];
                    }
                }
            }
            if !on && self.mix == 0.0 {
                continue;
            }
            self.now.g = self.target.g + (self.now.g - self.target.g) * self.keep;
            self.now.a1 = self.target.a1 + (self.now.a1 - self.target.a1) * self.keep;
            self.now.a2 = self.target.a2 + (self.now.a2 - self.target.a2) * self.keep;
            self.mix = if on {
                (self.mix + self.fade).min(1.0)
            } else {
                (self.mix - self.fade).max(0.0)
            };
            let c = self.now;
            let mix = self.mix;
            let run = |sample: &mut f32, m: &mut Memory| {
                let x = f64::from(*sample);
                let y = c.g * (x - m.x1) - c.a1 * m.y1 - c.a2 * m.y2;
                m.x1 = x;
                m.y2 = m.y1;
                m.y1 = y;
                *sample = (x + (y - x) * mix) as f32;
            };
            let [l, r] = &mut self.memory;
            run(&mut left[frame], l);
            run(&mut right[frame], r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    /// The frequency at which a resonance is loudest, searched in cents.
    fn peak(c: &Coefficients) -> (f64, f64) {
        (0..=7200)
            .map(|cents| 100.0 * (f64::from(cents) / 1200.0).exp2())
            .map(|f| (f, c.gain_at(f, RATE)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap()
    }

    #[test]
    fn the_cry_baby_sweeps_from_450_hz_to_2200_and_its_peak_narrows_at_the_heel() {
        let at = |w| Coefficients::at(Voice::CRY_BABY, w, Q, RATE);
        let (heel, heel_gain) = peak(&at(0.0));
        let (middle, _) = peak(&at(0.5));
        let (toe, toe_gain) = peak(&at(1.0));
        assert!((heel / 450.0 - 1.0).abs() < 0.03, "{heel}");
        assert!((middle / 998.0 - 1.0).abs() < 0.04, "{middle}");
        assert!((toe / 2216.0 - 1.0).abs() < 0.08, "{toe}");
        // Smith's peak heights: g · Q · 44100 / (2π f).
        assert!((heel_gain / 12.5 - 1.0).abs() < 0.05, "{heel_gain}");
        assert!((toe_gain / 2.6 - 1.0).abs() < 0.12, "{toe_gain}");
        // Nothing at 0 Hz, whatever the pedal.
        assert!(at(0.3).gain_at(0.0, RATE) < 1e-12);
        // The same pedal at another rate.
        let other = Coefficients::at(Voice::CRY_BABY, 0.0, Q, 96_000.0);
        assert!((other.gain_at(450.0, 96_000.0) / heel_gain - 1.0).abs() < 0.03);
    }

    #[test]
    fn a_mark_s_q_narrows_or_widens_the_peak_and_keeps_its_height() {
        let width = |q: f64| {
            let c = Coefficients::at(Voice::CRY_BABY, 0.5, q, RATE);
            let (f, top) = peak(&c);
            // The band within 3 dB of the peak, in cents.
            let inside = (0..=7200)
                .map(|cents| 100.0 * (f64::from(cents) / 1200.0).exp2())
                .filter(|&x| c.gain_at(x, RATE) >= top / 2f64.sqrt())
                .count();
            (f, top, inside)
        };
        let (f4, top4, wide4) = width(4.0);
        let (f16, top16, wide16) = width(16.0);
        let (_, top1, wide1) = width(1.0);
        assert!((f16 / f4 - 1.0).abs() < 0.03);
        assert!(
            wide16 * 3 < wide4 && wide4 * 3 < wide1,
            "{wide16} {wide4} {wide1}"
        );
        assert!((top16 / top4 - 1.0).abs() < 0.1 && (top1 / top4 - 1.0).abs() < 0.35);
    }

    #[test]
    fn off_it_passes_the_track_untouched_and_it_comes_in_without_a_click() {
        let sound: Vec<f32> = (0..4800)
            .map(|i| (2.0 * PI * 700.0 * f64::from(i) / RATE).sin() as f32 * 0.5)
            .collect();
        let mut pedal = Pedal::new(Voice::CRY_BABY, RATE);
        let (mut l, mut r) = (sound.clone(), sound.clone());
        pedal.process(&mut l, &mut r, |_| None);
        assert_eq!((&l, &r), (&sound, &sound));
        // On from frame 960: untouched before, no jump at the seam, and
        // by the end a 700 Hz tone under a half-way pedal is louder.
        let (mut l, mut r) = (sound.clone(), sound.clone());
        pedal.process(&mut l, &mut r, |f| (f >= 960).then_some((0.3, Q)));
        assert_eq!(l[..960], sound[..960]);
        let jump = |a: &[f32]| {
            a.windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0f32, f32::max)
        };
        assert!(
            jump(&l[950..980]) < 1.5 * jump(&sound),
            "{}",
            jump(&l[950..980])
        );
        let loud = |a: &[f32]| a.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(
            loud(&l[3800..]) > 2.0 * loud(&sound),
            "{}",
            loud(&l[3800..])
        );
        assert_eq!(l, r);
        // Off again: back to the track itself within the fade.
        let (mut l, mut r) = (sound.clone(), sound.clone());
        pedal.process(&mut l, &mut r, |_| None);
        let fade = (FADE * RATE) as usize + BLOCK;
        assert_eq!(l[fade..], sound[fade..]);
    }

    #[test]
    fn a_rocking_pedal_stays_stable_at_every_rate_and_q() {
        for rate in [22_050.0, 44_100.0, 96_000.0, 192_000.0] {
            for q in [0.25, 1.0, 4.0, 16.0, 64.0] {
                let voice = Voice {
                    heel: 250.0,
                    range: 5.0,
                };
                let mut pedal = Pedal::new(voice, rate);
                let mut noise = 12345u32;
                let mut worst = 0.0f32;
                for block in 0..200 {
                    let mut l: Vec<f32> = (0..256)
                        .map(|_| {
                            noise = noise.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                            (noise >> 8) as f32 / 8_388_608.0 - 1.0
                        })
                        .collect();
                    let mut r = l.clone();
                    // The pedal jumps from end to end every block.
                    let w = f64::from(block % 2);
                    pedal.process(&mut l, &mut r, |_| Some((w, q)));
                    worst = l.iter().fold(worst, |m, v| m.max(v.abs()));
                    assert!(l.iter().all(|v| v.is_finite()));
                }
                assert!(worst < 200.0, "rate {rate}, q {q}: {worst}");
            }
        }
    }
}
