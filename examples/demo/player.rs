//! Live playback backend: plays the same script in a real window.
//!
//! Without the `recorder` feature the demo opens an actual Saudade window and
//! plays itself. [`AutoPlayer`] wraps the real [`Solitaire`] widget and, driven
//! by the runtime's animation ticks, injects the scripted pointer/keyboard
//! events on a wall-clock schedule — so what you watch live is exactly what the
//! recorder captures into the GIF. You can still grab the mouse and take over at
//! any time; the board is the same live widget.

use std::path::Path;
use std::time::{Duration, Instant};

use saudade::{App, Event, EventCtx, Painter, PopupRequest, Rect, Theme, Widget, WindowConfig};
use solitaire::game::{DrawMode, Game};
use solitaire::ui::Solitaire;

/// Builds the timed event script, then runs it live.
pub struct Backend {
    width: i32,
    height: i32,
    script: Vec<(Duration, Event)>,
    clock: Duration,
}

impl Backend {
    pub fn start(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            script: Vec::new(),
            clock: Duration::ZERO,
        }
    }

    /// Schedule an input event at the current point on the timeline, then
    /// advance the clock by `delay` centiseconds.
    pub fn input(&mut self, event: Event, delay: u32) {
        self.script.push((self.clock, event));
        self.advance(delay);
    }

    /// Advance the timeline by `cs` centiseconds with nothing scheduled.
    pub fn hold(&mut self, cs: u32) {
        self.advance(cs);
    }

    /// Nothing to schedule for the cascade — once the auto-finish key is
    /// injected the live widget animates itself off the runtime's ticks. Just
    /// leave time on the clock so playback doesn't end early.
    pub fn celebrate(&mut self, _delay: u32) {
        self.advance(600);
    }

    fn advance(&mut self, cs: u32) {
        self.clock += Duration::from_millis(cs as u64 * 10);
    }

    /// Open the window and play the script (the `path` is unused live).
    pub fn finish(self, _path: &Path) -> std::io::Result<()> {
        let player = AutoPlayer {
            inner: Solitaire::from_game(Game::demo_deal(DrawMode::One)),
            script: self.script,
            next: 0,
            start: None,
        };
        App::new(
            WindowConfig::new("Solitaire \u{2014} Demo", self.width, self.height)
                .resizable(true)
                .min_size(self.width, self.height),
            player,
        )
        .run();
        Ok(())
    }
}

/// Wraps the real game widget and feeds it the scripted events on schedule,
/// while forwarding the runtime's ticks so the win cascade still animates.
struct AutoPlayer {
    inner: Solitaire,
    script: Vec<(Duration, Event)>,
    next: usize,
    start: Option<Instant>,
}

impl Widget for AutoPlayer {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn layout(&mut self, bounds: Rect) {
        self.inner.layout(bounds);
    }

    fn paint(&mut self, painter: &mut Painter, theme: &Theme) {
        self.inner.paint(painter, theme);
    }

    fn paint_overlay(&mut self, painter: &mut Painter, theme: &Theme) {
        self.inner.paint_overlay(painter, theme);
    }

    fn event(&mut self, event: &Event, ctx: &mut EventCtx) {
        if let Event::Tick = event {
            let now = Instant::now();
            let started = *self.start.get_or_insert(now);
            let elapsed = now.duration_since(started);
            // Fire every scripted event whose time has arrived.
            while self.next < self.script.len() && self.script[self.next].0 <= elapsed {
                let scripted = self.script[self.next].1.clone();
                self.next += 1;
                self.inner.event(&scripted, ctx);
            }
            // Forward the tick so the inner widget's own animation advances.
            self.inner.event(&Event::Tick, ctx);
            ctx.request_paint();
        } else {
            // Let a watching human take over the mouse / keyboard at any time.
            self.inner.event(event, ctx);
        }
    }

    fn captures_pointer(&self) -> bool {
        self.inner.captures_pointer()
    }

    fn collect_popups(&self, out: &mut Vec<PopupRequest>) {
        self.inner.collect_popups(out);
    }

    fn wants_ticks(&self) -> bool {
        // Keep ticking while events remain to fire, or while the board is
        // animating its win cascade.
        self.next < self.script.len() || self.inner.wants_ticks()
    }
}
