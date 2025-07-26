use crate::cubie::Cubie;
use std::str::FromStr;

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
