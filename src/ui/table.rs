//! The playfield widget: it draws the board and turns pointer input into moves
//! on a shared [`Game`].
//!
//! Saudade widgets are callback-free, so the `Table` owns the game behind an
//! `Rc<RefCell<…>>` it shares with the menu bar (which mutates the same game
//! through a command queue — see [`crate::ui::app`]). Card dragging is done by
//! hand: a press picks up a movable run, [`Widget::captures_pointer`] keeps the
//! pointer stream flowing during the drag, the lifted stack is drawn in the
//! overlay pass, and the release asks the game for the move. The winning
//! "bouncing cards" cascade is a tick-driven particle animation.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use saudade::{Event, EventCtx, Key, MouseButton, NamedKey, Painter, Point, Rect, Theme, Widget};

use crate::game::{Card, Game, Pile, Rng, TABLEAU_PILES};
use crate::ui::card_art;
use crate::ui::layout::{self, CARD_H, CARD_W, FAN_UP, Geometry};

/// How close in time two presses on the same card count as a double-click.
const DOUBLE_CLICK: Duration = Duration::from_millis(450);

/// A run of cards lifted off a pile and following the pointer.
struct Drag {
    from: Pile,
    /// Index of the lifted run's bottom card within its source pile.
    start: usize,
    cards: Vec<Card>,
    /// Pointer offset from the bottom card's top-left, so the stack tracks the
    /// grab point rather than jumping its corner to the cursor.
    grab_dx: i32,
    grab_dy: i32,
    pos: Point,
}

/// One card in flight during the winning cascade.
struct Flyer {
    card: Card,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

/// The celebratory bouncing-cards animation played on a win.
struct WinAnim {
    /// Cards still sitting on each foundation, decremented as they launch — the
    /// board draws only this many so a pile visibly empties.
    remaining: [usize; 4],
    /// Next foundation to launch from (round-robins across the four).
    next_foundation: usize,
    flyers: Vec<Flyer>,
    ticks_to_spawn: i32,
    rng: Rng,
    bounds: Rect,
    /// Set true for one frame when the cascade ends, so the host can react.
    finished: bool,
}

const GRAVITY: f32 = 0.55;
const BOUNCE: f32 = 0.78;
const TICKS_BETWEEN_LAUNCHES: i32 = 3;

pub struct Table {
    bounds: Rect,
    game: Rc<RefCell<Game>>,
    drag: Option<Drag>,
    /// The pile a drop would currently land on, highlighted while dragging.
    drop_target: Option<Pile>,
    last_click: Option<(Instant, Pile, usize)>,
    anim: Option<WinAnim>,
    /// Latched when the cascade finishes; the host consumes it to show the
    /// "you won" dialog exactly once.
    celebration_done: bool,
}

impl Table {
    pub fn new(game: Rc<RefCell<Game>>) -> Table {
        Table {
            bounds: Rect::new(0, 0, 0, 0),
            game,
            drag: None,
            drop_target: None,
            last_click: None,
            anim: None,
            celebration_done: false,
        }
    }

    fn geometry(&self) -> Geometry {
        Geometry::new(self.bounds)
    }

    /// Clear any transient interaction state — used when a fresh game is dealt.
    pub fn reset(&mut self) {
        self.drag = None;
        self.drop_target = None;
        self.last_click = None;
        self.anim = None;
        self.celebration_done = false;
    }

    pub fn is_animating(&self) -> bool {
        self.anim.is_some()
    }

    /// Take (and clear) the "cascade just finished" latch.
    pub fn take_celebration_finished(&mut self) -> bool {
        std::mem::take(&mut self.celebration_done)
    }

    /// Begin the winning bouncing-cards cascade over the (won) board.
    pub fn start_celebration(&mut self) {
        self.drag = None;
        self.drop_target = None;
        self.anim = Some(WinAnim {
            remaining: [13, 13, 13, 13],
            next_foundation: 0,
            flyers: Vec::new(),
            ticks_to_spawn: 0,
            rng: Rng::seeded(0x5011_741A_E512_0B0F),
            bounds: self.bounds,
            finished: false,
        });
    }

    // -------------------------------------------------------- interaction

