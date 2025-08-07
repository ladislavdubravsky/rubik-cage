#![allow(dead_code)]
use crate::core::{cage::Cage, cubie::Cubie};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Rotation {
    Clockwise,
    CounterClockwise,
    HalfTurn,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Layer {
    Down,
    Equator,
    Up,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Move {
    Drop {
        color: Cubie,
        column: (usize, usize),
    }, // (x, y) coordinates
    RotateLayer {
        layer: Layer,
        rotation: Rotation,
    },
    Flip,
}

impl Move {
    pub fn inverse(self) -> Option<Self> {
        match self {
            Move::Drop { .. } => None, // Dropping a cubie cannot be inverted
            Move::RotateLayer { layer, rotation } => {
                let inverse_rotation = match rotation {
                    Rotation::Clockwise => Rotation::CounterClockwise,
                    Rotation::CounterClockwise => Rotation::Clockwise,
                    Rotation::HalfTurn => Rotation::HalfTurn,
                };
                Some(Move::RotateLayer {
                    layer,
                    rotation: inverse_rotation,
                })
            }
            Move::Flip => Some(Move::Flip),
        }
    }
}

impl Cage {
    pub fn is_center(x: usize, y: usize) -> bool {
        x == 1 && y == 1
    }

    pub fn drop(&mut self, color: Cubie, (x, y): (usize, usize)) -> Result<usize, &'static str> {
        if Self::is_center(x, y) {
            return Err("Cannot drop into the center column");
        }
        if x >= 3 || y >= 3 {
            return Err("Invalid column coordinates");
        }

        // Find lowest empty slot in column (x, y)
        for z in 0..3 {
            if self.grid[x][y][z].is_none() {
                self.grid[x][y][z] = Some(color);
                return Ok(z);
            }
        }

        Err("Column is full")
    }

    pub fn apply_gravity(&mut self) {
        for x in 0..3 {
            for y in 0..3 {
                if Self::is_center(x, y) {
                    continue;
                }

                let cubies: Vec<Cubie> = (0..3).filter_map(|z| self.grid[x][y][z]).collect();

                // Refill column from bottom up
                for z in 0..3 {
                    self.grid[x][y][z] = cubies.get(z).copied();
                }
            }
        }
    }

    pub fn rotate_layer(&mut self, layer: Layer, rotation: Rotation) {
        let z = match layer {
            Layer::Down => 0,
            Layer::Equator => 1,
            Layer::Up => 2,
        };

        let mut rotated = [[None; 3]; 3];

        for x in 0..3 {
            for y in 0..3 {
                let val = self.grid[x][y][z];
                let (rx, ry) = match rotation {
                    Rotation::Clockwise => (2 - y, x),
                    Rotation::CounterClockwise => (y, 2 - x),
                    Rotation::HalfTurn => (2 - x, 2 - y),
                };
                rotated[rx][ry] = val;
            }
        }

        for x in 0..3 {
            for y in 0..3 {
                self.grid[x][y][z] = rotated[x][y];
            }
        }

        // Apply gravity after rotation
        self.apply_gravity();
    }

    pub fn flip(&mut self) {
        let mut new_grid = [[[None; 3]; 3]; 3];

        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    if Self::is_center(x, y) {
                        continue;
                    }
                    let new_y = 2 - y;
                    let new_z = 2 - z;
                    new_grid[x][new_y][new_z] = self.grid[x][y][z];
                }
            }
        }

        self.grid = new_grid;
        self.apply_gravity();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_drop_cubie_basic() {
        let mut cage = Cage::new();
        assert!(cage.drop(Cubie::Red, (0, 1)).is_ok());
        cage.draw();
    }

    #[test]
    fn test_drop_cubie_full_column() {
        let mut cage = Cage::new();
        let col = (1, 2);
        assert!(cage.drop(Cubie::Red, col).is_ok());
        assert!(cage.drop(Cubie::Green, col).is_ok());
        assert!(cage.drop(Cubie::Blue, col).is_ok());
        assert!(cage.drop(Cubie::Yellow, col).is_err());
        cage.draw();
    }

    #[test]
    fn test_rotate_layer() {
        let mut cage = Cage::from_str("O........,W........,Y........").unwrap();
        cage.rotate_layer(Layer::Down, Rotation::Clockwise);

        let expected = Cage::from_str(".........,O........,W.Y......").unwrap();
        assert_eq!(cage, expected);
        cage.draw();
    }

    #[test]
    fn test_flip() {
        let mut cage = Cage::from_str("R........,GB.......,YWO......").unwrap();
        cage.draw();
        cage.flip();

        let expected = Cage::from_str("......Y..,......GW.,......RBO").unwrap();
        assert_eq!(cage, expected);

        println!("After flip:");
        cage.draw();
    }
}
