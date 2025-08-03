mod cage;
mod cubie;
mod game;
mod line;
mod r#move;
mod search;
mod zobrist;

use cage::Cage;
use std::str::FromStr;

fn main() {
    let cage = Cage::from_str("R........,GB.......,YOW.....B").unwrap();
    cage.draw();
    println!("Someone won?: {}", cage.has_line().is_some());
}
