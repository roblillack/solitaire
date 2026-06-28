//! The scripted Solitaire demo.
//!
//! Watch it play itself live in a window:
//!
//! ```sh
//! cargo run --example demo
//! ```
//!
//! Or record it as the `demo.gif` embedded in the README:
//!
//! ```sh
//! cargo run --release --example demo --features recorder
//! ```
//!
//! Every gesture below is fed through the app's real event handling, so the
//! demo always reflects the game's current behavior. It plays the near-won
//! [`demo_deal`](solitaire::game::Game::demo_deal): a right-click and a
//! double-click send cards home, a drag drops a queen onto a king, and the
//! auto-finish triggers the winning bouncing-cards cascade.
//!
//! The coordinates are the pixel centres of the demo board's cards at the
//! window's default size — fixed, because the deal is fixed.

mod driver;

use std::path::Path;

use driver::Demo;
use solitaire::ui::layout_hint;

// Card centres on the demo board (window 554×624, 20px menu strip).
const J_HEARTS: (i32, i32) = (200, 210); // tableau 2 top
const Q_HEARTS: (i32, i32) = (200, 188); // tableau 2 after J♥ leaves
const K_CLUBS: (i32, i32) = (354, 188); // tableau 4
const J_CLUBS: (i32, i32) = (46, 210); // tableau 0 top
const J_DIAMONDS: (i32, i32) = (123, 210); // tableau 1 top
const J_SPADES: (i32, i32) = (277, 210); // tableau 3 top

fn main() -> std::io::Result<()> {
    let (w, h) = layout_hint();
    let mut demo = Demo::start(w, h);

    // Right-click frees J♥, exposing Q♥.
    demo.send_home(J_HEARTS.0, J_HEARTS.1);
    demo.pause(300);

    // Drag the exposed Q♥ onto K♣ — the headline gesture.
    demo.drag(Q_HEARTS, K_CLUBS);
    demo.pause(400);

    // Double-click J♣ home, then right-click the last two jacks.
    demo.double_click(J_CLUBS.0, J_CLUBS.1);
    demo.pause(250);
    demo.send_home(J_DIAMONDS.0, J_DIAMONDS.1);
    demo.pause(200);
    demo.send_home(J_SPADES.0, J_SPADES.1);
    demo.pause(400);

    // Auto-finish: the rest cascades home and the cards bounce.
    demo.finish_game();

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("demo.gif");
    demo.stop(path)
}
