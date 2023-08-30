use std::{collections::HashMap, fmt::Display};

use once_cell::sync::Lazy;

use crate::ty::Ty;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct RegId(u8);

impl RegId {
    pub fn inner(&self) -> usize {
        self.0 as usize
    }
    pub fn ty(&self) -> Ty {
        if self.0 == 1 {
            Ty::Usize
        } else {
            Ty::I32
        }
    }
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct FRegId(u8);

impl FRegId {
    pub fn inner(&self) -> usize {
        self.0 as usize
    }
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

#[cfg(not(feature = "isa_2nd"))]
pub static ABINAME_TABLE: [&str; MAX_REG_ID] = [
    "zero", "ra", "sp", "gp", "hp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
    "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
    "t5", "t6",
];

#[cfg(feature = "isa_2nd")]
pub static ABINAME_TABLE: [&str; MAX_REG_ID] = [
    "zero", "ra", "sp", "gp", "hp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
    "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
    "t5", "t6", "x32", "x33", "x34", "x35", "x36", "x37", "x38", "x39", "x40", "x41", "x42", "x43",
    "x44", "x45", "x46", "x47", "x48", "x49", "x50", "x51", "x52", "x53", "x54", "x55", "x56",
    "x57", "x58", "x59", "x60", "x61", "x62", "x63",
];

pub static ABINAME_LOOKUP: Lazy<HashMap<&str, RegId>> = Lazy::new(|| {
    ABINAME_TABLE
        .iter()
        .enumerate()
        .map(|(i, n)| (*n, RegId(i as u8)))
        .collect()
});

impl Display for RegId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = ABINAME_TABLE[self.0 as usize];
        f.write_str(s)
    }
}

pub const MAX_REG_ID: usize = 1 << REG_BIT_WIDTH;
pub const REG_BIT_WIDTH: u32 = if cfg!(feature = "isa_2nd") { 6 } else { 5 };

impl TryFrom<u32> for RegId {
    type Error = anyhow::Error;
    fn try_from(rs: u32) -> Result<Self, Self::Error> {
        #[cfg(debug_assertions)]
        if rs >= (1 << (REG_BIT_WIDTH + 1)) {
            return Err(anyhow::anyhow!(
                "register should be {REG_BIT_WIDTH}-bit integer, found {}",
                rs
            ));
        }
        Ok(Self(rs as u8))
    }
}

impl TryFrom<&str> for RegId {
    type Error = ();
    fn try_from(rs: &str) -> Result<Self, Self::Error> {
        ABINAME_LOOKUP.get(rs).cloned().ok_or(())
    }
}

#[cfg(not(feature = "isa_2nd"))]
pub static F_ABINAME_TABLE: [&str; MAX_REG_ID] = [
    "fzero", "fone", "ft2", "ft3", "ft4", "ft5", "ft6", "ft7", "fs0", "fs1", "fa0", "fa1", "fa2",
    "fa3", "fa4", "fa5", "fa6", "fa7", "fs2", "fs3", "fs4", "fs5", "fs6", "fs7", "fs8", "fs9",
    "fs10", "fs11", "ft8", "ft9", "ft10", "ft11",
];

#[cfg(feature = "isa_2nd")]
pub static F_ABINAME_TABLE: [&str; MAX_REG_ID] = [
    "fzero", "fone", "ft2", "ft3", "ft4", "ft5", "ft6", "ft7", "fs0", "fs1", "fa0", "fa1", "fa2",
    "fa3", "fa4", "fa5", "fa6", "fa7", "fs2", "fs3", "fs4", "fs5", "fs6", "fs7", "fs8", "fs9",
    "fs10", "fs11", "ft8", "ft9", "ft10", "ft11", "f32", "f33", "f34", "f35", "f36", "f37", "f38",
    "f39", "f40", "f41", "f42", "f43", "f44", "f45", "f46", "f47", "f48", "f49", "f50", "f51",
    "f52", "f53", "f54", "f55", "f56", "f57", "f58", "f59", "f60", "f61", "f62", "f63",
];

pub static F_ABINAME_LOOKUP: Lazy<HashMap<&str, FRegId>> = Lazy::new(|| {
    F_ABINAME_TABLE
        .iter()
        .enumerate()
        .map(|(i, n)| (*n, FRegId(i as u8)))
        .collect()
});

impl Display for FRegId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = F_ABINAME_TABLE[self.0 as usize];
        f.write_str(s)
    }
}

impl TryFrom<u32> for FRegId {
    type Error = anyhow::Error;
    fn try_from(rs: u32) -> Result<Self, Self::Error> {
        #[cfg(debug_assertions)]
        if rs >= (1 << (REG_BIT_WIDTH + 1)) {
            return Err(anyhow::anyhow!(
                "f register should be {REG_BIT_WIDTH}-bit integer, found {}",
                rs
            ));
        }
        Ok(Self(rs as u8))
    }
}

impl TryFrom<&str> for FRegId {
    type Error = ();
    fn try_from(rs: &str) -> Result<Self, Self::Error> {
        F_ABINAME_LOOKUP.get(rs).cloned().ok_or(())
    }
}
