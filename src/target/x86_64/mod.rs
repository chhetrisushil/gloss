//! The x86-64 target: typed instructions, their text and their bytes.

pub mod emit;
pub mod encode;
mod encode_alu;
mod encode_flow;
#[cfg(test)]
mod encode_flow_tests;
#[cfg(test)]
mod encode_tests;
pub mod inst;
pub mod linux;
pub mod lower;
#[cfg(test)]
mod lower_tests;
#[cfg(test)]
mod tests;
