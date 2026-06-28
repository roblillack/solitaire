//! The top-level widget: it owns the game, composites the menu bar, the board
//! and the modal dialogs, and turns queued [`Command`]s and keyboard shortcuts
//! into game actions.
//!
//! The board and the menu share one `Rc<RefCell<Game>>`. The menu can't reach
//! the game directly (its callbacks only get an `EventCtx`), so it pushes
//! [`Command`]s onto a shared queue that this widget drains after every event —
//! the same callback-free pattern the rest of the app uses. Winning kicks off
//! the board's bouncing-cards cascade; when it finishes, a "deal again?" dialog
//! appears.

use std::cell::RefCell;
use std::rc::Rc;

use saudade::{Dialog, Event, EventCtx, Key, Painter, PopupRequest, Rect, Theme, Widget};

use crate::game::{DrawMode, Game};
use crate::ui::menu::{Command, CommandQueue, build_menu_bar};
use crate::ui::table::Table;

/// Height of the in-window menu strip, in logical pixels.
const MENU_H: i32 = 20;

const ABOUT_TEXT: &str = "Solitaire\n\
    \n\
    Klondike, Windows 3.1 style.\n\
    Built with the Saudade toolkit.\n\
    \n\
    \u{00A9} 2026 Robert Lillack";

/// Tracks the one-shot win celebration so it neither repeats nor blocks a fresh
/// deal.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WinPhase {
    /// The game is in play (or simply not won).
    None,
    /// The bouncing-cards cascade is running.
    Celebrating,
    /// The cascade finished and the win dialog has been shown.
    Done,
}

pub struct Solitaire {
    bounds: Rect,
    game: Rc<RefCell<Game>>,
    queue: CommandQueue,
    menubar: saudade::MenuBar,
    table: Table,
    dialog: Dialog,
    win_phase: WinPhase,
    /// Bumped on every new deal so [`fresh_seed`](Self::fresh_seed) never
    /// repeats within the same wall-clock instant.
    deals: u64,
}

impl Solitaire {
    /// A new game dealt from `seed`.
    pub fn new(seed: u64, draw_mode: DrawMode) -> Solitaire {
        Self::from_game(Game::new(seed, draw_mode))
    }

    /// Build the UI around an already-constructed [`Game`]. The snapshot tests
    /// and the scripted demo use this to inject a deterministic board.
    pub fn from_game(game: Game) -> Solitaire {
        let game = Rc::new(RefCell::new(game));
        let queue: CommandQueue = Rc::new(RefCell::new(Vec::new()));
        Solitaire {
            bounds: Rect::new(0, 0, 0, 0),
            menubar: build_menu_bar(queue.clone()),
            table: Table::new(game.clone()),
            dialog: Dialog::new(),
            game,
            queue,
            win_phase: WinPhase::None,
            deals: 0,
        }
    }

    /// A pseudo-unique seed for a fresh deal, mixing the wall clock with a
    /// per-deal counter so two deals in the same instant still differ.
    fn fresh_seed(&mut self) -> u64 {
        self.deals = self.deals.wrapping_add(1);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        nanos ^ self.deals.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }

    /// Map a Ctrl+letter shortcut to its command.
    fn shortcut(ch: char) -> Option<Command> {
        match ch.to_ascii_lowercase() {
            'n' => Some(Command::NewDeal),
            'r' => Some(Command::Restart),
            'z' => Some(Command::Undo),
            'f' => Some(Command::AutoFinish),
            'm' => Some(Command::ToggleDraw),
            'q' => Some(Command::Quit),
            _ => None,
        }
    }

    /// Drain and apply every queued command.
    fn drain_commands(&mut self, ctx: &mut EventCtx) {
        let commands: Vec<Command> = self.queue.borrow_mut().drain(..).collect();
        for command in commands {
            match command {
                Command::NewDeal => {
                    let mode = self.game.borrow().draw_mode();
                    let seed = self.fresh_seed();
                    *self.game.borrow_mut() = Game::new(seed, mode);
                    self.table.reset();
                    self.win_phase = WinPhase::None;
                    ctx.request_paint();
                }
                Command::Restart => {
                    let (seed, mode) = {
                        let game = self.game.borrow();
                        (game.seed(), game.draw_mode())
                    };
                    *self.game.borrow_mut() = Game::new(seed, mode);
                    self.table.reset();
                    self.win_phase = WinPhase::None;
                    ctx.request_paint();
                }
                Command::Undo => {
                    if self.game.borrow_mut().undo() {
                        self.table.reset();
                        self.win_phase = WinPhase::None;
                        ctx.request_paint();
                    }
                }
                Command::AutoFinish => {
                    let can = self.game.borrow().can_auto_finish();
                    if can {
                        self.game.borrow_mut().auto_finish();
                        ctx.request_paint();
                    }
                }
                Command::ToggleDraw => {
                    let mode = self.game.borrow().draw_mode().toggled();
                    self.game.borrow_mut().set_draw_mode(mode);
                    ctx.request_paint();
                }
                Command::About => {
                    self.dialog.show_info("About Solitaire", ABOUT_TEXT);
                    ctx.request_paint();
                }
                Command::Quit => ctx.close(),
            }
        }
    }

