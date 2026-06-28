//! The Klondike Solitaire rules engine.
//!
//! This module is pure game logic with no dependency on the `ui` layer or on
//! Saudade — it owns the deal, the legal-move rules, undo, and win detection,
//! and nothing about how any of it is drawn. The UI reads the piles through the
//! accessors and asks for moves through the small, validated mutation API
//! ([`Game::draw`], [`Game::try_move`], [`Game::auto_to_foundation`],
//! [`Game::auto_finish`], [`Game::undo`]); every one of those takes a snapshot
//! first, so undo is a single uniform mechanism rather than per-move inverses.
//!
//! Klondike, briefly: 28 cards are dealt into seven tableau piles (pile *i*
//! holds *i + 1* cards, only the top face up); the remaining 24 form the face-
//! down stock. Cards are turned from the stock to the waste, built **down in
//! alternating colours** on the tableau, and **up in suit** (Ace first) on the
//! four foundations. Clear all 52 onto the foundations to win.

mod card;
mod rng;

pub use card::{Card, Color, Rank, Suit};
pub use rng::Rng;

/// Number of tableau piles in Klondike.
pub const TABLEAU_PILES: usize = 7;

/// Identifies one of the piles on the table. The two index-carrying variants
/// address a specific foundation (by suit index, 0..4) or tableau column
/// (0..7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pile {
    Stock,
    Waste,
    Foundation(usize),
    Tableau(usize),
}

/// How many cards a stock turn deals to the waste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawMode {
    /// Turn one card at a time — the easier, more forgiving game.
    One,
    /// Turn three at a time — the traditional, harder game.
    Three,
}

impl DrawMode {
    pub fn count(self) -> usize {
        match self {
            DrawMode::One => 1,
            DrawMode::Three => 3,
        }
    }

    pub fn toggled(self) -> DrawMode {
        match self {
            DrawMode::One => DrawMode::Three,
            DrawMode::Three => DrawMode::One,
        }
    }
}

/// The mutable card layout — everything an undo snapshot needs to capture.
/// Cloned wholesale onto the history stack before each move; at 52 small `Copy`
/// cards that is a handful of bytes, so the simplicity is free.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Layout {
    /// Face down; the last element is the top (next to be turned).
    stock: Vec<Card>,
    /// Face up; the last element is the top (the playable card).
    waste: Vec<Card>,
    /// One ascending pile per suit, indexed by [`Suit::index`].
    foundations: [Vec<Card>; 4],
    /// The seven columns; index 0 is the bottom of a column, the last its top.
    tableau: [Vec<Card>; TABLEAU_PILES],
}

/// A full Solitaire game: the card layout, the undo history, and the draw mode.
pub struct Game {
    layout: Layout,
    history: Vec<Layout>,
    draw_mode: DrawMode,
    seed: u64,
}

impl Game {
    /// Deal a fresh game from `seed`. The deal is a pure function of the seed,
    /// so the same seed always produces the same board.
    pub fn new(seed: u64, draw_mode: DrawMode) -> Game {
        let mut deck = Card::full_deck();
        Rng::seeded(seed).shuffle(&mut deck);

        let mut tableau: [Vec<Card>; TABLEAU_PILES] = Default::default();
        // Pile i gets i + 1 cards; only the last one dealt to it is face up.
        for (i, pile) in tableau.iter_mut().enumerate() {
            for row in 0..=i {
                let mut card = deck.pop().expect("28 cards for the tableau");
                card.face_up = row == i;
                pile.push(card);
            }
        }
        // The remaining 24 cards are the face-down stock.
        let stock = deck;

        Game {
            layout: Layout {
                stock,
                waste: Vec::new(),
                foundations: Default::default(),
                tableau,
            },
            history: Vec::new(),
            draw_mode,
            seed,
        }
    }

    // ----------------------------------------------------------- accessors

    pub fn stock(&self) -> &[Card] {
        &self.layout.stock
    }

    pub fn waste(&self) -> &[Card] {
        &self.layout.waste
    }

    pub fn foundation(&self, index: usize) -> &[Card] {
        &self.layout.foundations[index]
    }

    pub fn tableau(&self, index: usize) -> &[Card] {
        &self.layout.tableau[index]
    }

    pub fn draw_mode(&self) -> DrawMode {
        self.draw_mode
    }

