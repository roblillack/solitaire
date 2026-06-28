//! Board geometry: where every pile and card sits, and what pile a point hits.
//!
//! All positions are derived from the widget's bounds each frame, so the board
//! stays centred and the tableau fan compresses to keep long columns on screen
//! when the window is short. The [`Table`](crate::ui::table::Table) widget owns
//! the bounds; this module is the pure mapping from *(pile, card index)* to a
//! rectangle and back from a point to a pile.

use saudade::{Point, Rect};

use crate::game::{Game, Pile, TABLEAU_PILES};

/// Logical card size. A 9:13-ish ratio reads as a real playing card.
pub const CARD_W: i32 = 64;
pub const CARD_H: i32 = 88;

/// Outer margin around the board and the gap between columns.
pub const MARGIN: i32 = 14;
pub const GAP: i32 = 13;

/// Vertical gap between the top row (stock / waste / foundations) and the
/// tableau below it.
const ROW_GAP: i32 = 22;

/// Fan step between successive cards in a tableau column, by whether the upper
/// card is face up (a wide reveal so the rank shows) or face down (a thin sliver).
pub const FAN_UP: i32 = 24;
const FAN_DOWN: i32 = 11;
/// Never compress the fan below this — cards must still be distinguishable.
const FAN_MIN: i32 = 7;

/// Horizontal step between the (up to three) fanned waste cards in draw-three.
pub const WASTE_FAN: i32 = 18;

/// The natural width of the seven-column board, used to centre it in the window.
pub fn content_width() -> i32 {
    2 * MARGIN + TABLEAU_PILES as i32 * CARD_W + (TABLEAU_PILES as i32 - 1) * GAP
}

/// Pure board geometry for a given widget rectangle.
#[derive(Clone, Copy)]
pub struct Geometry {
    bounds: Rect,
    /// Left edge of column 0 (the board is centred horizontally).
    origin_x: i32,
    top_y: i32,
    tableau_y: i32,
}

impl Geometry {
    pub fn new(bounds: Rect) -> Geometry {
        let content_w = content_width();
        // Centre the board, but never push column 0 off the left edge.
        let origin_x = bounds.x + ((bounds.w - content_w) / 2).max(0) + MARGIN;
        let top_y = bounds.y + MARGIN;
        Geometry {
            bounds,
            origin_x,
            top_y,
            tableau_y: top_y + CARD_H + ROW_GAP,
        }
    }

    fn column_x(&self, col: usize) -> i32 {
        self.origin_x + col as i32 * (CARD_W + GAP)
    }

    /// The stock sits at column 0 of the top row.
    pub fn stock_rect(&self) -> Rect {
        Rect::new(self.column_x(0), self.top_y, CARD_W, CARD_H)
    }

    /// The waste base (column 1). Individual fanned cards offset from here.
    pub fn waste_rect(&self) -> Rect {
        Rect::new(self.column_x(1), self.top_y, CARD_W, CARD_H)
    }

    /// Rectangle of the `slot`-th *shown* waste card (0 is leftmost). The waste
    /// shows at most the top three cards fanned to the right.
    pub fn waste_card_rect(&self, slot: usize) -> Rect {
        let base = self.waste_rect();
        Rect::new(base.x + slot as i32 * WASTE_FAN, base.y, CARD_W, CARD_H)
    }

    /// The four foundations occupy columns 3..7 of the top row.
    pub fn foundation_rect(&self, index: usize) -> Rect {
        Rect::new(self.column_x(3 + index), self.top_y, CARD_W, CARD_H)
    }

    /// Top-left of a tableau column's first card.
    pub fn tableau_origin(&self, col: usize) -> Rect {
        Rect::new(self.column_x(col), self.tableau_y, CARD_W, CARD_H)
    }

