use crate::core::{
    cubie::Cubie,
    line::{LINES, Line},
};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(
    Clone, Copy, Debug, Hash, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
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
    pub fn has_line(&self) -> Option<(Cubie, Line)> {
        self.lines().next()
    }

    /// Deterministic line order; callers deciding an outcome must inspect both players.
    pub fn lines(&self) -> impl Iterator<Item = (Cubie, Line)> + '_ {
        LINES.iter().filter_map(|line| {
            let [x, y, z] = line[0];
            let color = self.grid[x][y][z]?;
            line.iter()
                .all(|&[x, y, z]| self.grid[x][y][z] == Some(color))
                .then_some((color, *line))
        })
    }

    /// This function is only used in normalizing the representation of the cage.
    fn flip_horizontal(&mut self) {
        self.grid.swap(0, 2);
    }

    /// This function is only used in normalizing the representation of the cage.
    fn rotate_cage(&mut self) {
        let mut new_grid = [[[None; 3]; 3]; 3];
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    new_grid[y][2 - x][z] = self.grid[x][y][z];
                }
            }
        }
        self.grid = new_grid;
    }

    /// Symmetric cages have the same game evaluation, normalize to reduce the search space.
    pub fn normalize(&mut self) -> bool {
        let mut largest = self.grid;

        let mut reflection_happened = false;
        for i in 0..2 {
            for _ in 0..4 {
                self.rotate_cage();
                if self.grid > largest {
                    largest = self.grid;
                    if i == 1 {
                        reflection_happened = true;
                    }
                }
            }
            self.flip_horizontal();
        }

        self.grid = largest;

        reflection_happened
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

            if idx >= 27 {
                return Err("Too many slots".into());
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

    #[test]
    fn test_flip_horizontal() {
        let mut cage = Cage::from_str("R........,G........,B........").unwrap();
        cage.draw();

        println!("Flipped:");
        cage.flip_horizontal();
        cage.draw();

        let flipped = Cage::from_str("..R......,..G......,..B......").unwrap();
        assert_eq!(cage, flipped);
    }

    #[test]
    fn test_rotate_cage() {
        let mut cage = Cage::from_str("R........,G........,BY.......").unwrap();
        cage.draw();

        println!("Rotated:");
        cage.rotate_cage();
        cage.draw();

        let rotated = Cage::from_str("......R..,......G..,...Y..B..").unwrap();
        assert_eq!(cage, rotated);
    }

    #[test]
    fn test_normalize() {
        // we expect lexicographically largest symmetry
        let mut cage = Cage::from_str("........R,........G,.......YB").unwrap();
        println!("Cage: {:?}", cage);
        cage.normalize();
        println!("Normalized: {:?}", cage);
        let normalized = Cage::from_str("R........,G........,B..Y.....").unwrap();
        assert_eq!(cage, normalized);
    }
}
