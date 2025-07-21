mod cage;
mod cubie;

use cage::Cage;
use std::str::FromStr;

fn main() {
    let cage = Cage::from_str("R........,GB.......,YOW.....B").unwrap();
    cage.draw();
}