    /// The y-offset (from the column top) of each card in a tableau pile, after
    /// compressing the fan so the whole column fits the available height.
    pub fn tableau_offsets(&self, pile: &[crate::game::Card]) -> Vec<i32> {
        let n = pile.len();
        let mut offsets = Vec::with_capacity(n);
        if n == 0 {
            return offsets;
        }
        // Natural step before each card (0 before the first); a face-down upper
        // card reveals a thin sliver, a face-up one a wide reveal.
        let mut steps = Vec::with_capacity(n);
        steps.push(0);
        for card in &pile[..n - 1] {
            steps.push(if card.face_up { FAN_UP } else { FAN_DOWN });
        }
        let natural: i32 = steps.iter().sum();

        // Compress proportionally if the column would overflow the window.
        let avail = (self.bounds.bottom() - MARGIN - self.tableau_y - CARD_H).max(0);
        if natural > avail && natural > 0 {
            for step in steps.iter_mut().skip(1) {
                *step = (*step * avail / natural).max(FAN_MIN);
            }
        }

        let mut y = 0;
        for step in steps {
            y += step;
            offsets.push(y);
        }
        offsets
    }

    /// Rectangle of card `card_index` within tableau column `col`.
    pub fn tableau_card_rect(
        &self,
        col: usize,
        card_index: usize,
        pile: &[crate::game::Card],
    ) -> Rect {
        let offsets = self.tableau_offsets(pile);
        let dy = offsets.get(card_index).copied().unwrap_or(0);
        Rect::new(self.column_x(col), self.tableau_y + dy, CARD_W, CARD_H)
    }

    // --------------------------------------------------------- hit testing

    /// The pile and card index a press at `pos` should pick up from, if any.
    /// Only ever returns a foundation/waste top or a tableau card.
    pub fn pick_at(&self, game: &Game, pos: Point) -> Option<(Pile, usize)> {
        // Waste: only its top card is pickable.
        let waste = game.waste();
        if !waste.is_empty() {
            let shown = shown_waste(waste.len());
            let top_slot = shown - 1;
            if self.waste_card_rect(top_slot).contains(pos) {
                return Some((Pile::Waste, waste.len() - 1));
            }
        }
        // Foundations: only the top card.
        for i in 0..4 {
            let f = game.foundation(i);
            if !f.is_empty() && self.foundation_rect(i).contains(pos) {
                return Some((Pile::Foundation(i), f.len() - 1));
            }
        }
        // Tableau: the deepest card whose exposed strip contains the point.
        for col in 0..TABLEAU_PILES {
            let pile = game.tableau(col);
            if pile.is_empty() {
                continue;
            }
            let offsets = self.tableau_offsets(pile);
            let x = self.column_x(col);
            if pos.x < x || pos.x >= x + CARD_W {
                continue;
            }
            for idx in (0..pile.len()).rev() {
                let top = self.tableau_y + offsets[idx];
                // The last card claims its full height; the rest only their
                // visible strip up to the next card.
                let bottom = if idx + 1 < pile.len() {
                    self.tableau_y + offsets[idx + 1]
                } else {
                    top + CARD_H
                };
                if pos.y >= top && pos.y < bottom {
                    return Some((Pile::Tableau(col), idx));
                }
            }
        }
        None
    }

    /// `true` if `pos` is over the stock (whether or not it holds cards).
    pub fn hits_stock(&self, pos: Point) -> bool {
        self.stock_rect().contains(pos)
    }

    /// The pile a dropped stack at `pos` should land on, if any. Unlike
    /// [`pick_at`], this includes empty foundations and columns so a King can
    /// be dropped onto a cleared column and an Ace onto a bare foundation.
    pub fn drop_pile_at(&self, game: &Game, pos: Point) -> Option<Pile> {
        for i in 0..4 {
            if self.foundation_rect(i).contains(pos) {
                return Some(Pile::Foundation(i));
            }
        }
        for col in 0..TABLEAU_PILES {
            let x = self.column_x(col);
            if pos.x < x || pos.x >= x + CARD_W {
                continue;
            }
            // The column's drop band runs from its top to the bottom of its
            // last card (at least one card tall for an empty column).
            let pile = game.tableau(col);
            let bottom = if pile.is_empty() {
                self.tableau_y + CARD_H
            } else {
                let offsets = self.tableau_offsets(pile);
                self.tableau_y + offsets[pile.len() - 1] + CARD_H
            };
            if pos.y >= self.tableau_y && pos.y < bottom {
                return Some(Pile::Tableau(col));
            }
        }
        None
    }
}

/// How many waste cards are drawn fanned (at most three, and never more than
/// the waste holds).
pub fn shown_waste(waste_len: usize) -> usize {
    waste_len.clamp(1, 3)
}
