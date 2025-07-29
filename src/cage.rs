use crate::{
    cubie::Cubie,
    line::{Line, slot_to_lines},
};
use std::{collections::HashMap, str::FromStr};

#[derive(Debug, PartialEq)]
pub struct Cage {
    pub grid: [[[Option<Cubie>; 3]; 3]; 3],
}

impl Cage {
    pub fn new() -> Self {
        Self {
            grid: [[[None; 3]; 3]; 3],
        }
    }

    pub fn draw(&self) {
        for z in (0..3).rev() {
            for y in 0..3 {
                print!("{}", " ".repeat(3 - y));
                for x in 0..3 {
                    let cubie = self.grid[x][y][z];
                    match cubie {
                        Some(c) => c.draw(),
                        None => print!("{}", "▯"),
                    };
                }
                println!();
            }
        }
    }

    /// Checks if there are 3 same color cubies in a row, column, or diagonal. Center column is not
    /// available!
    pub fn has_line(&self) -> Option<Cubie> {
        let slot_to_lines = slot_to_lines();
        // TODO: use incrementally
        let mut counts: HashMap<(Line, Cubie), u8> = HashMap::new();

        for (slot, lines) in slot_to_lines {
            if let Some(cubie) = self.grid[slot[0]][slot[1]][slot[2]] {
                for &line in lines {
                    *counts.entry((line, cubie)).or_insert(0) += 1;
                    if counts[&(line, cubie)] == 3 {
                        return Some(cubie); // Found a line with 3 same color cubies
                    }
                }
            }
        }

        None // No lines found
    }
}

impl FromStr for Cage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut cage = Cage::new();
        let mut idx = 0;

        for ch in s.chars() {
            if ch.is_whitespace() || ch == ',' {
                continue; // Skip human-readable formatting
            }

            let x = idx % 3;
            let y = (idx / 3) % 3;
            let z = 2 - (idx / 9); // The cubies are input top (z=2) to bottom (z=0)

            if (x, y) == (1, 1) {
                // Center column is unavailable on the puzzle
                idx += 1;
                continue;
            }

            let cubie = match ch {
                '.' => None,
                other => Some(Cubie::from_char(other)?),
            };

            cage.grid[x][y][z] = cubie;
            idx += 1;
        }

        if idx != 27 {
            return Err(format!(
                "Expected 27 non-whitespace, non-comma characters, got {}",
                idx
            ));
        }

        Ok(cage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rustfmt::skip]
    #[test]
    fn test_line_detection() {
        assert!(Cage::from_str("R........,R........,R........").unwrap().has_line().is_some());
        assert!(Cage::from_str(".........,....Y....,......BBB").unwrap().has_line().is_some());
        assert!(Cage::from_str(".O......O,W.GWG.W..,R..Y....Y").unwrap().has_line().is_some());
        assert!(Cage::from_str("R........,...R.....,......R..").unwrap().has_line().is_some());
        assert!(Cage::from_str("..O......,.....O...,........O").unwrap().has_line().is_some());
        assert!(Cage::from_str("R........,O........,R........").unwrap().has_line().is_none());

        let cage_full = Cage::from_str("WYRBOGGOB,OGBYRYWBG,ROWBYGOWB").unwrap();
        assert!(cage_full.has_line().is_none());
        cage_full.draw();
    }
}
