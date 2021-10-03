use rand_chacha::ChaCha8Rng;

pub trait JsRandom {
    fn new() -> Self;
}

impl JsRandom for ChaCha8Rng {
    fn new() -> Self {
        use rand::rngs::OsRng;
        use rand::{RngCore, SeedableRng};

        let mut seed = [0; 32];
        OsRng.fill_bytes(&mut seed);
        ChaCha8Rng::from_seed(seed)
    }
}
