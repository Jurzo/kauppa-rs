use serde::{Deserialize, Serialize};

/// One step up or down in a user-ordered list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Up,
    Down,
}

/// Swap `items[index]` with its neighbour in `direction`.
///
/// Returns `false` and leaves `items` untouched when `index` is out of range or
/// the move would go past either end.
pub fn step<T>(items: &mut [T], index: usize, direction: Direction) -> bool {
    if index >= items.len() {
        return false;
    }
    let neighbour = match direction {
        Direction::Up => index.checked_sub(1),
        Direction::Down => Some(index + 1).filter(|&j| j < items.len()),
    };
    let Some(neighbour) = neighbour else {
        return false;
    };
    items.swap(index, neighbour);
    true
}

/// Whether `proposed` is exactly `current` in some order: the same length,
/// every id once, none missing or foreign (MB-0066, RR-0053 sub-answer 4).
/// A full order is saved only when this holds.
#[must_use]
pub fn is_reordering<T: Ord>(current: &[T], proposed: &[T]) -> bool {
    if current.len() != proposed.len() {
        return false;
    }
    let mut a: Vec<&T> = current.iter().collect();
    let mut b: Vec<&T> = proposed.iter().collect();
    a.sort_unstable();
    b.sort_unstable();
    // `current` has no repeats (ids), so equal sorted lists also rule out a
    // repeat in `proposed`.
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_swaps_with_neighbour() {
        let mut items = ['a', 'b', 'c'];
        assert!(step(&mut items, 1, Direction::Up));
        assert_eq!(items, ['b', 'a', 'c']);
        assert!(step(&mut items, 1, Direction::Down));
        assert_eq!(items, ['b', 'c', 'a']);
    }

    #[test]
    fn step_is_a_noop_at_the_ends_and_out_of_range() {
        let mut items = ['a', 'b'];
        assert!(!step(&mut items, 0, Direction::Up));
        assert!(!step(&mut items, 1, Direction::Down));
        assert!(!step(&mut items, 2, Direction::Up));
        assert_eq!(items, ['a', 'b']);
    }

    #[test]
    fn is_reordering_needs_the_exact_set() {
        let current = ['a', 'b', 'c'];
        assert!(is_reordering(&current, &['c', 'a', 'b']));
        assert!(is_reordering(&current, &current));
        assert!(!is_reordering(&current, &['a', 'b']), "missing");
        assert!(!is_reordering(&current, &['a', 'b', 'c', 'd']), "extra");
        assert!(!is_reordering(&current, &['a', 'a', 'b']), "repeated");
        assert!(!is_reordering(&current, &['a', 'b', 'x']), "foreign");
        assert!(is_reordering::<char>(&[], &[]));
    }
}
