use core::fmt;
use std::{fmt::Display, mem};

use num_enum::UnsafeFromPrimitive;

use crate::register::{FRegId, RegId};

/// represents instruction. immediates are sign-extended.
#[derive(Debug, Clone)]
pub enum Instr<IR, IW, FR, FW> {
    R {
        instr: RInstr,
        rd: IW,
        rs1: IR,
        rs2: IR,
    },
    I {
        instr: IInstr,
        rd: IW,
        rs1: IR,
        imm: u32,
    },
    S {
        instr: SInstr,
        rs1: IR,
        rs2: IR,
        imm: u32,
    },
    B {
        instr: BInstr,
        rs1: IR,
        rs2: IR,
        imm: u32,
    },
    P {
        instr: PInstr,
        rs1: IR,
        imm: u32,
        imm2: u32,
    },
    J {
        instr: JInstr,
        rd: IW,
        imm: u32,
    },
    IO(IOInstr<IR, IW, FW>),
    F(FInstr<IR, IW, FR, FW>),
    Misc(MiscInstr),
}

pub struct InstrId(u8);

impl InstrId {
    /// upper bound
    pub const MAX: usize = (15 << 3) + 3;
    pub fn inner(&self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for InstrId {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        macro_rules! variant_count {
            ($ty:ty) => {
                mem::variant_count::<$ty>() as u8
            };
        }
        let upper = value >> 3;
        let lower = value & 0b111;
        let b = match upper {
            0 => lower < variant_count!(RInstr),
            1 => lower < variant_count!(IInstr),
            2 => lower < variant_count!(SInstr),
            3 => lower < variant_count!(BInstr),
            4 => lower < variant_count!(PInstr),
            5 => lower < variant_count!(JInstr),
            6 => lower < variant_count!(IOInstr<(), (), ()>),
            7 => lower < variant_count!(EInstr),
            8 => lower < variant_count!(GInstr),
            9 => lower < variant_count!(HInstr),
            10 => lower < variant_count!(KInstr),
            11 => lower < variant_count!(XInstr),
            12 => lower < variant_count!(YInstr),
            13 => lower < variant_count!(WInstr),
            14 => lower < variant_count!(VInstr),
            15 => lower < 3,
            _ => false,
        };
        if b {
            Ok(Self(value))
        } else {
            Err(())
        }
    }
}

impl fmt::Display for InstrId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = self.0;
        let upper = id >> 3;
        let lower = id & 0b111;
        unsafe {
            match upper {
                0 => write!(f, "{}", RInstr::unchecked_transmute_from(lower)),
                1 => write!(f, "{}", IInstr::unchecked_transmute_from(lower)),
                2 => write!(f, "{}", SInstr::unchecked_transmute_from(lower)),
                3 => write!(f, "{}", BInstr::unchecked_transmute_from(lower)),
                4 => write!(f, "{}", PInstr::unchecked_transmute_from(lower)),
                5 => write!(f, "{}", JInstr::unchecked_transmute_from(lower)),
                6 => match lower {
                    0 => write!(f, "outb"),
                    1 => write!(f, "inw"),
                    2 => write!(f, "finw"),
                    _ => unreachable!("lower == {lower}"),
                },
                7 => write!(f, "{}", EInstr::unchecked_transmute_from(lower)),
                8 => write!(f, "{}", GInstr::unchecked_transmute_from(lower)),
                9 => write!(f, "{}", HInstr::unchecked_transmute_from(lower)),
                10 => write!(f, "{}", KInstr::unchecked_transmute_from(lower)),
                11 => write!(f, "{}", XInstr::unchecked_transmute_from(lower)),
                12 => write!(f, "{}", YInstr::unchecked_transmute_from(lower)),
                13 => write!(f, "{}", WInstr::unchecked_transmute_from(lower)),
                14 => write!(f, "{}", VInstr::unchecked_transmute_from(lower)),
                15 => match lower {
                    0 => write!(f, "flw"),
                    1 => write!(f, "fsw"),
                    2 => write!(f, "end"),
                    _ => unreachable!("lower == {lower}"),
                },
                _ => unreachable!("upper == {upper}"),
            }
        }
    }
}

