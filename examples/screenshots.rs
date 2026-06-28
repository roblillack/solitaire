//! Renders the README screenshots straight from the real [`Solitaire`] widget.
//!
//! Like the snapshot tests, it drives the deterministic game engine through
//! Saudade's offscreen [`MockBackend`], then wraps each window in Canoe-style
//! chrome (a title bar, frame and drop shadow) with [`MockBackend::render_framed`]
//! so the images look the way the app does on the desktop. No window system is
//! needed and the output is byte-stable. Refresh the images under `docs/` with:
//!
//! ```sh
//! cargo run --example screenshots
//! ```

use std::path::Path;

use saudade::mock::MockBackend;
use saudade::{Event, Font, Key, Modifiers, Widget, WindowChrome};
use solitaire::game::{DrawMode, Game};
use solitaire::ui::Solitaire;

const WIDTH: i32 = 554;

fn main() {
    // The opening deal: the classic seven-column board.
    let board = Box::new(Solitaire::new(7, DrawMode::One));
    shoot("screenshot.png", WIDTH, 470, "Solitaire", board, &[]);

    // The winning bouncing-cards cascade in full flight.
    let win = Box::new(Solitaire::from_game(Game::demo_deal(DrawMode::One)));
    let mut events = vec![ctrl_char('f')];
    events.extend((0..56).map(|_| Event::Tick));
    shoot("screenshot-win.png", WIDTH, 624, "Solitaire", win, &events);
}

/// Render `widget` at `w × h`, dispatching `events` first, then wrap it in
/// window chrome titled `title` and write the framed PNG into `docs/<name>`.
fn shoot(name: &str, w: i32, h: i32, title: &str, mut widget: Box<dyn Widget>, events: &[Event]) {
    let backend = MockBackend::new(w, h)
        .with_scale(1.0)
        .with_sans_font(sans_font())
        .with_mono_font(mono_font());

    if !events.is_empty() {
        let _ = backend.render(widget.as_mut());
        widget.focus_first();
        for event in events {
            backend.dispatch(widget.as_mut(), event);
        }
    }

    let chrome = WindowChrome::resizable(title);
    let png = backend.render_framed(widget.as_mut(), &chrome).to_png();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join(name);
    std::fs::create_dir_all(path.parent().unwrap()).expect("create docs/");
    std::fs::write(&path, png).expect("write screenshot");
    println!("wrote {}", path.display());
}

fn sans_font() -> Font {
    Font::from_sans_bytes(include_bytes!("../tests/fonts/DejaVuSans.ttf").to_vec())
        .expect("bundled DejaVuSans.ttf failed to load")
}

fn mono_font() -> Font {
    Font::from_sans_bytes(include_bytes!("../tests/fonts/DejaVuSansMono.ttf").to_vec())
        .expect("bundled DejaVuSansMono.ttf failed to load")
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
