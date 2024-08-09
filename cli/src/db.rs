use blake2::{
    digest::{Update, VariableOutput},
    Blake2bVar,
};
use parity_scale_codec::{Decode, Encode};

const DB_LOCATION: &str = "./db";
const HASH_SIZE: usize = 2;

#[derive(Encode, Decode, Debug)]
pub struct TestState {}

pub fn hash(words: &[&str]) -> String {
    let phrase = words.join("");

    let mut hasher = Blake2bVar::new(HASH_SIZE).unwrap();
    let mut hash = [0; HASH_SIZE];

    hasher.update(phrase.as_bytes());
    hasher.finalize_variable(&mut hash).unwrap();

    hex::encode(hash)
}
