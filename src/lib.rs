pub mod concurrent;
pub mod non_concurrent;

#[allow(non_snake_case)]
pub const fn ALWAYS_UPDATE<T>(_: &T, _: &T) -> bool {true}

#[allow(non_snake_case)]
pub const fn NEVER_UPDATE<T>(_: &T, _: &T) -> bool {false}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum InsertOrUpdateResult{
    Inserted,
    Updated,
    Neither
}

const fn get_index<const N: usize>(key: [u8; N], depth: usize) -> usize{
    ((key[depth/4] >> (6-((depth % 4) * 2))) & 0b00000011) as usize
}