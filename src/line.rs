use std::collections::HashMap;

pub type Slot = [usize; 3];
pub type Line = [Slot; 3];

// TODO: macro?
pub const LINES: [Line; 28] = [
    // Horizontal lines
    [[0, 0, 0], [1, 0, 0], [2, 0, 0]],
    [[0, 0, 0], [0, 1, 0], [0, 2, 0]],
    [[2, 0, 0], [2, 1, 0], [2, 2, 0]],
    [[0, 2, 0], [1, 2, 0], [2, 2, 0]],
    [[0, 0, 1], [1, 0, 1], [2, 0, 1]],
    [[0, 0, 1], [0, 1, 1], [0, 2, 1]],
    [[2, 0, 1], [2, 1, 1], [2, 2, 1]],
    [[0, 2, 1], [1, 2, 1], [2, 2, 1]],
    [[0, 0, 2], [1, 0, 2], [2, 0, 2]],
    [[0, 0, 2], [0, 1, 2], [0, 2, 2]],
    [[2, 0, 2], [2, 1, 2], [2, 2, 2]],
    [[0, 2, 2], [1, 2, 2], [2, 2, 2]],
    // Vertical lines
    [[0, 0, 0], [0, 0, 1], [0, 0, 2]],
    [[1, 0, 0], [1, 0, 1], [1, 0, 2]],
    [[2, 0, 0], [2, 0, 1], [2, 0, 2]],
    [[0, 1, 0], [0, 1, 1], [0, 1, 2]],
    [[2, 1, 0], [2, 1, 1], [2, 1, 2]],
    [[0, 2, 0], [0, 2, 1], [0, 2, 2]],
    [[1, 2, 0], [1, 2, 1], [1, 2, 2]],
    [[2, 2, 0], [2, 2, 1], [2, 2, 2]],
    // Diagonal lines
    [[0, 0, 0], [0, 1, 1], [0, 2, 2]],
    [[0, 0, 0], [1, 0, 1], [2, 0, 2]],
    [[0, 2, 0], [0, 1, 1], [0, 0, 2]],
    [[0, 2, 0], [1, 2, 1], [2, 2, 2]],
    [[2, 0, 0], [2, 1, 1], [2, 2, 2]],
    [[2, 0, 0], [1, 0, 1], [0, 0, 2]],
    [[2, 2, 0], [2, 1, 1], [2, 0, 2]],
    [[2, 2, 0], [1, 2, 1], [0, 2, 2]],
];

pub fn slot_to_lines() -> HashMap<Slot, Vec<Line>> {
    let mut slot_to_lines: HashMap<Slot, Vec<Line>> = HashMap::new();

    for line in LINES.iter() {
        for &slot in line {
            slot_to_lines.entry(slot).or_default().push(*line);
        }
    }

    slot_to_lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cage::Cage, cubie::Cubie};

    #[test]
    fn test_draw_lines() {
        for line in LINES.iter() {
            println!("Line {:?}:", line);

            let mut cage = Cage::new();
            for &point in line {
                cage.grid[point[0]][point[1]][point[2]] = Some(Cubie::Red);
            }
            cage.draw();
            println!();
        }
    }

    #[test]
    fn test_slot_to_lines() {
        let slot_to_lines = slot_to_lines();
        let lines_from_0 = slot_to_lines.get(&[0, 0, 0]).unwrap();
        assert_eq!(
            lines_from_0,
            &vec![
                [[0, 0, 0], [1, 0, 0], [2, 0, 0]],
                [[0, 0, 0], [0, 1, 0], [0, 2, 0]],
                [[0, 0, 0], [0, 0, 1], [0, 0, 2]],
                [[0, 0, 0], [0, 1, 1], [0, 2, 2]],
                [[0, 0, 0], [1, 0, 1], [2, 0, 2]]
            ]
        )
    }
}
