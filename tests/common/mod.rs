//! Helpers shared by the pixel-snapshot tests.
//!
//! Mirrors Saudade's own test harness: every widget tree is rendered against
//! the bundled DejaVu fonts (so glyph rasterization is bit-identical regardless
//! of the host's installed fonts) and compared to a checked-in PNG baseline via
//! `insta::assert_binary_snapshot!`. Review diffs with `cargo insta review`.

#![allow(dead_code)]

use saudade::mock::MockBackend;
use saudade::{Event, Font, Key, Modifiers, MouseButton, NamedKey, Point, Widget};

pub fn sans_font() -> Font {
    Font::from_sans_bytes(include_bytes!("../fonts/DejaVuSans.ttf").to_vec())
        .expect("bundled DejaVuSans.ttf failed to load")
}

pub fn mono_font() -> Font {
    Font::from_sans_bytes(include_bytes!("../fonts/DejaVuSansMono.ttf").to_vec())
        .expect("bundled DejaVuSansMono.ttf failed to load")
}

/// Every snapshot is captured at 1.0x — fractional scaling is covered by
/// Saudade's own harness, so per-resolution copies here would only multiply the
/// checked-in PNGs for no gain.
const SCALE: f32 = 1.0;

/// Render `build()` and emit one binary insta snapshot named `<name>.png`.
pub fn snapshot<F>(name: &str, width: i32, height: i32, build: F)
where
    F: FnOnce() -> Box<dyn Widget>,
{
    snapshot_one(name, width, height, build(), &[]);
}

/// Like [`snapshot`] but dispatches a sequence of synthetic events into the
/// freshly-built widget (after a warm-up layout + paint) before rendering, so
/// tests can capture interaction states deterministically.
pub fn snapshot_with_events<F, E>(name: &str, width: i32, height: i32, build: F, events: E)
where
    F: FnOnce() -> Box<dyn Widget>,
    E: FnOnce() -> Vec<Event>,
{
    snapshot_one(name, width, height, build(), &events());
}

fn snapshot_one(
    name: &str,
    width: i32,
    height: i32,
    mut widget: Box<dyn Widget>,
    events: &[Event],
) {
    let backend = MockBackend::new(width, height)
        .with_scale(SCALE)
        .with_sans_font(sans_font())
        .with_mono_font(mono_font());

    if !events.is_empty() {
        // Warm-up render: lays out at the logical size and paints once, so the
        // menu bar's cached label rects are ready for hit testing — exactly the
        // state the live app is in before the user's first input.
        let _ = backend.render(widget.as_mut());
        widget.focus_first();
        for event in events {
            backend.dispatch(widget.as_mut(), event);
        }
    }

    let snap = backend.render(widget.as_mut());
    let snap_name = format!("{name}.png");
    let mut settings = insta::Settings::clone_current();
    settings.set_prepend_module_to_snapshot(false);
    settings.set_snapshot_path("../snapshots");
    settings.bind(|| {
        insta::assert_binary_snapshot!(snap_name.as_str(), snap.to_png());
    });
}

// ----------------------------------------------------------- event builders

pub fn click(x: i32, y: i32) -> Event {
    Event::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        modifiers: Modifiers::default(),
    }
}

pub fn release(x: i32, y: i32) -> Event {
    Event::PointerUp {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        modifiers: Modifiers::default(),
    }
}

pub fn right_click(x: i32, y: i32) -> Event {
    Event::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Right,
        modifiers: Modifiers::default(),
    }
}

pub fn motion(x: i32, y: i32) -> Event {
    Event::PointerMove {
        pos: Point::new(x, y),
    }
}

pub fn tick() -> Event {
    Event::Tick
}

pub fn ctrl_char(ch: char) -> Event {
    Event::KeyDown {
        key: Key::Char(ch),
        modifiers: Modifiers {
            control: true,
            ..Modifiers::default()
        },
    }
}

pub fn alt_char(ch: char) -> Event {
    Event::KeyDown {
        key: Key::Char(ch),
        modifiers: Modifiers {
            alt: true,
            ..Modifiers::default()
        },
    }
}

pub fn key(k: NamedKey) -> Event {
    Event::KeyDown {
        key: Key::Named(k),
        modifiers: Modifiers::default(),
    }
}
