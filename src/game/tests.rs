//! Unit tests for the rules engine.

use super::*;

/// Count every card across every pile — a full game must always hold 52.
fn total_cards(game: &Game) -> usize {
    let mut n = game.stock().len() + game.waste().len();
    for i in 0..4 {
        n += game.foundation(i).len();
    }
    for i in 0..TABLEAU_PILES {
        n += game.tableau(i).len();
    }
    n
}

#[test]
fn deal_lays_out_klondike_correctly() {
    let game = Game::new(12345, DrawMode::One);
    assert_eq!(total_cards(&game), 52);

    // Pile i holds i + 1 cards; only its top is face up, the rest face down.
    for i in 0..TABLEAU_PILES {
        let pile = game.tableau(i);
        assert_eq!(pile.len(), i + 1, "tableau {i} size");
        assert!(pile.last().unwrap().face_up, "tableau {i} top is face up");
        assert!(
            pile[..pile.len() - 1].iter().all(|c| !c.face_up),
            "tableau {i} body is face down"
        );
    }

    // 52 - (1+2+..+7) = 52 - 28 = 24 cards in the stock, all face down.
    assert_eq!(game.stock().len(), 24);
    assert!(game.stock().iter().all(|c| !c.face_up));
    assert!(game.waste().is_empty());
    assert!(!game.is_won());
}

#[test]
fn deal_is_deterministic_per_seed() {
    let a = Game::new(99, DrawMode::Three);
    let b = Game::new(99, DrawMode::Three);
    for i in 0..TABLEAU_PILES {
        assert_eq!(a.tableau(i), b.tableau(i));
    }
    assert_eq!(a.stock(), b.stock());
    // A different seed should (essentially always) deal a different board.
    let c = Game::new(100, DrawMode::Three);
    assert_ne!(a.stock(), c.stock());
}

#[test]
fn draw_turns_cards_to_the_waste_and_recycles() {
    let mut game = Game::new(1, DrawMode::Three);
    let stock0 = game.stock().len();
    assert!(game.draw());
    assert_eq!(game.waste().len(), 3);
    assert_eq!(game.stock().len(), stock0 - 3);
    assert!(game.waste().iter().all(|c| c.face_up));

    // Exhaust the stock, then one more draw recycles the waste face down.
    while !game.stock().is_empty() {
        game.draw();
    }
    let waste_len = game.waste().len();
    assert!(waste_len > 0);
    assert!(game.draw(), "an empty stock recycles the waste");
    assert_eq!(game.stock().len(), waste_len);
    assert!(game.waste().is_empty());
    assert!(game.stock().iter().all(|c| !c.face_up));
}

#[test]
fn draw_one_mode_turns_a_single_card() {
    let mut game = Game::new(1, DrawMode::One);
    game.draw();
    assert_eq!(game.waste().len(), 1);
}

#[test]
fn foundation_builds_up_in_suit_from_the_ace() {
    let mut game = Game::demo_deal(DrawMode::One);
    // Clubs foundation is at 10; the J♣ sits on tableau 0's top.
    assert!(game.auto_to_foundation(Pile::Tableau(0)));
    assert_eq!(game.foundation(Suit::Clubs.index()).len(), 11);
    // The queen underneath was exposed and is now sendable too.
    assert!(game.auto_to_foundation(Pile::Tableau(0)));
    assert_eq!(game.foundation(Suit::Clubs.index()).len(), 12);
}

#[test]
fn tableau_builds_down_in_alternating_colour() {
    let mut game = Game::demo_deal(DrawMode::One);
    // Send J♥ up so Q♥ (red) is exposed on tableau 2, then drop it on K♣
    // (black) heading tableau 4: a legal alternating-colour build.
    assert!(game.auto_to_foundation(Pile::Tableau(2)));
    assert!(game.can_move(Pile::Tableau(2), 1, Pile::Tableau(4)));
    assert!(game.try_move(Pile::Tableau(2), 1, Pile::Tableau(4)));
    assert_eq!(game.tableau(4).len(), 2);
    assert_eq!(game.top(Pile::Tableau(4)).unwrap().rank, Rank::new(12));

    // A same-colour drop is rejected: expose Q♦ (red) and try it on K♦ (red).
    assert!(game.auto_to_foundation(Pile::Tableau(1)));
    assert!(!game.can_move(Pile::Tableau(1), 1, Pile::Tableau(5)));
}

#[test]
fn empty_column_only_accepts_a_king() {
    let mut game = Game::demo_deal(DrawMode::One);
    // Clear tableau 0 by sending its J then Q to the clubs foundation.
    game.auto_to_foundation(Pile::Tableau(0));
    game.auto_to_foundation(Pile::Tableau(0));
    assert!(game.tableau(0).is_empty());
    // A queen can't head an empty column…
    assert!(!game.can_move(Pile::Tableau(1), 1, Pile::Tableau(0)));
    // …but the K♥ on tableau 6 can.
    assert!(game.can_move(Pile::Tableau(6), 1, Pile::Tableau(0)));
}

#[test]
fn undo_restores_the_previous_layout() {
    let mut game = Game::new(5, DrawMode::One);
    let before_stock = game.stock().to_vec();
    assert!(!game.can_undo());
    game.draw();
    assert!(game.can_undo());
    assert_ne!(game.stock(), before_stock.as_slice());
    assert!(game.undo());
    assert_eq!(game.stock(), before_stock.as_slice());
    assert!(!game.can_undo());
    assert!(!game.undo(), "nothing left to undo");
}

#[test]
fn illegal_moves_do_not_touch_history() {
    let mut game = Game::demo_deal(DrawMode::One);
    // Q♠ on K♣ is illegal (same colour); it must not record a checkpoint.
    assert!(!game.try_move(Pile::Tableau(3), 1, Pile::Tableau(4)));
    assert!(!game.can_undo());
}

#[test]
fn auto_finish_wins_from_the_demo_deal() {
    let mut game = Game::demo_deal(DrawMode::One);
    assert!(game.can_auto_finish());
    assert!(game.auto_finish());
    assert!(game.is_won());
    assert_eq!(total_cards(&game), 52);
    // The whole cascade collapses to a single undo step.
    assert!(game.undo());
    assert!(!game.is_won());
    assert!(!game.can_undo());
}

#[test]
fn step_to_foundation_advances_one_card_at_a_time() {
    let mut game = Game::demo_deal(DrawMode::One);
    let mut steps = 0;
    while game.step_to_foundation() {
        steps += 1;
        assert!(steps <= 52, "auto-finish must terminate");
    }
    assert!(game.is_won());
    // Twelve cards were outstanding (four each of J, Q, K).
    assert_eq!(steps, 12);
}

#[test]
fn demo_deal_holds_a_full_deck() {
    let game = Game::demo_deal(DrawMode::Three);
    assert_eq!(total_cards(&game), 52);
}
