mod cage;
mod cubie;
mod r#move;

use cage::Cage;
use std::str::FromStr;

fn main() {
    let cage = Cage::from_str("R........,GB.......,YOW.....B").unwrap();
    cage.draw();
}
