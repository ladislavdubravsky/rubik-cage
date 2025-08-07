use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::sync::LazyLock;

pub static POS_COLOR: LazyLock<[[[[u64; 3]; 3]; 3]; 6]> = LazyLock::new(|| {
    let mut rng = StdRng::seed_from_u64(0x12345678);

    let mut table = [[[[0; 3]; 3]; 3]; 6];
    for x in 0..3 {
        for y in 0..3 {
            for z in 0..3 {
                for color in 0..6 {
                    table[color][x][y][z] = rng.random::<u64>();
                }
            }
        }
    }

    table
});

pub static P2_TO_MOVE: LazyLock<u64> = LazyLock::new(|| {
    let mut rng = StdRng::seed_from_u64(0x87654321);
    rng.random::<u64>()
});
