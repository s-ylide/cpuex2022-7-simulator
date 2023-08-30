//! Runtime type information

use std::fmt;

use crate::memory::Addr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Ty {
    I32,
    Usize,
    I32OrUsize,
    F32,
    Unknown,
}

use Ty::*;

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            I32 => write!(f, "i32"),
            Usize => write!(f, "usize"),
            I32OrUsize => write!(f, "i32 | usize"),
            F32 => write!(f, "f32"),
            Unknown => write!(f, "?"),
        }
    }
}

impl PartialOrd for Ty {
    /// `a < b` if and only if `b` is precise than `a`.
    /// ```
    /// use core_sim::ty::Ty;
    ///
    /// let a = Unknown;
    /// let b = I32;
    /// let c = F32;
    ///
    /// assert!(a < b);
    /// assert!(a < c);
    /// assert_eq!(b.partial_cmp(&c), None);
    /// ```
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::*;
        match (self, other) {
            (a, b) if a == b => Some(Equal),
            (I32, I32OrUsize) | (Usize, I32OrUsize) => Some(Greater),
            (I32OrUsize, I32) | (I32OrUsize, Usize) => Some(Less),
            (_, Unknown) => Some(Greater),
            (Unknown, _) => Some(Less),
            _ => None,
        }
    }
}

pub trait Typed {
    fn typed(self, ty: Ty) -> TypedU32;
}

impl Typed for u32 {
    fn typed(self, ty: Ty) -> TypedU32 {
        TypedU32 { ty, value: self }
    }
}

#[derive(Clone, Copy)]
pub struct TypedU32 {
    pub ty: Ty,
    value: u32,
}

impl fmt::Debug for TypedU32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl TypedU32 {
    pub fn as_i(&self) -> Option<i32> {
        if self.ty >= I32OrUsize {
            Some(self.value as i32)
        } else {
            None
        }
    }
    pub fn as_i32(&self) -> Option<i32> {
        if self.ty == I32 {
            Some(self.value as i32)
        } else {
            None
        }
    }
    pub fn as_f32(&self) -> Option<f32> {
        if self.ty == F32 {
            Some(f32::from_bits(self.value))
        } else {
            None
        }
    }
    pub fn get_unchecked(&self) -> u32 {
        self.value
    }
}

impl fmt::Display for TypedU32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.ty {
            I32 => write!(f, "{}", self.value as i32),
            Usize => write!(f, "{}", Addr::new(self.value as usize)),
            I32OrUsize => write!(
                f,
                "{i32} ({u32} as addr)",
                i32 = self.value as i32,
                u32 = Addr::new(self.value as usize)
            ),
            F32 => write!(f, "{}", f32::from_bits(self.value)),
            Unknown => write!(
                f,
                "{i32} ({u32} as addr) ({f32} as f32)",
                i32 = self.value as i32,
                u32 = Addr::new(self.value as usize),
                f32 = f32::from_bits(self.value)
            ),
        }
    }
}
