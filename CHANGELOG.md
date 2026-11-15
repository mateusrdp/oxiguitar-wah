# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- The effect `wah` (mateusrdp/oxiguitar#250): a wah pedal on any track,
  rocked by the score's wah-wah marks. OxiGuitar hands it the pedal's
  position across each marked beat and the mark's Q (plugin interface 1.8,
  the track's control curves); where no mark's beat plays, the pedal is off
  and the track passes untouched. It comes in and goes out over 10 ms, has
  no latency and allocates nothing while it plays.
- The pedal is Julius O. Smith's digital Cry Baby (CCRMA, "Digital
  CryBaby"), our own code from its formulas: one resonance from 450 Hz with
  the heel down (Q 8) to 2.2 kHz with the toe down (Q 2), its gain
  following the pedal. A mark's Q scales the pedal's own (4 is the Cry
  Baby's with the pedal half way).
- The track dialog's section **Wah-wah**, for every track: whether the
  track plays the score's marks (it does unless switched off), **Heel**
  (where the sweep starts, 200 to 800 Hz), **Sweep** (how far the toe takes
  it, 1 to 3.5 octaves) and **Cry Baby**, which sets the two back. Kept per
  track in the song's `.oxg` file under `oxiguitar.wah`; a change reaches
  the running pedal while the song plays.
- `build-oxp.sh`, the contract test and the real-time checks through
  OxiGuitar's plugin host (`tests/contract.rs`), and the release and publish
  workflows (oxiguitar-packages' `plugin-release.yml` and
  `plugin-publish.yml`).
