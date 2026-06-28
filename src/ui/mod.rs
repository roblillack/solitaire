//! The Saudade front-end: the board widget, its geometry and card art, the
//! menu bar, and the top-level [`Solitaire`] widget that ties them together.
//!
//! The binary, the snapshot tests, and the demo all drive the same
//! [`Solitaire`] widget — only the backend differs (a real window, the
//! offscreen `MockBackend`, or the GIF recorder).

mod app;
mod card_art;
mod layout;
mod menu;
mod table;

pub use app::Solitaire;
pub use menu::Command;
pub use table::Table;

/// The window's preferred (and minimum) logical size: the natural width of the
/// seven-column board, and a height tall enough to fan out a full game.
pub fn layout_hint() -> (i32, i32) {
    (layout::content_width(), 624)
}
