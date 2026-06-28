//! Procedural card rendering.
//!
//! Cards are drawn from primitives rather than bitmaps, so they stay crisp at
//! every DPI and need no bundled raster assets: a rounded white face with the
//! rank and suit in opposite corners and a large central pip, a cross-hatched
//! back, and outlined empty slots.
//!
//! The suit pips are vector marks baked from SVG at compile time with Saudade's
//! [`include_svg!`] — so they render identically on every platform (the old
//! Unicode ♠♥♦♣ glyphs depended on a font that actually carried them, which is
//! not a safe assumption) and carry no SVG parser into the binary. Each pip is a
//! single monochrome shape painted in the card's ink with [`Painter::draw_svg_tinted`].

use saudade::{Color, Painter, Rect, SvgImage, include_svg};

use crate::game::{Card, Color as CardColor, Suit};

/// The green Solitaire table the whole board floats on.
pub const FELT: Color = Color::rgb(0x00, 0x70, 0x30);
/// A face-down card's two-tone back.
const BACK_BASE: Color = Color::rgb(0x12, 0x3A, 0x8C);
const BACK_WEAVE: Color = Color::rgb(0x2E, 0x5C, 0xC8);
/// The red and black suit inks.
const RED: Color = Color::rgb(0xC0, 0x00, 0x00);
const BLACK: Color = Color::rgb(0x10, 0x10, 0x10);
const FACE: Color = Color::WHITE;
const BORDER: Color = Color::BLACK;
/// Highlight ring drawn around a valid drop target.
const DROP_HINT: Color = Color::rgb(0xFF, 0xF0, 0x60);
/// Faint outline / pip for empty slots, drawn over the felt.
const SLOT_LINE: Color = Color::rgb(0x00, 0x55, 0x24);

const RANK_SIZE: f32 = 15.0;
/// Side of the small corner suit pip, in logical pixels.
const CORNER_PIP: i32 = 11;
/// Side of the big central pip on a number card.
const CENTER_PIP: i32 = 30;
/// Side of the faint suit pip shown on an empty foundation.
const HINT_PIP: i32 = 28;
const CENTER_SIZE: f32 = 34.0;

// Suit pips and the stock's recycle arrow, flattened from SVG at build time.
// `include_svg!` resolves paths against the crate root (CARGO_MANIFEST_DIR).
const CLUBS: SvgImage = include_svg!("assets/suits/clubs.svg");
const DIAMONDS: SvgImage = include_svg!("assets/suits/diamonds.svg");
const HEARTS: SvgImage = include_svg!("assets/suits/hearts.svg");
const SPADES: SvgImage = include_svg!("assets/suits/spades.svg");
const RECYCLE: SvgImage = include_svg!("assets/recycle.svg");

fn ink(color: CardColor) -> Color {
    match color {
        CardColor::Red => RED,
        CardColor::Black => BLACK,
    }
}

/// The vector pip for a suit (a single monochrome shape, tinted at draw time).
fn suit_pip(suit: Suit) -> SvgImage {
    match suit {
        Suit::Clubs => CLUBS,
        Suit::Diamonds => DIAMONDS,
        Suit::Hearts => HEARTS,
        Suit::Spades => SPADES,
    }
}

/// A `size`×`size` square centered inside `rect`.
fn centered_square(rect: Rect, size: i32) -> Rect {
    Rect::new(
        rect.x + (rect.w - size) / 2,
        rect.y + (rect.h - size) / 2,
        size,
        size,
    )
}

/// A 1-px border that skips the four corner pixels, so the card reads as
/// slightly rounded against the felt — the same trick the toolkit's button
/// chrome uses for its outer outline.
fn rounded_border(painter: &mut Painter, rect: Rect, color: Color) {
    if rect.w > 2 {
        painter.h_line(rect.x + 1, rect.y, rect.w - 2, color);
        painter.h_line(rect.x + 1, rect.bottom() - 1, rect.w - 2, color);
    }
    if rect.h > 2 {
        painter.v_line(rect.x, rect.y + 1, rect.h - 2, color);
        painter.v_line(rect.right() - 1, rect.y + 1, rect.h - 2, color);
    }
}

