use bincode::{Decode, Encode};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
pub enum Cubie {
    White,
    Yellow,
    Red,
    Orange,
    Blue,
    Green,
}

impl Cubie {
    pub fn from_char(c: char) -> Result<Self, String> {
        match c {
            'R' => Ok(Cubie::Red),
            'G' => Ok(Cubie::Green),
            'B' => Ok(Cubie::Blue),
            'Y' => Ok(Cubie::Yellow),
            'W' => Ok(Cubie::White),
            'O' => Ok(Cubie::Orange),
            _ => Err(format!("Invalid color char: {}", c)),
        }
    }

    pub fn draw(self) {
        match self {
            Cubie::Red => print!("{}", "▮".red()),
            Cubie::Green => print!("{}", "▮".green()),
            Cubie::Blue => print!("{}", "▮".blue()),
            Cubie::Yellow => print!("{}", "▮".yellow()),
            Cubie::White => print!("{}", "▮".white()),
            Cubie::Orange => print!("{}", "▮".magenta()),
        }
    }
}

impl std::fmt::Display for Cubie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Cubie::White => "White",
            Cubie::Yellow => "Yellow",
            Cubie::Red => "Red",
            Cubie::Orange => "Orange",
            Cubie::Blue => "Blue",
            Cubie::Green => "Green",
        };
        write!(f, "{}", s)
    }
}