    fn card_rect(&self, geom: &Geometry, game: &Game, pile: Pile, idx: usize) -> Rect {
        match pile {
            Pile::Stock => geom.stock_rect(),
            Pile::Waste => {
                let shown = layout::shown_waste(game.waste().len());
                let slot = shown.saturating_sub(1);
                geom.waste_card_rect(slot)
            }
            Pile::Foundation(i) => geom.foundation_rect(i),
            Pile::Tableau(col) => geom.tableau_card_rect(col, idx, game.tableau(col)),
        }
    }

    fn pile_cards(game: &Game, pile: Pile) -> &[Card] {
        match pile {
            Pile::Stock => game.stock(),
            Pile::Waste => game.waste(),
            Pile::Foundation(i) => game.foundation(i),
            Pile::Tableau(i) => game.tableau(i),
        }
    }

    fn on_press(&mut self, pos: Point, button: MouseButton, ctx: &mut EventCtx) {
        let geom = self.geometry();
        let pick = {
            let game = self.game.borrow();
            geom.pick_at(&game, pos)
        };

        // Right-click sends the picked pile's top card straight home.
        if button == MouseButton::Right {
            if let Some((pile, _)) = pick
                && self.game.borrow_mut().auto_to_foundation(pile)
            {
                ctx.request_paint();
            }
            return;
        }
        if button != MouseButton::Left {
            return;
        }

        let Some((pile, idx)) = pick else {
            // A press on the stock turns / recycles it; elsewhere is a no-op.
            if geom.hits_stock(pos) {
                self.last_click = None;
                if self.game.borrow_mut().draw() {
                    ctx.request_paint();
                }
            }
            return;
        };

        // Double-click on the same card auto-plays it to a foundation.
        let now = Instant::now();
        if let Some((then, last_pile, last_idx)) = self.last_click
            && last_pile == pile
            && last_idx == idx
            && now.duration_since(then) <= DOUBLE_CLICK
        {
            self.last_click = None;
            if self.game.borrow_mut().auto_to_foundation(pile) {
                ctx.request_paint();
            }
            return;
        }
        self.last_click = Some((now, pile, idx));

        // Otherwise begin a drag, if this card heads a movable run.
        let can_lift = self.game.borrow().is_movable_run(pile, idx);
        if !can_lift {
            return;
        }
        let cards = {
            let game = self.game.borrow();
            Self::pile_cards(&game, pile)[idx..].to_vec()
        };
        let rect = {
            let game = self.game.borrow();
            self.card_rect(&geom, &game, pile, idx)
        };
        self.drag = Some(Drag {
            from: pile,
            start: idx,
            cards,
            grab_dx: pos.x - rect.x,
            grab_dy: pos.y - rect.y,
            pos,
        });
        ctx.request_paint();
    }

    fn on_move(&mut self, pos: Point, ctx: &mut EventCtx) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        drag.pos = pos;
        let from = drag.from;
        let count = drag.cards.len();
        let geom = self.geometry();
        let game = self.game.borrow();
        let target = geom
            .drop_pile_at(&game, pos)
            .filter(|&t| game.can_move(from, count, t));
        drop(game);
        self.drop_target = target;
        ctx.request_paint();
    }

