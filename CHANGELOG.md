# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-11-25

### Changed

- **The plugin file is a tar archive** (mateusrdp/oxiguitar#347): `build-oxp.sh` packs `manifest.json` first and the library second (`plugin.so`, `plugin.dll` or `plugin.dylib`), as OxiGuitar 0.23 and later read plugin files, which older OxiGuitars can't load. The SDK is at OxiGuitar v0.23.0 (interface 1.16); the curve ignores the pointer moving over it without a button and leaving it (interface 1.14's pointer kinds), and the contract test names no display system (`NO_DISPLAY`). `scripts/test-all.py` 1.1.0, as in every repository.

### Added

- **Every test with one command** (mateusrdp/oxiguitar#273): `scripts/test-all.py` builds and runs all of this repository's tests, and tests the plugin against the newest OxiGuitar release, with a progress bar, a summary to paste into an issue and a log to diagnose from. The manual has a Features page naming the tests of every feature (CI checks it) and a Testing page.

### Changed

- **CI outlasts a slow Ubuntu mirror**: it installs packages with `scripts/apt-install.sh`, the same file as in every OxiGuitar repository. apt is stopped only when its downloads have not grown for 2 minutes, then tried again on the next mirror; while they grow, it may take as long as it needs (24 Nov 2026: fixed 5-minute tries failed jobs on mirrors at 80 to 185 kB/s).
- **The manual explains the ideas behind the plugin** (its overview, with pictures), shows how to use it with screenshots, says in Architecture and strategy what it can't do yet, what is left out on purpose and where help is wanted, and has a Credits page listing every project it uses, with links and thanks. The README has a plain-words introduction and a Credits section.

## [0.1.0] - 2026-11-16

### Added

- The palette's **Closed Wah-Wah** and **Open Wah-Wah** (mateusrdp/oxiguitar#250,
  #293), which OxiGuitar itself no longer has: each opens the plugin's
  **wah-wah editor** for the caret's beat or the selected beats, with the
  mark (none, open, closed), the filter's Q and the pedal's curve across
  the beat, whose points are dragged. OK writes them as one step for Undo.
  The mark is the score's own; the curve and the Q are the plugin's data on
  the beat, which OxiGuitar saves in the `.oxg` file beside the song.
- The effect `wah`: a wah pedal on any track that follows the score (plugin
  interface 1.8). It is given every beat its track plays and makes one curve
  of their marks: an open mark takes the pedal from heel to toe over the
  first half of its beat, a closed one back, a mark with a curve of its own
  plays that. Where no mark's beat plays, the pedal is off and the track
  passes untouched. It comes in and goes out over 10 ms, has no latency and
  allocates nothing while it plays. It runs before amp models and
  equalizers unless the Track tab moves it.
- The pedal is Julius O. Smith's digital Cry Baby (CCRMA, "Digital
  CryBaby"), our own code from its formulas: one resonance from 450 Hz with
  the heel down (Q 8) to 2.2 kHz with the toe down (Q 2), its gain
  following the pedal. A mark's Q scales the pedal's own (4 is the Cry
  Baby's with the pedal half way).
- The Track tab's section **Wah-wah**, for every track: whether the
  track plays the score's marks (it does unless switched off), **Heel**
  (where the sweep starts, 200 to 800 Hz), **Sweep** (how far the toe takes
  it, 1 to 3.5 octaves) and **Cry Baby**, which sets the two back. Kept per
  track in the song's `.oxg` file under `oxiguitar.wah`; a change reaches
  the running pedal while the song plays.
- An example sound and the example that renders it
  (`cargo run --example sound`), and a screenshot of the editor.
- `build-oxp.sh`, the contract test and the real-time checks through
  OxiGuitar's plugin host (`tests/contract.rs`), and the release and publish
  workflows (oxiguitar-packages' `plugin-release.yml` and
  `plugin-publish.yml`).
