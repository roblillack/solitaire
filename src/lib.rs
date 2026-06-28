//! Solitaire — a classic Windows 3.1-flavored Klondike, built on the
//! [Saudade](https://github.com/roblillack/saudade) toolkit.
//!
//! The crate is split in two:
//!
//! * [`game`] is the pure rules engine — the deal, the legal-move rules, undo,
//!   and win detection — with no dependency on the toolkit. It is deterministic
//!   for a given seed, which is what keeps the snapshot tests and the scripted
//!   demo byte-stable.
//! * [`ui`] draws the board with Saudade and turns pointer / keyboard input
//!   into moves on a shared [`game::Game`].
//!
//! The binary ([`main.rs`](../src/main.rs)) is a thin shell that opens a window
//! around [`ui::Solitaire`]; the snapshot tests and the demo drive the very
//! same widget through Saudade's offscreen `MockBackend`.

pub mod game;
pub mod ui;