impl<IR, IW, FR, FW> Instr<IR, IW, FR, FW> {
    pub fn id(&self) -> InstrId {
        use FInstr::*;
        use IOInstr::*;
        use Instr::*;
        fn id(upper: u8, lower: u8) -> InstrId {
            InstrId((upper << 3) + lower)
        }
        match self {
            R { instr, .. } => id(0, *instr as u8),
            I { instr, .. } => id(1, *instr as u8),
            S { instr, .. } => id(2, *instr as u8),
            B { instr, .. } => id(3, *instr as u8),
            P { instr, .. } => id(4, *instr as u8),
            J { instr, .. } => id(5, *instr as u8),
            IO(io) => id(6, {
                match io {
                    Outb { .. } => 0,
                    Inw { .. } => 1,
                    Finw { .. } => 2,
                }
            } as u8),
            F(f) => match f {
                E { instr, .. } => id(7, *instr as u8),
                G { instr, .. } => id(8, *instr as u8),
                H { instr, .. } => id(9, *instr as u8),
                K { instr, .. } => id(10, *instr as u8),
                X { instr, .. } => id(11, *instr as u8),
                Y { instr, .. } => id(12, *instr as u8),
                W { instr, .. } => id(13, *instr as u8),
                V { instr, .. } => id(14, *instr as u8),
                Flw { .. } => id(15, 0),
                Fsw { .. } => id(15, 1),
            },
            Misc(MiscInstr::End) => id(15, 2),
        }
    }
}

pub type DecodedInstr = Instr<RegId, RegId, FRegId, FRegId>;