    /// Drive the win celebration: start the cascade when the game is first won,
    /// and pop the "deal again?" dialog once it finishes.
    fn poll_win(&mut self, ctx: &mut EventCtx) {
        let won = self.game.borrow().is_won();
        if !won {
            self.win_phase = WinPhase::None;
        } else if self.win_phase == WinPhase::None {
            self.win_phase = WinPhase::Celebrating;
            self.table.start_celebration();
            ctx.request_paint();
        }
        if self.table.take_celebration_finished() {
            self.win_phase = WinPhase::Done;
            let queue = self.queue.clone();
            self.dialog.show_confirm(
                "Solitaire",
                "Congratulations \u{2014} you won!\n\nDeal a new game?",
                "New Game",
                move |ctx| {
                    queue.borrow_mut().push(Command::NewDeal);
                    ctx.request_paint();
                },
            );
            ctx.request_paint();
        }
    }

    /// Common tail of `event`: apply queued commands, then advance win state.
    fn after_event(&mut self, ctx: &mut EventCtx) {
        self.drain_commands(ctx);
        self.poll_win(ctx);
    }
}

impl Widget for Solitaire {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.menubar
            .layout(Rect::new(bounds.x, bounds.y, bounds.w, MENU_H));
        self.table.layout(Rect::new(
            bounds.x,
            bounds.y + MENU_H,
            bounds.w,
            (bounds.h - MENU_H).max(0),
        ));
        self.dialog.layout(bounds);
    }

    fn paint(&mut self, painter: &mut Painter, theme: &Theme) {
        self.table.paint(painter, theme);
        self.menubar.paint(painter, theme);
        // Floating content over the siblings: the drag preview and the win
        // cascade, then the menu drop-down, then the modal dialog.
        self.table.paint_overlay(painter, theme);
        self.menubar.paint_overlay(painter, theme);
        self.dialog.paint(painter, theme);
        self.dialog.paint_overlay(painter, theme);
    }

    fn event(&mut self, event: &Event, ctx: &mut EventCtx) {
        // A modal dialog swallows the whole tree while it is open.
        if self.dialog.is_open() {
            self.dialog.event(event, ctx);
            self.after_event(ctx);
            return;
        }

        // Ctrl+letter shortcuts, before anything else claims the key.
        if let Event::KeyDown {
            key: Key::Char(ch),
            modifiers,
        } = event
            && modifiers.control
            && !modifiers.alt
            && let Some(command) = Self::shortcut(*ch)
        {
            self.queue.borrow_mut().push(command);
            ctx.consume_event();
            self.after_event(ctx);
            return;
        }

        if event.is_keyboard() {
            // The menu bar gets first crack (Alt mnemonics, an open menu).
            self.menubar.event(event, ctx);
            if !ctx.is_consumed() && !self.menubar.captures_pointer() {
                self.table.event(event, ctx);
            }
        } else if event.position().is_none() {
            // Non-positional (PointerLeave, Tick): both children see it.
            self.menubar.event(event, ctx);
            self.table.event(event, ctx);
        } else {
            let pos = event.position().expect("positional event has a position");
            if self.table.captures_pointer() {
                // A drag in progress keeps the pointer even over the menu strip.
                self.table.event(event, ctx);
            } else if self.menubar.captures_pointer() || self.menubar.bounds().contains(pos) {
                self.menubar.event(event, ctx);
            } else {
                self.table.event(event, ctx);
            }
        }

        self.after_event(ctx);
    }

    fn captures_pointer(&self) -> bool {
        self.dialog.is_open() || self.menubar.captures_pointer() || self.table.captures_pointer()
    }

    fn collect_popups(&self, out: &mut Vec<PopupRequest>) {
        self.menubar.collect_popups(out);
        self.dialog.collect_popups(out);
    }

    fn wants_ticks(&self) -> bool {
        self.table.wants_ticks() || self.dialog.wants_ticks()
    }
}
