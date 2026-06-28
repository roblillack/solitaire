//! Playing-card primitives: suits, ranks, and a single card.
//!
//! These are deliberately tiny value types with no rendering or game logic —
//! the [`Game`](crate::game::Game) builds on them and the `ui` layer draws
//! them. The suits carry no glyph here: the renderer paints each pip from a
//! compile-time-baked SVG (see `ui::card_art`), so this layer stays free of any
//! toolkit or font dependency.

/// The two card colours. Klondike's tableau is built in alternating colours,
/// so this is what the move rules actually compare — not the suit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Red,
    Black,
}

/// The four French suits, ordered clubs–diamonds–hearts–spades. The ordinal
/// doubles as the foundation index, so a card always knows which of the four
/// foundation piles it belongs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    /// All four suits in foundation order.
    pub const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];

    /// 0-based index, matching the position in [`Suit::ALL`] and the foundation
    /// pile this suit lands on.
    pub fn index(self) -> usize {
        match self {
            Suit::Clubs => 0,
            Suit::Diamonds => 1,
            Suit::Hearts => 2,
            Suit::Spades => 3,
        }
    }

    pub fn color(self) -> Color {
        match self {
            Suit::Diamonds | Suit::Hearts => Color::Red,
            Suit::Clubs | Suit::Spades => Color::Black,
        }
    }
}

/// A card rank from Ace (1) through King (13).
///
/// Stored as a 1-based value so the arithmetic in the move rules ("the next
/// rank up") reads naturally. Construction is clamped to the valid range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rank(u8);

impl Rank {
    pub const ACE: Rank = Rank(1);
    pub const KING: Rank = Rank(13);

    /// Build a rank, clamping out-of-range values into `[1, 13]`.
    pub fn new(value: u8) -> Rank {
        Rank(value.clamp(1, 13))
    }

    /// The raw 1..=13 value.
    pub fn value(self) -> u8 {
        self.0
    }

    /// `true` if `self` is exactly one rank above `other` (e.g. 7 over 6) —
    /// the relationship a foundation build (ascending) and a tableau build
    /// (descending) are both phrased in terms of.
    pub fn is_one_above(self, other: Rank) -> bool {
        self.0 == other.0 + 1
    }

    /// Short label as shown on the card corner: `A`, `2`…`10`, `J`, `Q`, `K`.
    pub fn label(self) -> &'static str {
        match self.0 {
            1 => "A",
            2 => "2",
            3 => "3",
            4 => "4",
            5 => "5",
            6 => "6",
            7 => "7",
            8 => "8",
            9 => "9",
            10 => "10",
            11 => "J",
            12 => "Q",
            13 => "K",
            _ => "?",
        }
    }
}

/// A single playing card: its suit, rank, and whether it is currently shown
/// face up (face-down cards in the stock and the buried tableau rows render as
/// a card back).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
    pub face_up: bool,
}

impl Card {
    /// A face-down card of the given suit and rank.
    pub fn new(suit: Suit, rank: Rank) -> Card {
        Card {
            suit,
            rank,
            face_up: false,
        }
    }

    pub fn color(self) -> Color {
        self.suit.color()
    }

    /// A fresh, ordered 52-card deck, every card face down.
    pub fn full_deck() -> Vec<Card> {
        let mut deck = Vec::with_capacity(52);
        for suit in Suit::ALL {
            for value in 1..=13 {
                deck.push(Card::new(suit, Rank::new(value)));
            }
        }
        deck
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_deck_has_52_unique_cards() {
        let deck = Card::full_deck();
        assert_eq!(deck.len(), 52);
        let mut seen = std::collections::HashSet::new();
        for card in &deck {
            assert!(
                seen.insert((card.suit, card.rank.value())),
                "duplicate card {card:?}"
            );
        }
    }

    #[test]
    fn colors_and_one_above() {
        assert_eq!(Suit::Hearts.color(), Color::Red);
        assert_eq!(Suit::Spades.color(), Color::Black);
        assert!(Rank::new(7).is_one_above(Rank::new(6)));
        assert!(!Rank::new(7).is_one_above(Rank::new(5)));
    }

    #[test]
    fn rank_is_clamped() {
        assert_eq!(Rank::new(0), Rank::ACE);
        assert_eq!(Rank::new(99), Rank::KING);
    }
}
