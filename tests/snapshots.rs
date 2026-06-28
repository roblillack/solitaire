//! Pixel-snapshot tests for the Solitaire UI, rendered against the deterministic
//! game engine through Saudade's offscreen `MockBackend` so they never depend on
//! a real window, the system clock, or installed fonts.

mod common;

use common::{
    alt_char, click, ctrl_char, key, motion, right_click, snapshot, snapshot_with_events, tick,
};
use saudade::{NamedKey, Widget};
use solitaire::game::{DrawMode, Game};
use solitaire::ui::Solitaire;

/// The window size the binary uses, so snapshot coordinates match the real app.
const W: i32 = 554;
const H: i32 = 624;

/// A fresh, deterministic deal.
fn fresh() -> Box<dyn Widget> {
    Box::new(Solitaire::new(42, DrawMode::One))
}

/// The near-won demo board (full foundations, the face cards laid out, K♠ on
/// the waste) — also what the scripted demo plays.
fn demo() -> Box<dyn Widget> {
    Box::new(Solitaire::from_game(Game::demo_deal(DrawMode::One)))
}

/// The opening board: seven tableau columns (one card face up each), a full
/// stock, and four empty foundations showing their faint suit hints.
#[test]
fn new_game() {
    snapshot("new_game", W, H, fresh);
}

/// Turning the stock in draw-three mode fans three cards onto the waste.
#[test]
fn draw_three() {
    snapshot_with_events(
        "draw_three",
        W,
        H,
        || Box::new(Solitaire::new(42, DrawMode::Three)),
        || vec![click(46, 78)], // press on the stock
    );
}

/// The near-won demo board, exercising the full range of card art — every rank
/// and suit across the foundations, court cards, the felt, and the empty stock.
#[test]
fn demo_board() {
    snapshot("demo_board", W, H, demo);
}

/// Clicking the "Game" label drops its menu, with the accelerator hints
/// right-aligned beside each action — exercises menu-bar + popup compositing.
#[test]
fn game_menu_open() {
    snapshot_with_events("game_menu_open", W, H, fresh, || vec![click(20, 10)]);
}

/// Alt+H then Enter opens Help and fires its pre-highlighted About item; the
/// modal About box floats over the board.
#[test]
fn about_dialog() {
    snapshot_with_events("about_dialog", W, H, fresh, || {
        vec![alt_char('h'), key(NamedKey::Enter)]
    });
}

/// Mid-drag: right-click sends J♥ home, exposing Q♥; pressing it and moving
/// toward K♣ floats the lifted card and highlights the valid drop target.
#[test]
fn mid_drag() {
    snapshot_with_events("mid_drag", W, H, demo, || {
        vec![
            right_click(200, 210), // J♥ (tableau 2 top) -> hearts foundation
            click(200, 180),       // pick up the exposed Q♥
            motion(360, 180),      // drag it over K♣ on tableau 4
        ]
    });
}

/// The winning cascade in flight: auto-finish wins the demo board, then a run
/// of ticks sends the bouncing cards arcing across the felt.
#[test]
fn win_cascade() {
    snapshot_with_events("win_cascade", W, H, demo, || {
        let mut events = vec![ctrl_char('f')]; // auto-finish -> win -> cascade
        events.extend((0..40).map(|_| tick()));
        events
    });
}
