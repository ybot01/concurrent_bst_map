mod non_recursive;
mod recursive;
pub mod experiment;

#[allow(non_snake_case)]
pub const fn ALWAYS_UPDATE<T>(_: &T, _: &T) -> bool {true}

#[allow(non_snake_case)]
pub const fn NEVER_UPDATE<T>(_: &T, _: &T) -> bool {false}