    fn on_release(&mut self, pos: Point, ctx: &mut EventCtx) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        self.drop_target = None;
        let geom = self.geometry();
        let mut game = self.game.borrow_mut();
        if let Some(target) = geom.drop_pile_at(&game, pos) {
            game.try_move(drag.from, drag.cards.len(), target);
        }
        drop(game);
        ctx.request_paint();
    }

    // ----------------------------------------------------------- animation

    fn tick(&mut self, ctx: &mut EventCtx) {
        let Some(anim) = self.anim.as_mut() else {
            return;
        };
        let bounds = anim.bounds;
        let geom = Geometry::new(bounds);

        // Launch the next foundation card on a stagger, cycling the four piles.
        if anim.ticks_to_spawn <= 0 {
            for _ in 0..4 {
                let f = anim.next_foundation;
                anim.next_foundation = (anim.next_foundation + 1) % 4;
                if anim.remaining[f] > 0 {
                    anim.remaining[f] -= 1;
                    let card = self.game.borrow().foundation(f)[anim.remaining[f]];
                    let rect = geom.foundation_rect(f);
                    // A sideways kick (away from centre) plus a small upward pop.
                    let dir = if rect.x < bounds.x + bounds.w / 2 {
                        1.0
                    } else {
                        -1.0
                    };
                    let speed = 2.0 + anim.rng.below(4) as f32;
                    anim.flyers.push(Flyer {
                        card,
                        x: rect.x as f32,
                        y: rect.y as f32,
                        vx: dir * speed,
                        vy: -(2.0 + anim.rng.below(3) as f32),
                    });
                    anim.ticks_to_spawn = TICKS_BETWEEN_LAUNCHES;
                    break;
                }
            }
        } else {
            anim.ticks_to_spawn -= 1;
        }

        // Advance every flyer; bounce off the floor; drop those off the sides.
        let floor = (bounds.bottom() - CARD_H) as f32;
        anim.flyers.retain_mut(|f| {
            f.vy += GRAVITY;
            f.x += f.vx;
            f.y += f.vy;
            if f.y >= floor {
                f.y = floor;
                f.vy = -f.vy * BOUNCE;
            }
            let off_left = (f.x as i32 + CARD_W) < bounds.x;
            let off_right = f.x as i32 > bounds.right();
            !(off_left || off_right)
        });

        let launched_all = anim.remaining.iter().all(|&n| n == 0);
        if launched_all && anim.flyers.is_empty() {
            anim.finished = true;
        }
        if anim.finished {
            self.anim = None;
            self.celebration_done = true;
        }
        ctx.request_paint();
    }

    // ------------------------------------------------------------- drawing

    /// The number of cards currently shown on foundation `i` (fewer than the
    /// game holds while the cascade is emptying it).
    fn foundation_shown(&self, i: usize, game: &Game) -> usize {
        match &self.anim {
            Some(anim) => anim.remaining[i],
            None => game.foundation(i).len(),
        }
    }

    fn draw_pile_top(painter: &mut Painter, rect: Rect, card: Option<Card>) {
        match card {
            Some(c) if c.face_up => card_art::draw_face(painter, rect, c),
            Some(_) => card_art::draw_back(painter, rect),
            None => {}
        }
    }

    /// Where a card dropped on `pile` would land, for the drop highlight.
    fn landing_rect(&self, geom: &Geometry, game: &Game, pile: Pile) -> Rect {
        match pile {
            Pile::Foundation(i) => geom.foundation_rect(i),
            Pile::Tableau(col) => {
                let cards = game.tableau(col);
                if cards.is_empty() {
                    geom.tableau_origin(col)
                } else {
                    let r = geom.tableau_card_rect(col, cards.len() - 1, cards);
                    Rect::new(r.x, r.y + FAN_UP, CARD_W, CARD_H)
                }
            }
            _ => geom.stock_rect(),
        }
    }
}

