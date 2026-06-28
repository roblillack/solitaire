//! The menu bar and the command queue it drives.
//!
//! Saudade menu callbacks receive only an [`EventCtx`], not the game, so the
//! menu can't mutate state directly. Instead each item pushes a [`Command`]
//! onto a shared queue; the root widget drains the queue after every event and
//! applies the commands (see [`crate::ui::app::Solitaire`]). This is the same
//! callback-free indirection the rest of the app uses to share one game between
//! the board and the menu.

use std::cell::RefCell;
use std::rc::Rc;

use saudade::{Menu, MenuBar, MenuItem, Rect};

/// A queued user request, produced by a menu item (or a keyboard shortcut) and
/// applied by the root widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Deal a brand-new random game.
    NewDeal,
    /// Re-deal the current seed from the start.
    Restart,
    /// Undo the last move.
    Undo,
    /// Play every remaining card to the foundations (only offered when it is
    /// guaranteed to win).
    AutoFinish,
    /// Switch between turning one and three cards from the stock.
    ToggleDraw,
    /// Show the About box.
    About,
    /// Quit the program.
    Quit,
}

/// A shared queue of pending [`Command`]s.
pub type CommandQueue = Rc<RefCell<Vec<Command>>>;

/// Build the Game / Help menu bar, wiring each item to push its [`Command`].
/// The bar is created at a zero rect; the root lays it out as a top strip.
pub fn build_menu_bar(queue: CommandQueue) -> MenuBar {
    let push = move |queue: &CommandQueue, cmd: Command| {
        let queue = queue.clone();
        move |ctx: &mut saudade::EventCtx| {
            queue.borrow_mut().push(cmd);
            ctx.request_paint();
        }
    };

    MenuBar::new(Rect::new(0, 0, 0, 0))
        .add_menu(Menu::new(
            "&Game",
            vec![
                MenuItem::action("&New Deal", push(&queue, Command::NewDeal)).with_accel("Ctrl+N"),
                MenuItem::action("&Restart Deal", push(&queue, Command::Restart))
                    .with_accel("Ctrl+R"),
                MenuItem::separator(),
                MenuItem::action("&Undo", push(&queue, Command::Undo)).with_accel("Ctrl+Z"),
                MenuItem::action("&Auto-Finish", push(&queue, Command::AutoFinish))
                    .with_accel("Ctrl+F"),
                MenuItem::separator(),
                MenuItem::action("Toggle Draw &Mode", push(&queue, Command::ToggleDraw))
                    .with_accel("Ctrl+M"),
                MenuItem::separator(),
                MenuItem::action("E&xit", push(&queue, Command::Quit)).with_accel("Ctrl+Q"),
            ],
        ))
        .add_menu(Menu::new(
            "&Help",
            vec![MenuItem::action(
                "&About Solitaire\u{2026}",
                push(&queue, Command::About),
            )],
        ))
}
