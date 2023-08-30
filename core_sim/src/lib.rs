#![feature(variant_count)]

mod bin;
pub mod breakpoint;
pub mod common;
pub mod cpu;
pub mod debug_symbol;
pub mod instr;
pub mod io;
pub mod memory;
pub mod ppm;
pub mod reg_file;
pub mod register;
pub mod sim;
pub mod sld;
pub mod ty;

mod fpu_wrapper;
#[cfg(feature = "stat")]
pub mod stat;

#[cfg(feature = "stat")]
pub mod cache;

#[cfg(not(feature = "isa_2nd"))]
mod decode_instr;

#[cfg(feature = "isa_2nd")]
mod decode_instr_2nd;

#[cfg(feature = "time_predict")]
pub mod branch_predictor;