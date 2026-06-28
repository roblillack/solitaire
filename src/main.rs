//! The Solitaire binary: a thin shell that opens a resizable window around the
//! [`Solitaire`](solitaire::ui::Solitaire) widget. All the game and UI logic
//! lives in the library, which the snapshot tests and the demo share.

use std::time::{SystemTime, UNIX_EPOCH};

use saudade::{App, WindowConfig};
use solitaire::game::DrawMode;
use solitaire::ui::{Solitaire, layout_hint};

fn main() {
    // Seed the opening deal from the wall clock so each launch is a new game.
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let (w, h) = layout_hint();
    let root = Solitaire::new(seed, DrawMode::One);

    App::new(
        WindowConfig::new("Solitaire", w, h)
            .resizable(true)
            .min_size(w, h),
        root,
    )
    .run();
}