impl<IR: Display, IW: Display, FR: Display, FW: Display> Display for Instr<IR, IW, FR, FW> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Instr::*;
        match self {
            R {
                instr,
                rd,
                rs1,
                rs2,
            } => write!(f, "{instr} {rd}, {rs1}, {rs2}"),
            I {
                instr: IInstr::Lw,
                rd,
                rs1,
                imm,
            } => write!(
                f,
                "{instr} {rd}, {imm}({rs1})",
                instr = IInstr::Lw,
                imm = *imm as i32
            ),
            I {
                instr,
                rd,
                rs1,
                imm,
            } => write!(f, "{instr} {rd}, {rs1}, {imm}", imm = *imm as i32),
            S {
                instr,
                rs1,
                rs2,
                imm,
            } => write!(f, "{instr} {rs2}, {imm}({rs1})", imm = *imm as i32),
            B {
                instr,
                rs1,
                rs2,
                imm,
            } => write!(f, "{instr} {rs1}, {rs2}, {imm}", imm = *imm as i32),
            P {
                instr,
                rs1,
                imm,
                imm2,
            } => write!(
                f,
                "{instr} {rs1}, {imm2}, {imm}",
                imm = *imm as i32,
                imm2 = *imm2 as i32
            ),
            J { instr, rd, imm } => write!(f, "{instr} {rd}, {imm}", imm = *imm as i32),
            IO(instr) => write!(f, "{instr}"),
            F(finstr) => {
                use FInstr::*;
                match finstr {
                    E {
                        instr,
                        rd,
                        rs1,
                        rs2,
                    } => write!(f, "{instr} {rd}, {rs1}, {rs2}"),
                    G {
                        instr,
                        rd,
                        rs1,
                        rs2,
                        rs3,
                    } => write!(f, "{instr} {rd}, {rs1}, {rs2}, {rs3}"),
                    H { instr, rd, rs1 } => write!(f, "{instr} {rd}, {rs1}"),
                    K {
                        instr,
                        rd,
                        rs1,
                        rs2,
                    } => write!(f, "{instr} {rd}, {rs1}, {rs2}"),
                    X { instr, rd, rs1 } => write!(f, "{instr} {rd}, {rs1}"),
                    Y { instr, rd, rs1 } => write!(f, "{instr} {rd}, {rs1}"),
                    W {
                        instr,
                        rs1,
                        rs2,
                        imm,
                    } => write!(f, "{instr} {rs1}, {rs2}, {imm}"),
                    V { instr, rs1, imm } => write!(f, "{instr} {rs1}, {imm}"),
                    Flw { rd, rs1, imm } => write!(f, "flw {rd}, {imm}({rs1})"),
                    Fsw { rs2, rs1, imm } => write!(f, "fsw {rs2}, {imm}({rs1})"),
                }
            }
            Misc(MiscInstr::End) => write!(f, "end"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum FInstr<IR, IW, FR, FW> {
    E {
        instr: EInstr,
        rd: FW,
        rs1: FR,
        rs2: FR,
    },
    G {
        instr: GInstr,
        rd: FW,
        rs1: FR,
        rs2: FR,
        rs3: FR,
    },
    H {
        instr: HInstr,
        rd: FW,
        rs1: FR,
    },
    K {
        instr: KInstr,
        rd: IW,
        rs1: FR,
        rs2: FR,
    },
    X {
        instr: XInstr,
        rd: FW,
        rs1: IR,
    },
    Y {
        instr: YInstr,
        rd: IW,
        rs1: FR,
    },
    W {
        instr: WInstr,
        rs1: FR,
        rs2: FR,
        imm: u32,
    },
    V {
        instr: VInstr,
        rs1: FR,
        imm: u32,
    },
    /// I
    Flw {
        rd: FW,
        rs1: IR,
        imm: u32,
    },
    /// S
    Fsw {
        rs2: FR,
        rs1: IR,
        imm: u32,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum MiscInstr {
    End,
}

cfg_if::cfg_if! {
    if #[cfg(feature = "isa_2nd")] {
        #[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
        #[repr(u8)]
        pub enum RInstr {
            Add,
            Xor,
            Min,
            Max,
        }

        impl Display for RInstr {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                use RInstr::*;
                let s = match self {
                    Add => "add",
                    Xor => "xor",
                    Min => "min",
                    Max => "max",
                };
                f.write_str(s)
            }
        }
    }
    else {
        #[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
        #[repr(u8)]
        pub enum RInstr {
            Add,
            Sub,
            Xor,
            Or,
            And,
            Sll,
            Sra,
            Slt,
        }

        impl Display for RInstr {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                use RInstr::*;
                let s = match self {
                    Add => "add",
                    Sub => "sub",
                    Xor => "xor",
                    Or => "or",
                    And => "and",
                    Sll => "sll",
                    Sra => "sra",
                    Slt => "slt",
                };
                f.write_str(s)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum IInstr {
    Addi,
    Xori,
    #[cfg(not(feature = "isa_2nd"))]
    Ori,
    #[cfg(not(feature = "isa_2nd"))]
    Andi,
    Slli,
    #[cfg(not(feature = "isa_2nd"))]
    Slti,
    Lw,
    Jalr,
}

impl Display for IInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use IInstr::*;
        let s = match self {
            Addi => "addi",
            Xori => "xori",
            #[cfg(not(feature = "isa_2nd"))]
            Ori => "ori",
            #[cfg(not(feature = "isa_2nd"))]
            Andi => "andi",
            Slli => "slli",
            #[cfg(not(feature = "isa_2nd"))]
            Slti => "slti",
            Lw => "lw",
            Jalr => "jalr",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum SInstr {
    Sw,
}

impl Display for SInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use SInstr::*;
        let s = match self {
            Sw => "sw",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum BInstr {
    Beq,
    Bne,
    Blt,
    Bge,
    #[cfg(feature = "isa_2nd")]
    Bxor,
    #[cfg(feature = "isa_2nd")]
    Bxnor,
}

impl Display for BInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use BInstr::*;
        let s = match self {
            Beq => "beq",
            Bne => "bne",
            Blt => "blt",
            Bge => "bge",
            #[cfg(feature = "isa_2nd")]
            Bxor => "bxor",
            #[cfg(feature = "isa_2nd")]
            Bxnor => "bxnor",
        };
        f.write_str(s)
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "isa_2nd")] {
        #[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
        #[repr(u8)]
        pub enum PInstr {
            Beqi,
            Bnei,
            Blti,
            Bgei,
            Bgti,
            Blei,
        }

        impl Display for PInstr {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                use PInstr::*;
                let s = match self {
                    Beqi => "beqi",
                    Bnei => "bnei",
                    Blti => "blti",
                    Bgei => "bgei",
                    Bgti => "bgti",
                    Blei => "blei",
                };
                f.write_str(s)
            }
        }
    }
    else {
        #[derive(Debug, Clone, Copy)]
        pub enum PInstr {}

        impl PInstr {
            fn unchecked_transmute_from(_: u8) -> Self {
                unreachable!()
            }
        }

        impl Display for PInstr {
            fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                unreachable!()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum JInstr {
    Jal,
}

impl Display for JInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use JInstr::*;
        let s = match self {
            Jal => "jal",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum IOInstr<IR, IW, FW> {
    Outb { rs: IR },
    Inw { rd: IW },
    Finw { rd: FW },
}

impl<IR: Display, IW: Display, FW: Display> Display for IOInstr<IR, IW, FW> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use IOInstr::*;
        match self {
            Outb { rs } => write!(f, "outb {rs}"),
            Inw { rd } => write!(f, "inw {rd}"),
            Finw { rd } => write!(f, "finw {rd}"),
        }
    }
}

#[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum EInstr {
    Fadd,
    Fsub,
    Fmul,
    Fdiv,
    Fsgnj,
    Fsgnjn,
    Fsgnjx,
}

impl Display for EInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use EInstr::*;
        let s = match self {
            Fadd => "fadd",
            Fsub => "fsub",
            Fmul => "fmul",
            Fdiv => "fdiv",
            Fsgnj => "fsgnj",
            Fsgnjn => "fsgnjn",
            Fsgnjx => "fsgnjx",
        };
        f.write_str(s)
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "isa_2nd")] {
        #[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
        #[repr(u8)]
        pub enum GInstr {
            Fmadd,
            Fmsub,
            Fnmadd,
            Fnmsub,
        }

        impl Display for GInstr {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                use GInstr::*;
                let s = match self {
                    Fmadd => "fmadd",
                    Fmsub => "fmsub",
                    Fnmadd => "fnmadd",
                    Fnmsub => "fnmsub",
                };
                f.write_str(s)
            }
        }
    }
    else {
        #[derive(Debug, Clone, Copy)]
        pub enum GInstr {}

        impl GInstr {
            fn unchecked_transmute_from(_: u8) -> Self {
                unreachable!()
            }
        }

        impl Display for GInstr {
            fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                unreachable!()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum HInstr {
    Fsqrt,
    Fhalf,
    Ffloor,
    #[cfg(feature = "isa_2nd")]
    Ffrac,
    #[cfg(feature = "isa_2nd")]
    Finv,
}

impl Display for HInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use HInstr::*;
        let s = match self {
            Fsqrt => "fsqrt",
            Fhalf => "fhalf",
            Ffloor => "ffloor",
            #[cfg(feature = "isa_2nd")]
            Ffrac => "ffrac",
            #[cfg(feature = "isa_2nd")]
            Finv => "finv",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum KInstr {
    Flt,
}

impl Display for KInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use KInstr::*;
        let s = match self {
            Flt => "flt",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum XInstr {
    Fitof,
}

impl Display for XInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use XInstr::*;
        let s = match self {
            Fitof => "fitof",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum YInstr {
    Fiszero,
    Fispos,
    Fisneg,
    Fftoi,
}

impl Display for YInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use YInstr::*;
        let s = match self {
            Fiszero => "fiszero",
            Fispos => "fispos",
            Fisneg => "fisneg",
            Fftoi => "fftoi",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum WInstr {
    Fblt,
    Fbge,
}

impl Display for WInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use WInstr::*;
        let s = match self {
            Fblt => "fblt",
            Fbge => "fbge",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum VInstr {
    Fbeqz,
    Fbnez,
}

impl Display for VInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use VInstr::*;
        let s = match self {
            Fbeqz => "fbeqz",
            Fbnez => "fbnez",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instr_id() {
        let instr = IInstr::Addi;
        let instr: Instr<(), (), (), ()> = Instr::I {
            instr,
            rd: (),
            rs1: (),
            imm: 0,
        };
        let s = format!("{}", instr.id());
        assert_eq!(s, "addi");
        let instr: Instr<(), (), (), ()> = Instr::F(FInstr::Fsw {
            rs2: (),
            rs1: (),
            imm: 0,
        });
        let s = format!("{}", instr.id());
        assert_eq!(s, "fsw");
    }
}