/// Draw a face-up card filling `rect`.
pub fn draw_face(painter: &mut Painter, rect: Rect, card: Card) {
    painter.fill_rect(rect.inset(1), FACE);
    rounded_border(painter, rect, BORDER);

    let color = ink(card.color());
    let rank = card.rank;
    let pip = suit_pip(card.suit);

    // Top-left index: rank, with its suit pip tucked just beneath it.
    let lx = rect.x + 5;
    painter.text(lx, rect.y + 3, rank.label(), RANK_SIZE, color);
    let tl_pip = Rect::new(lx, rect.y + 4 + RANK_SIZE as i32, CORNER_PIP, CORNER_PIP);
    painter.draw_svg_tinted(&pip, tl_pip, color);

    // Bottom-right index: the same block, right-aligned and upright (the board
    // reads top-down, so a rotated mirror would only add noise at this size).
    let rank_w = painter.measure_text(rank.label(), RANK_SIZE).w;
    let br_pip = Rect::new(
        rect.right() - 5 - CORNER_PIP,
        rect.bottom() - 4 - CORNER_PIP,
        CORNER_PIP,
        CORNER_PIP,
    );
    painter.draw_svg_tinted(&pip, br_pip, color);
    painter.text(
        rect.right() - 5 - rank_w,
        br_pip.y - 3 - RANK_SIZE as i32,
        rank.label(),
        RANK_SIZE,
        color,
    );

    // Centre: the big letter for court cards, the big pip otherwise.
    if rank.value() >= 11 {
        painter.text_centered(rect, rank.label(), CENTER_SIZE, color);
    } else {
        painter.draw_svg_tinted(&pip, centered_square(rect, CENTER_PIP), color);
    }
}

/// Draw a face-down card back filling `rect`.
pub fn draw_back(painter: &mut Painter, rect: Rect) {
    painter.fill_rect(rect.inset(1), BACK_BASE);
    // A small inset weave so the border reads as a margin around the pattern.
    let inner = rect.inset(5);
    if inner.w > 0 && inner.h > 0 {
        painter.fill_checker(inner, BACK_BASE, BACK_WEAVE);
    }
    rounded_border(painter, rect, BORDER);
}

/// Draw an empty pile placeholder over the felt. `hint` is an optional vector
/// mark drawn faintly in the centre — a suit pip on a foundation, a recycle
/// arrow on an exhausted stock.
pub fn draw_empty(painter: &mut Painter, rect: Rect, hint: Option<SvgImage>) {
    rounded_border(painter, rect, SLOT_LINE);
    // A second, inset outline gives the slot a subtle sunken depth.
    rounded_border(painter, rect.inset(1), SLOT_LINE);
    if let Some(mark) = hint {
        painter.draw_svg_tinted(&mark, centered_square(rect, HINT_PIP), SLOT_LINE);
    }
}

/// The faint suit pip shown on an empty foundation, so the player can see which
/// suit belongs where.
pub fn foundation_hint(index: usize) -> Option<SvgImage> {
    Suit::ALL.get(index).map(|&s| suit_pip(s))
}

/// Draw a highlight ring around a valid drop target.
pub fn draw_drop_hint(painter: &mut Painter, rect: Rect) {
    painter.stroke_rect(rect, DROP_HINT);
    painter.stroke_rect(rect.inset(1), DROP_HINT);
}

/// The "turn the waste back into the stock" arrow shown on an empty stock.
pub fn recycle_image() -> SvgImage {
    RECYCLE
}

#[cfg(test)]
mod tests {
    use crate::game::{Card, Rank, Suit};

    #[test]
    fn court_cards_render_a_letter_centre() {
        // A guard that the rank label drives the centre glyph for face cards.
        let king = Card {
            suit: Suit::Spades,
            rank: Rank::KING,
            face_up: true,
        };
        assert_eq!(king.rank.label(), "K");
        assert!(king.rank.value() >= 11);
    }
}