impl Widget for Table {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
        if let Some(anim) = self.anim.as_mut() {
            anim.bounds = bounds;
        }
    }

    fn paint(&mut self, painter: &mut Painter, _theme: &Theme) {
        let geom = self.geometry();
        painter.fill_rect(self.bounds, card_art::FELT);

        let game = self.game.borrow();

        // --- top row: stock, waste, foundations ---
        if game.stock().is_empty() {
            card_art::draw_empty(painter, geom.stock_rect(), Some(card_art::recycle_image()));
        } else {
            card_art::draw_back(painter, geom.stock_rect());
        }

        // Waste: up to three fanned; the topmost is hidden while it is dragged.
        let waste = game.waste();
        let dragging_waste = matches!(
            self.drag,
            Some(Drag {
                from: Pile::Waste,
                ..
            })
        );
        if waste.is_empty() {
            card_art::draw_empty(painter, geom.waste_rect(), None);
        } else {
            let shown = layout::shown_waste(waste.len());
            let first = waste.len() - shown;
            for (slot, card) in waste[first..].iter().enumerate() {
                let is_top = first + slot == waste.len() - 1;
                if is_top && dragging_waste {
                    continue;
                }
                card_art::draw_face(painter, geom.waste_card_rect(slot), *card);
            }
        }

        for i in 0..4 {
            let rect = geom.foundation_rect(i);
            let shown = self.foundation_shown(i, &game);
            // Hide the top foundation card while it is being dragged off.
            let hide_top =
                matches!(self.drag, Some(Drag { from: Pile::Foundation(f), .. }) if f == i);
            let shown = if hide_top {
                shown.saturating_sub(1)
            } else {
                shown
            };
            if shown == 0 {
                card_art::draw_empty(painter, rect, card_art::foundation_hint(i));
            } else {
                Self::draw_pile_top(painter, rect, Some(game.foundation(i)[shown - 1]));
            }
        }

        // --- tableau ---
        for col in 0..TABLEAU_PILES {
            let cards = game.tableau(col);
            // Cards from the dragged run's start are floating, not on the pile.
            let hide_from = match self.drag {
                Some(Drag {
                    from: Pile::Tableau(c),
                    start,
                    ..
                }) if c == col => Some(start),
                _ => None,
            };
            if cards.is_empty() {
                card_art::draw_empty(painter, geom.tableau_origin(col), None);
                continue;
            }
            for (idx, card) in cards.iter().enumerate() {
                if hide_from.is_some_and(|s| idx >= s) {
                    break;
                }
                let rect = geom.tableau_card_rect(col, idx, cards);
                if card.face_up {
                    card_art::draw_face(painter, rect, *card);
                } else {
                    card_art::draw_back(painter, rect);
                }
            }
        }
    }

    fn paint_overlay(&mut self, painter: &mut Painter, _theme: &Theme) {
        // Drop-target highlight, beneath the floating stack. `drop_target` is
        // only ever set while a drag is in flight, so a highlight implies a drag.
        if let Some(target) = self.drop_target {
            let game = self.game.borrow();
            let geom = self.geometry();
            let rect = self.landing_rect(&geom, &game, target);
            card_art::draw_drop_hint(painter, rect);
        }

        // The lifted stack tracking the pointer.
        if let Some(drag) = self.drag.as_ref() {
            let base_x = drag.pos.x - drag.grab_dx;
            let base_y = drag.pos.y - drag.grab_dy;
            for (k, card) in drag.cards.iter().enumerate() {
                let rect = Rect::new(base_x, base_y + k as i32 * FAN_UP, CARD_W, CARD_H);
                card_art::draw_face(painter, rect, *card);
            }
        }

        // The winning cascade flies over everything.
        if let Some(anim) = self.anim.as_ref() {
            for flyer in &anim.flyers {
                let rect = Rect::new(flyer.x as i32, flyer.y as i32, CARD_W, CARD_H);
                card_art::draw_face(painter, rect, flyer.card);
            }
        }
    }

    fn event(&mut self, event: &Event, ctx: &mut EventCtx) {
        // Ticks always drive the cascade — even though the board ignores *input*
        // while it plays.
        if let Event::Tick = event {
            self.tick(ctx);
            return;
        }
        // The board ignores input while the victory cascade plays.
        if self.anim.is_some() {
            return;
        }
        match event {
            Event::PointerDown { pos, button, .. } => self.on_press(*pos, *button, ctx),
            Event::PointerMove { pos } => self.on_move(*pos, ctx),
            Event::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => self.on_release(*pos, ctx),
            // Lost the pointer mid-drag (e.g. the window lost focus): drop the
            // lifted cards back where they came from rather than leaving them
            // stuck to the cursor.
            Event::PointerLeave if self.drag.is_some() => {
                self.drag = None;
                self.drop_target = None;
                ctx.request_paint();
            }
            // Space / Enter turns the stock — a handy keyboard affordance.
            Event::KeyDown {
                key: Key::Named(NamedKey::Space) | Key::Named(NamedKey::Enter),
                modifiers,
            } if !modifiers.has_command() => {
                let drawn = self.game.borrow_mut().draw();
                if drawn {
                    ctx.request_paint();
                }
            }
            _ => {}
        }
    }

    fn captures_pointer(&self) -> bool {
        self.drag.is_some()
    }

    fn wants_ticks(&self) -> bool {
        self.anim.is_some()
    }
}
