//! The x86-64 target: typed instructions, their text and their bytes.

pub mod emit;
pub mod encode;
mod encode_alu;
#[cfg(test)]
mod encode_tests;
pub mod inst;
#[cfg(test)]
mod tests;
