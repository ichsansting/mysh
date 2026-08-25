use crate::domain::drift::{Drift, DriftSide};
use std::path::PathBuf;

/// One selectable row in the picker — a `Drift` plus whether it's currently checked.
#[derive(Clone, Debug)]
pub struct Item {
    pub rel: PathBuf,
    pub side: DriftSide,
    pub selected: bool,
}

impl From<Drift> for Item {
    /// Everything starts checked — the picker narrows down, it never starts empty.
    fn from(drift: Drift) -> Self {
        Item {
            rel: drift.rel,
            side: drift.side,
            selected: true,
        }
    }
}

/// A key as the terminal layer interprets it — arrows are already decoded from
/// their escape sequence by the time they reach this module.
#[derive(Clone, Copy, Debug)]
pub enum Key {
    Up,
    Down,
    Enter,
    Char(char),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Confirmed,
    Aborted,
}

#[derive(Clone, Debug)]
pub struct State {
    pub items: Vec<Item>,
    pub cursor: usize,
    pub message: Option<String>,
    pub outcome: Option<Outcome>,
}

impl State {
    pub fn new(items: Vec<Item>) -> Self {
        State {
            items,
            cursor: 0,
            message: None,
            outcome: None,
        }
    }
}

/// The whole picker state machine: one keystroke in, next state out. Pure —
/// no I/O, no terminal codes. Validated interactively via
/// `examples/picker_prototype.rs` before being lifted in here.
pub fn apply(mut state: State, key: Key) -> State {
    state.message = None;
    match key {
        Key::Up => state.cursor = state.cursor.saturating_sub(1),
        Key::Down => {
            if state.cursor + 1 < state.items.len() {
                state.cursor += 1;
            }
        }
        Key::Enter => state.outcome = Some(Outcome::Confirmed),
        Key::Char(' ') => {
            let i = state.cursor;
            state.items[i].selected = !state.items[i].selected;
        }
        Key::Char('q') => state.outcome = Some(Outcome::Aborted),
        Key::Char('a') => {
            for item in &mut state.items {
                item.selected = true;
            }
            state.message = Some("all selected".to_string());
        }
        Key::Char('0') => {
            for item in &mut state.items {
                item.selected = false;
            }
            state.message = Some("none selected".to_string());
        }
        Key::Char(other) => {
            state.message = Some(format!("unrecognized key: '{other}'"));
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(n: usize) -> Vec<Item> {
        (0..n)
            .map(|i| Item {
                rel: PathBuf::from(format!("f{i}")),
                side: DriftSide::Target,
                selected: true,
            })
            .collect()
    }

    #[test]
    fn starts_with_everything_selected_and_cursor_at_zero() {
        let state = State::new(items(3));
        assert!(state.items.iter().all(|i| i.selected));
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn space_toggles_only_the_item_under_the_cursor() {
        let mut state = State::new(items(3));
        state = apply(state, Key::Down);
        state = apply(state, Key::Char(' '));
        assert_eq!(
            state.items.iter().map(|i| i.selected).collect::<Vec<_>>(),
            vec![true, false, true]
        );
    }

    #[test]
    fn cursor_clamps_at_both_ends() {
        let mut state = State::new(items(2));
        state = apply(state, Key::Up); // already at 0, stays
        assert_eq!(state.cursor, 0);
        state = apply(state, Key::Down);
        state = apply(state, Key::Down); // already at last, stays
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn select_all_and_select_none_affect_every_item() {
        let mut state = State::new(items(3));
        state = apply(state, Key::Char('0'));
        assert!(state.items.iter().all(|i| !i.selected));
        state = apply(state, Key::Char('a'));
        assert!(state.items.iter().all(|i| i.selected));
    }

    #[test]
    fn enter_confirms_without_changing_selection() {
        let mut state = State::new(items(2));
        state = apply(state, Key::Char(' ')); // deselect item 0
        state = apply(state, Key::Enter);
        assert_eq!(state.outcome, Some(Outcome::Confirmed));
        assert!(!state.items[0].selected);
        assert!(state.items[1].selected);
    }

    #[test]
    fn q_aborts() {
        let state = apply(State::new(items(1)), Key::Char('q'));
        assert_eq!(state.outcome, Some(Outcome::Aborted));
    }

    #[test]
    fn unrecognized_key_sets_a_message_and_changes_nothing_else() {
        let mut state = State::new(items(2));
        state = apply(state, Key::Char('z'));
        assert!(state.outcome.is_none());
        assert!(state.items.iter().all(|i| i.selected));
        assert_eq!(state.message.as_deref(), Some("unrecognized key: 'z'"));
    }
}
