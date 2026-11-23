//! Opt-in (`GLOSS_OBJDUMP=1`): every instruction the encoder can make is disassembled by
//! GNU objdump and compared with our own text, byte for byte and operand for operand.

#[path = "objdump/cmp.rs"]
mod cmp;
