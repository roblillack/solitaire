//! The demo script vocabulary, shared by both playback backends.
//!
//! Each verb turns into one or more synthesized input events with human pacing.
//! The backend decides what an event means: with the `recorder` feature it is
//! rendered into a GIF frame (see `recorder.rs`); without it, the script plays
//! live in a real window (see `player.rs`).

#[cfg(feature = "recorder")]
#[path = "recorder.rs"]
mod backend;
#[cfg(not(feature = "recorder"))]
#[path = "player.rs"]
mod backend;

use std::path::Path;

use saudade::{Event, Key, Modifiers, MouseButton, Point};

/// On-screen timings, in centiseconds (the GIF delay unit).
mod pace {
    /// Opening hold on the dealt board.
    pub const FIRST: u32 = 110;
    /// Between the two presses of a double-click.
    pub const DBL: u32 = 8;
    /// One step of a slow drag.
    pub const DRAG_STEP: u32 = 2;
    /// After a completed gesture — time to read the result.
    pub const ACTION: u32 = 75;
    /// Per-tick delay while the winning cascade plays.
    pub const CASCADE: u32 = 3;
    /// Closing hold before the GIF loops / the window idles.
    pub const LAST: u32 = 320;
}

/// A scripted Solitaire session.
pub struct Demo {
    backend: backend::Backend,
}

#[allow(dead_code)]
impl Demo {
    /// Start a session on the demo board at `width`×`height`.
    pub fn start(width: i32, height: i32) -> Self {
        let mut backend = backend::Backend::start(width, height);
        backend.hold(pace::FIRST);
        Self { backend }
    }

    /// Linger on the current state for `ms` milliseconds.
    pub fn pause(&mut self, ms: u32) {
        self.backend.hold(ms / 10);
    }

    /// Right-click a card to send it straight to its foundation.
    pub fn send_home(&mut self, x: i32, y: i32) {
        self.backend
            .input(pointer_down(x, y, MouseButton::Right), pace::ACTION);
        self.backend
            .input(pointer_up(x, y, MouseButton::Right), pace::ACTION);
    }

    /// Double-click a card — the other "send it home" gesture.
    pub fn double_click(&mut self, x: i32, y: i32) {
        self.backend
            .input(pointer_down(x, y, MouseButton::Left), pace::DBL);
        self.backend
            .input(pointer_up(x, y, MouseButton::Left), pace::DBL);
        self.backend
            .input(pointer_down(x, y, MouseButton::Left), pace::DBL);
        self.backend
            .input(pointer_up(x, y, MouseButton::Left), pace::ACTION);
    }

    /// Drag a card (or run) from one point to another, in smooth steps so the
    /// lifted stack visibly tracks the cursor.
    pub fn drag(&mut self, from: (i32, i32), to: (i32, i32)) {
        const STEPS: i32 = 12;
        self.backend.input(
            pointer_down(from.0, from.1, MouseButton::Left),
            pace::DRAG_STEP,
        );
        for i in 1..=STEPS {
            let x = from.0 + (to.0 - from.0) * i / STEPS;
            let y = from.1 + (to.1 - from.1) * i / STEPS;
            self.backend.input(pointer_move(x, y), pace::DRAG_STEP);
        }
        self.backend
            .input(pointer_up(to.0, to.1, MouseButton::Left), pace::ACTION);
    }

    /// Turn the stock (a left click on the stock pile).
    pub fn draw_stock(&mut self, x: i32, y: i32) {
        self.backend
            .input(pointer_down(x, y, MouseButton::Left), pace::DBL);
        self.backend
            .input(pointer_up(x, y, MouseButton::Left), pace::ACTION);
    }

    /// Trigger the auto-finish (Ctrl+F) and play the winning cascade to the end.
    pub fn finish_game(&mut self) {
        self.backend.input(ctrl_char('f'), pace::ACTION);
        self.backend.celebrate(pace::CASCADE);
    }

    /// Hold the final state, then write the GIF (recording) or run the window
    /// (live).
    pub fn stop(mut self, path: impl AsRef<Path>) -> std::io::Result<()> {
        self.backend.hold(pace::LAST);
        self.backend.finish(path.as_ref())
    }
}

fn pointer_down(x: i32, y: i32, button: MouseButton) -> Event {
    Event::PointerDown {
        pos: Point::new(x, y),
        button,
        modifiers: Modifiers::default(),
    }
}

fn pointer_up(x: i32, y: i32, button: MouseButton) -> Event {
    Event::PointerUp {
        pos: Point::new(x, y),
        button,
        modifiers: Modifiers::default(),
    }
}

fn pointer_move(x: i32, y: i32) -> Event {
    Event::PointerMove {
        pos: Point::new(x, y),
    }
}

fn ctrl_char(ch: char) -> Event {
    Event::KeyDown {
        key: Key::Char(ch),
        modifiers: Modifiers {
            control: true,
            ..Modifiers::default()
        },
    }
}
