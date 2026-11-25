# Architecture and strategy

## How it fits into OxiGuitar

The plugin does three jobs through OxiGuitar's plugin interface:

1. It adds two **tools** to the palette, which write the open or closed mark on the caret's beat or on the selected beats, as one Undo step.
2. It draws the **editor**, a dialog with the mark, its Q and the pedal's curve.
3. It runs an **effect that follows the score**: OxiGuitar tells it every beat the track plays, with its marks, and the effect builds one curve for the pedal out of them. An open mark takes the pedal from heel to toe over the first half of its beat, a closed mark takes it back, and a mark with a curve of its own plays that. On beats with no mark the pedal is off and the track passes untouched.

The effect runs before the amp and the equalizer, like a pedal in front of an amplifier, unless you move it in the Track tab.

## The sound

The filter is Julius O. Smith's digital model of a Dunlop Cry Baby, from *Making Virtual Electric Guitars and Associated Effects Using Faust* (CCRMA, Stanford). He measured a real Cry Baby at several pedal angles and fitted one resonance to each. With the pedal at `w`, from 0 (heel) to 1 (toe), the peak sits at 450 · 2^(2.3 w) Hz, its Q is 2^(2 (1 − w) + 1) and its gain 0.1 · 4^w. The code is ours, written from those formulas, with no other library. It has no latency, allocates nothing while playing, and fades in and out over 10 ms so nothing clicks.

## Current limitations

- **One pedal.** Every track's pedal is a Cry Baby, adjustable only by where the sweep starts (**Heel**) and how far it goes (**Sweep**). Other classic wahs (a Vox, a bass wah) would each need their own measured curve.
- **Curves don't travel.** A mark's curve and Q live in the `.oxg` file. Send someone the `.gp` file alone and they get plain open and closed marks.
- **No auto-wah.** The pedal only moves where the score has marks. A wah that follows how hard you pick (an envelope filter) is a different effect.
- **No expression pedal input.** You can't rock a real pedal plugged into the computer: OxiGuitar has no live input yet.

## Left out on purpose

- **Wah support in the core.** The mark is the score's, but the tool, the editor and the sound are all the plugin's, so OxiGuitar stays lean and anyone can write a different wah plugin.
- **Editing while playing.** OxiGuitar doesn't edit the score while a song plays. The Track tab's Heel and Sweep do work while playing, since they are the track's settings, not the score.

## Where you can help

- **More pedals.** If you can measure a real wah's response at several positions, a second preset is a self-contained job (`src/wah.rs`, `src/settings.rs`).
- **Listening.** Tell us if the default open and closed shapes feel right in real songs.

Every change needs its tests and a row on the [Features](features.md) page; the [Testing](testing.md) page says how to run them.
