# Architecture and strategy

The plugin does three jobs. It adds palette tools, draws an editor in a dialog, and runs an effect that follows the score, meaning it is told every beat the track plays, with its marks.

The filter is a digital model of a Dunlop Cry Baby by Julius O. Smith: one resonance swept from 450 Hz with the heel down to 2.2 kHz with the toe down. The code is ours, written from his published formulas.

The open or closed mark belongs to the score. Guitar Pro files hold it, and OxiGuitar draws it with or without this plugin. A mark's curve and Q belong to the plugin. No score format holds them, so they are saved in the `.oxg` file beside the song. They follow the beat when it is copied or moved.