    /// Change how many cards a stock turn deals. Affects subsequent draws only;
    /// the current board is left untouched.
    pub fn set_draw_mode(&mut self, mode: DrawMode) {
        self.draw_mode = mode;
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// `true` once every foundation holds a full Ace-through-King suit.
    pub fn is_won(&self) -> bool {
        self.layout.foundations.iter().all(|f| f.len() == 13)
    }

    /// `true` once the stock and waste are empty and every tableau card is face
    /// up — the point at which the rest of the game is a foregone conclusion
    /// and [`auto_finish`](Self::auto_finish) is guaranteed to win. The UI uses
    /// this to offer (or auto-trigger) the finishing flourish.
    pub fn can_auto_finish(&self) -> bool {
        // A single waste card is always on top (hence always reachable); with
        // the stock gone and every tableau card face up, greedily playing the
        // tops is then guaranteed to clear the board.
        !self.is_won()
            && self.layout.stock.is_empty()
            && self.layout.waste.len() <= 1
            && self
                .layout
                .tableau
                .iter()
                .all(|pile| pile.iter().all(|card| card.face_up))
    }

    /// Whether an undo is available.
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    // ------------------------------------------------------ pile inspection

    fn pile(&self, pile: Pile) -> &[Card] {
        match pile {
            Pile::Stock => &self.layout.stock,
            Pile::Waste => &self.layout.waste,
            Pile::Foundation(i) => &self.layout.foundations[i],
            Pile::Tableau(i) => &self.layout.tableau[i],
        }
    }

    fn pile_mut(&mut self, pile: Pile) -> &mut Vec<Card> {
        match pile {
            Pile::Stock => &mut self.layout.stock,
            Pile::Waste => &mut self.layout.waste,
            Pile::Foundation(i) => &mut self.layout.foundations[i],
            Pile::Tableau(i) => &mut self.layout.tableau[i],
        }
    }

    /// The top (visible / playable) card of a pile, if any.
    pub fn top(&self, pile: Pile) -> Option<Card> {
        self.pile(pile).last().copied()
    }

    /// `true` if the cards from `start` to the top of `pile` form a stack that
    /// can be picked up as one unit. The waste and foundations only ever lift
    /// their single top card; a tableau pile lifts any tail that is all face up
    /// and runs strictly down in alternating colours.
    pub fn is_movable_run(&self, pile: Pile, start: usize) -> bool {
        let cards = self.pile(pile);
        if start >= cards.len() {
            return false;
        }
        match pile {
            Pile::Stock => false,
            // Only the single top card of the waste or a foundation lifts.
            Pile::Waste | Pile::Foundation(_) => start == cards.len() - 1,
            Pile::Tableau(_) => {
                if !cards[start].face_up {
                    return false;
                }
                cards[start..].windows(2).all(|w| {
                    w[1].face_up
                        && w[0].color() != w[1].color()
                        && w[0].rank.is_one_above(w[1].rank)
                })
            }
        }
    }

    /// Whether `cards` (a validated pickup, bottom card first) may be dropped
    /// onto `dest`. Stock and waste are never drop targets.
    fn accepts(&self, dest: Pile, cards: &[Card]) -> bool {
        let Some(bottom) = cards.first().copied() else {
            return false;
        };
        match dest {
            Pile::Stock | Pile::Waste => false,
            Pile::Foundation(fi) => {
                // Foundations take one card at a time, of their own suit,
                // building up from the Ace.
                if cards.len() != 1 || bottom.suit.index() != fi {
                    return false;
                }
                match self.layout.foundations[fi].last() {
                    None => bottom.rank == Rank::ACE,
                    Some(top) => bottom.rank.is_one_above(top.rank),
                }
            }
            Pile::Tableau(ti) => match self.layout.tableau[ti].last() {
                // An empty column only accepts a King (heading a new run).
                None => bottom.rank == Rank::KING,
                Some(top) => {
                    top.face_up
                        && top.color() != bottom.color()
                        && top.rank.is_one_above(bottom.rank)
                }
            },
        }
    }

    /// `true` if moving the `count`-card tail of `from` onto `to` is legal,
    /// without performing it. Used by the UI to decide whether a drag should be
    /// accepted (and to highlight a valid drop target).
    pub fn can_move(&self, from: Pile, count: usize, to: Pile) -> bool {
        if from == to || count == 0 {
            return false;
        }
        let cards = self.pile(from);
        if count > cards.len() {
            return false;
        }
        let start = cards.len() - count;
        if !self.is_movable_run(from, start) {
            return false;
        }
        self.accepts(to, &cards[start..])
    }

    // --------------------------------------------------------- mutations

    /// Save the current layout so the next mutation can be undone.
    fn checkpoint(&mut self) {
        self.history.push(self.layout.clone());
    }

    /// Move the `count`-card tail of `from` onto `to` without flipping, undo, or
    /// validation — the shared core of every public move. Returns the layout to
    /// a consistent state by turning the newly exposed source card face up.
    fn relocate(&mut self, from: Pile, count: usize, to: Pile) {
        let src = self.pile_mut(from);
        let moved: Vec<Card> = src.split_off(src.len() - count);
        self.pile_mut(to).extend(moved);
        self.flip_exposed(from);
    }

    /// After cards leave a tableau column, turn its newly exposed top face up.
    fn flip_exposed(&mut self, pile: Pile) {
        if let Pile::Tableau(_) = pile
            && let Some(top) = self.pile_mut(pile).last_mut()
            && !top.face_up
        {
            top.face_up = true;
        }
    }

    /// Attempt the move `from`→`to` of its `count`-card tail. Records an undo
    /// checkpoint and performs the move only if it is legal; returns whether it
    /// happened.
    pub fn try_move(&mut self, from: Pile, count: usize, to: Pile) -> bool {
        if !self.can_move(from, count, to) {
            return false;
        }
        self.checkpoint();
        self.relocate(from, count, to);
        true
    }

    /// Send the top card of `from` to its matching foundation if it is legal —
    /// the gesture behind a double-click (or right-click) on a card. Returns
    /// whether a card moved.
    pub fn auto_to_foundation(&mut self, from: Pile) -> bool {
        let Some(card) = self.top(from) else {
            return false;
        };
        let dest = Pile::Foundation(card.suit.index());
        self.try_move(from, 1, dest)
    }

    /// Turn cards from the stock to the waste. With an empty stock this instead
    /// recycles the waste back into the stock (face down, ready to be turned
    /// again). Always recorded as one undoable step.
    pub fn draw(&mut self) -> bool {
        if self.layout.stock.is_empty() {
            if self.layout.waste.is_empty() {
                return false; // nothing to turn and nothing to recycle
            }
            self.checkpoint();
            // Recycle: the waste flips face down and reverses, so turning the
            // stock again replays it in the original order.
            let mut recycled: Vec<Card> = self.layout.waste.drain(..).rev().collect();
            for card in &mut recycled {
                card.face_up = false;
            }
            self.layout.stock = recycled;
            return true;
        }
        self.checkpoint();
        let n = self.draw_mode.count().min(self.layout.stock.len());
        for _ in 0..n {
            let mut card = self.layout.stock.pop().expect("stock not empty");
            card.face_up = true;
            self.layout.waste.push(card);
        }
        true
    }

    /// Play every card that can go to a foundation, repeatedly, as a single
    /// undoable step — the auto-finish flourish once the outcome is decided.
    /// Returns whether any card moved. Only the move itself is performed here;
    /// the caller drives any animation.
    pub fn auto_finish(&mut self) -> bool {
        // Snapshot once so the whole cascade undoes in one step.
        let before = self.layout.clone();
        let mut any = false;
        while let Some(step) = self.next_foundation_move() {
            self.relocate(step.0, 1, step.1);
            any = true;
        }
        if any {
            self.history.push(before);
        }
        any
    }

    /// Find one legal top-card→foundation move, scanning the waste and the
    /// tableau tops. Used to drive [`auto_finish`] one card at a time (and by
    /// the UI's per-tick finishing animation).
    pub fn next_foundation_move(&self) -> Option<(Pile, Pile)> {
        let sources = std::iter::once(Pile::Waste).chain((0..TABLEAU_PILES).map(Pile::Tableau));
        for from in sources {
            if let Some(card) = self.top(from) {
                let dest = Pile::Foundation(card.suit.index());
                if self.accepts(dest, &[card]) {
                    return Some((from, dest));
                }
            }
        }
        None
    }

    /// Perform a single foundation step previously reported by
    /// [`next_foundation_move`], recording it as one undoable move. Returns
    /// whether it happened. Lets the UI animate the auto-finish card by card
    /// while keeping each step on the undo stack.
    pub fn step_to_foundation(&mut self) -> bool {
        match self.next_foundation_move() {
            Some((from, to)) => self.try_move(from, 1, to),
            None => false,
        }
    }

    /// Undo the most recent move, if any. Returns whether anything was undone.
    pub fn undo(&mut self) -> bool {
        match self.history.pop() {
            Some(prev) => {
                self.layout = prev;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests;

// ----------------------------------------------------------- demo fixtures

impl Game {
    /// A deterministic board that is a few obvious moves from victory, used by
    /// the scripted demo and the win-state snapshot. Every foundation is built
    /// to the 10; the twelve face cards are laid out so they can all be sent
    /// home — including one (`K♠`) sitting on the waste — and one queen can be
    /// dragged onto a king first to show off the drag mechanic.
    pub fn demo_deal(draw_mode: DrawMode) -> Game {
        let up = |suit: Suit, rank: u8| Card {
            suit,
            rank: Rank::new(rank),
            face_up: true,
        };

        // Foundations: Ace..10 of every suit (40 cards).
        let mut foundations: [Vec<Card>; 4] = Default::default();
        for suit in Suit::ALL {
            foundations[suit.index()] = (1..=10).map(|r| up(suit, r)).collect();
        }

        // Tableau: the queens and jacks, jacks on top so they are immediately
        // foundation-ready; the four kings head their own short columns.
        let mut tableau: [Vec<Card>; TABLEAU_PILES] = Default::default();
        tableau[0] = vec![up(Suit::Clubs, 12), up(Suit::Clubs, 11)];
        tableau[1] = vec![up(Suit::Diamonds, 12), up(Suit::Diamonds, 11)];
        tableau[2] = vec![up(Suit::Hearts, 12), up(Suit::Hearts, 11)];
        tableau[3] = vec![up(Suit::Spades, 12), up(Suit::Spades, 11)];
        tableau[4] = vec![up(Suit::Clubs, 13)];
        tableau[5] = vec![up(Suit::Diamonds, 13)];
        tableau[6] = vec![up(Suit::Hearts, 13)];
        // The last king waits on the waste.
        let waste = vec![up(Suit::Spades, 13)];

        Game {
            layout: Layout {
                stock: Vec::new(),
                waste,
                foundations,
                tableau,
            },
            history: Vec::new(),
            draw_mode,
            seed: 0,
        }
    }
}
