# oxiguitar-wah

The wah-wah plugin for [OxiGuitar](https://github.com/mateusrdp/oxiguitar), a free, open-source guitar tablature and score viewer, editor and player for Windows, macOS and Linux. It plays the score's wah-wah marks: each mark rocks a wah pedal across its beat.

- Plugin file `wah.oxp`, id `oxiguitar.wah`, shown as **Wah-wah**.
- **It follows the score.** An **Open Wah-Wah** mark ("o") takes the pedal from the heel to the toe over the first half of its beat ("wah"), a **Closed** one ("+") back; marks that alternate rock it back and forth. In OxiGuitar's wah-wah editor (the palette's wah tools) each mark can have a curve of its own, drawn like a bend, and its own **Q**. On beats without a mark the pedal is off and the track passes untouched.
- **A Cry Baby by default**: Julius O. Smith's digital model of the Dunlop pedal, one resonance swept from 450 Hz (heel down, narrow) to 2.2 kHz (toe down, wide).
- Per track, in the track dialog's section **Wah-wah** (the Track tab), kept in the song's `.oxg` file beside it:
  - **Play the score's wah-wah marks**, ticked by default: untick it for a track whose marks should stay silent.
  - **Heel**, where the sweep starts (200 to 800 Hz), and **Sweep**, how far the toe takes it (1 to 3.5 octaves); **Cry Baby** sets both back (450 Hz, 2.3 octaves).
  - It works while the song plays.
- On any track: guitars, basses, keys. With the NAM plugin on the same track, OxiGuitar runs the wah first and then the amp, as a pedal in front of an amplifier; the track dialog moves one before the other.
- No latency, and nothing allocated while it plays. The pedal comes in and goes out over 10 ms, so nothing clicks.

No screenshot or example sound yet: they come with the first release.

## How it makes its sound

The filter is one second-order resonance with a zero at 0 Hz, as in Julius O. Smith's ["Digital CryBaby"](https://ccrma.stanford.edu/realsimple/faust_strings/Digital_CryBaby_Faust.html) (*Making Virtual Electric Guitars and Associated Effects Using Faust*, CCRMA, Stanford), which fits a measured Cry Baby: with the pedal at `w` from 0 (heel) to 1 (toe), the resonance is at 450 · 2^(2.3 w) Hz, its Q is 2^(2 (1 − w) + 1) and its gain 0.1 · 4^w. The code is our own, written from those formulas (`src/wah.rs`), with no other library. A mark's Q scales the pedal's own Q, 4 being the Cry Baby's with the pedal half way.

## Install

It needs an OxiGuitar with plugin interface 1.8 (mateusrdp/oxiguitar#250), which is not released yet; neither is this plugin. Once it is, every [release](https://github.com/mateusrdp/oxiguitar-wah/releases) will hold a zip per system (Linux x86_64 and ARM64, Windows x86_64 and ARM64, macOS), a `.deb`, an `.rpm` and a Flatpak for Linux, a per-user Windows installer, a macOS `.pkg`, and a `SHA256SUMS` file, as OxiGuitar's other plugins do.

## Build and test

Rust 1.99 or later from [rustup](https://rustup.rs). The plugin SDK comes from OxiGuitar's repository (a commit until the release with interface 1.8 has its tag); nothing of OxiGuitar's is on crates.io.

```sh
cargo test          # unit tests (the pedal's response against the model's numbers), the contract suite on the built .oxp file, and the real-time checks
./build-oxp.sh      # dist/wah-VERSION-SYSTEM-ARCH.oxp
```

## Licence

GPL-3.0-or-later. See LICENSE. Contributions are welcome under the same terms; see [CONTRIBUTING.md](CONTRIBUTING.md).
