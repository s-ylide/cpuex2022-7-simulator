use thiserror::Error;

use crate::{bin::*, instr::*};

#[derive(Error, Debug)]
pub enum DecodeError {
    #[error("invalid opcode found: `{0:#010x}`")]
    Invalid(u32),
}

impl DecodedInstr {
    /// returns which instr is encoded.
    pub fn decode_from(bin: u32) -> anyhow::Result<Self> {
        use Instr::*;
        if bin == 1 << 31 {
            return Ok(Misc(MiscInstr::End));
        }
        let opcode = mask_lower(bin, 3);
        let rd = extract(bin, 4..9);
        let funct3 = extract(bin, 10..12);
        let rs1 = extract(bin, 13..18);
        let rs2 = extract(bin, 19..24);
        let funct7 = extract(bin, 25..31);
        let imm_11_6 = extract(bin, 25..30);
        let sign = at(bin, 31);

        Ok(match opcode {
            0b0000 => {
                use RInstr::*;

                let instr = match funct3 {
                    0x0 => Add,
                    0x4 => Xor,
                    0x1 => Min,
                    0x3 => Max,
                    _ => Err(DecodeError::Invalid(bin))?,
                };
                let rd = rd.try_into()?;
                let rs1 = rs1.try_into()?;
                let rs2 = rs2.try_into()?;
                R {
                    instr,
                    rd,
                    rs1,
                    rs2,
                }
            }
            0b0010 => {
                use IInstr::*;

                let instr = match funct3 {
                    0x0 => Addi,
                    0x4 => Xori,
                    0x2 => Slli,
                    _ => Err(DecodeError::Invalid(bin))?,
                };
                let rd = rd.try_into()?;
                let rs1 = rs1.try_into()?;
                let imm = compose_3(sign, imm_11_6, rs2);
                I {
                    instr,
                    rd,
                    rs1,
                    imm,
                }
            }
            0b0110 => {
                use IInstr::*;

                let instr = Lw;
                let rd = rd.try_into()?;
                let rs1 = rs1.try_into()?;
                let imm = compose_3(sign, imm_11_6, rs2);
                I {
                    instr,
                    rd,
                    rs1,
                    imm,
                }
            }
            0b1010 => {
                use IInstr::*;

                let instr = match funct3 {
                    0x0 => Jalr,
                    _ => Err(DecodeError::Invalid(bin))?,
                };
                let rd = rd.try_into()?;
                let rs1 = rs1.try_into()?;
                let imm = compose_3(sign, imm_11_6, rs2);
                I {
                    instr,
                    rd,
                    rs1,
                    imm,
                }
            }
            0b0100 => {
                use SInstr::*;
                let instr = Sw;
                let rs1 = rs1.try_into()?;
                let rs2 = rs2.try_into()?;
                let imm = compose_3(sign, imm_11_6, rd);
                S {
                    instr,
                    imm,
                    rs1,
                    rs2,
                }
            }
            0b1000 => {
                use BInstr::*;

                let instr = match funct3 {
                    0x0 => Beq,
                    0x1 => Bne,
                    0x4 => Blt,
                    0x5 => Bge,
                    0x2 => Bxor,
                    0x3 => Bxnor,
                    _ => Err(DecodeError::Invalid(bin))?,
                };
                let rs1 = rs1.try_into()?;
                let rs2 = rs2.try_into()?;
                let imm = compose_4(sign, imm_11_6, rd);
                B {
                    instr,
                    rs1,
                    rs2,
                    imm,
                }
            }
            0b1100 => {
                use PInstr::*;

                let instr = match funct3 {
                    0x0 => Beqi,
                    0x1 => Bnei,
                    0x4 => Blti,
                    0x5 => Bgei,
                    0x6 => Bgti,
                    0x7 => Blei,
                    _ => Err(DecodeError::Invalid(bin))?,
                };
                let rs1 = rs1.try_into()?;
                let imm2 = sign_extend::<5>(at(rs2, 5), rs2);
                let imm = compose_4(sign, imm_11_6, rd);
                P {
                    instr,
                    rs1,
                    imm,
                    imm2,
                }
            }
            0b1110 => {
                let imm = compose_6(sign, imm_11_6, rs2, rs1, funct3);
                let instr = JInstr::Jal;
                let rd = rd.try_into()?;
                J { instr, rd, imm }
            }
            // IO
            0b0011 => match funct3 {
                0b001 => {
                    let rd = rd.try_into()?;
                    IO(IOInstr::Inw { rd })
                }
                0b010 => {
                    let rs = rs1.try_into()?;
                    IO(IOInstr::Outb { rs })
                }
                0b100 => {
                    let rd = rd.try_into()?;
                    IO(IOInstr::Finw { rd })
                }
                _ => Err(DecodeError::Invalid(bin))?,
            },
            // F
            0b0001 => {
                if funct3 == 0 {
                    let funct5 = funct7 >> 2;
                    match funct5 {
                        funct5 @ (0b00 | 0b01 | 0b10 | 0b11 | 0b110 | 0b111 | 0b1000) => {
                            use EInstr::*;
                            let rd = rd.try_into()?;
                            let rs1 = rs1.try_into()?;
                            let rs2 = rs2.try_into()?;
                            let instr = match funct5 {
                                0b0000 => Fadd,
                                0b0001 => Fsub,
                                0b0010 => Fmul,
                                0b0011 => Fdiv,
                                0b0110 => Fsgnj,
                                0b0111 => Fsgnjn,
                                0b1000 => Fsgnjx,
                                _ => unreachable!(),
                            };
                            F(FInstr::E {
                                instr,
                                rd,
                                rs1,
                                rs2,
                            })
                        }
                        funct5 @ (0b100 | 0b101 | 0b1100 | 0b1011 | 0b01001) => {
                            use HInstr::*;
                            let rd = rd.try_into()?;
                            let rs1 = rs1.try_into()?;
                            let instr = match funct5 {
                                0b00100 => Fsqrt,
                                0b00101 => Fhalf,
                                0b01100 => Ffrac,
                                0b01011 => Finv,
                                0b01001 => Ffloor,
                                _ => unreachable!(),
                            };
                            F(FInstr::H { instr, rd, rs1 })
                        }
                        0b10001 => {
                            use YInstr::*;
                            let rd = rd.try_into()?;
                            let rs1 = rs1.try_into()?;
                            let instr = Fftoi;
                            F(FInstr::Y { instr, rd, rs1 })
                        }
                        0b11001 => {
                            use XInstr::*;
                            let rd = rd.try_into()?;
                            let rs1 = rs1.try_into()?;
                            let instr = Fitof;
                            F(FInstr::X { instr, rd, rs1 })
                        }
                        _ => Err(DecodeError::Invalid(bin))?,
                    }
                } else if sign == 0 {
                    use GInstr::*;
                    let rd = rd.try_into()?;
                    let rs1 = rs1.try_into()?;
                    let rs2 = rs2.try_into()?;
                    let rs3 = imm_11_6.try_into()?;
                    let instr = match funct3 {
                        0b001 => Fmadd,
                        0b010 => Fmsub,
                        0b101 => Fnmadd,
                        0b110 => Fnmsub,
                        _ => Err(DecodeError::Invalid(bin))?,
                    };
                    F(FInstr::G {
                        instr,
                        rd,
                        rs1,
                        rs2,
                        rs3,
                    })
                } else if funct3 == 0b001 {
                    let rd = rd.try_into()?;
                    let rs1 = rs1.try_into()?;
                    let rs2 = rs2.try_into()?;
                    F(FInstr::K {
                        instr: KInstr::Flt,
                        rd,
                        rs1,
                        rs2,
                    })
                } else {
                    use YInstr::*;
                    let rd = rd.try_into()?;
                    let rs1 = rs1.try_into()?;
                    let instr = match funct3 {
                        0b100 => Fiszero,
                        0b101 => Fispos,
                        0b110 => Fisneg,
                        _ => Err(DecodeError::Invalid(bin))?,
                    };
                    F(FInstr::Y { instr, rd, rs1 })
                }
            }
            // V, W
            0b1001 => {
                use VInstr::*;
                use WInstr::*;
                if funct3 & 0b100 == 0 {
                    let rs1 = rs1.try_into()?;
                    let rs2 = rs2.try_into()?;
                    let imm = compose_4(sign, imm_11_6, rd);
                    let instr = match funct3 {
                        0b001 => Fblt,
                        0b010 => Fbge,
                        _ => Err(DecodeError::Invalid(bin))?,
                    };
                    F(FInstr::W {
                        instr,
                        rs1,
                        rs2,
                        imm,
                    })
                } else {
                    let rs1 = rs1.try_into()?;
                    let imm = compose_4(sign, imm_11_6, rd);
                    let instr = match funct3 {
                        0b100 => Fbeqz,
                        0b111 => Fbnez,
                        _ => Err(DecodeError::Invalid(bin))?,
                    };
                    F(FInstr::V { instr, rs1, imm })
                }
            }
            0b0111 => {
                let rd = rd.try_into()?;
                let rs1 = rs1.try_into()?;
                let imm = compose_3(sign, imm_11_6, rs2);
                F(FInstr::Flw { rd, rs1, imm })
            }
            0b0101 => {
                let rs1 = rs1.try_into()?;
                let rs2 = rs2.try_into()?;
                let imm = compose_3(sign, imm_11_6, rd);
                F(FInstr::Fsw { rs1, rs2, imm })
            }
            _ => Err(DecodeError::Invalid(bin))?,
        })
    }
}

fn compose_3(sign: u32, imm_11_6: u32, imm_5_0: u32) -> u32 {
    let imm = (imm_11_6 << 6) | imm_5_0;
    sign_extend::<12>(sign, imm)
}

fn compose_4(sign: u32, imm_11_6: u32, imm_5_2_13_12: u32) -> u32 {
    let imm_13_12 = mask(imm_5_2_13_12, 0..1);
    let imm = (imm_13_12 << 12) | (imm_11_6 << 6) | mask(imm_5_2_13_12, 2..5);
    sign_extend::<14>(sign, imm)
}

fn compose_6(sign: u32, imm_11_6: u32, imm_5_2_13_12: u32, imm_22_17: u32, imm_16_14: u32) -> u32 {
    let imm_13_12 = mask(imm_5_2_13_12, 0..1);
    let imm = (imm_22_17 << 17)
        | (imm_16_14 << 14)
        | (imm_13_12 << 12)
        | (imm_11_6 << 6)
        | mask(imm_5_2_13_12, 2..5);
    sign_extend::<23>(sign, imm)
}
