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
        if bin == 0 {
            return Ok(Misc(MiscInstr::End));
        }
        let opcode = mask_lower(bin, 6);
        let rd = extract(bin, 7..11);
        let funct3 = extract(bin, 12..14);
        let rs1 = extract(bin, 15..19);
        let rs2 = extract(bin, 20..24);
        let funct7 = extract(bin, 25..31);
        let mut imm = extract(bin, 20..31);
        let sign = at(bin, 31);

        Ok(match opcode {
            // R fmt
            0b0110011 => {
                use RInstr::*;
                let instr = match (funct3, funct7) {
                    (0x0, 0x00) => Add,
                    (0x0, 0x20) => Sub,
                    (0x4, 0x00) => Xor,
                    (0x6, 0x00) => Or,
                    (0x7, 0x00) => And,
                    (0x1, 0x00) => Sll,
                    (0x5, 0x20) => Sra,
                    (0x2, 0x00) => Slt,
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
            // I fmt
            0b0010011 => {
                use IInstr::*;

                let instr = match (funct3, funct7) {
                    (0x0, _) => Addi,
                    (0x4, _) => Xori,
                    (0x6, _) => Ori,
                    (0x7, _) => Andi,
                    (0x1, 0x00) => {
                        imm = mask(imm, 0..4);
                        Slli
                    }
                    _ => Err(DecodeError::Invalid(bin))?,
                };
                let rd = rd.try_into()?;
                let rs1 = rs1.try_into()?;
                let imm = i_imm(sign, imm);
                I {
                    instr,
                    rd,
                    rs1,
                    imm,
                }
            }
            0b0000011 => {
                use IInstr::*;

                let instr = match funct3 {
                    0x2 => Lw,
                    _ => Err(DecodeError::Invalid(bin))?,
                };
                let rd = rd.try_into()?;
                let rs1 = rs1.try_into()?;
                let imm = i_imm(sign, imm);
                I {
                    instr,
                    rd,
                    rs1,
                    imm,
                }
            }
            0b1100111 => {
                use IInstr::*;

                let instr = match funct3 {
                    0x0 => Jalr,
                    _ => Err(DecodeError::Invalid(bin))?,
                };
                let rd = rd.try_into()?;
                let rs1 = rs1.try_into()?;
                let imm = i_imm(sign, imm);
                I {
                    instr,
                    rd,
                    rs1,
                    imm,
                }
            }
            // S fmt
            0b0100011 => {
                use SInstr::*;
                let instr = match funct3 {
                    0x2 => Sw,
                    _ => Err(DecodeError::Invalid(bin))?,
                };
                let rs1 = rs1.try_into()?;
                let rs2 = rs2.try_into()?;
                let imm = s_imm(bin, sign);
                S {
                    instr,
                    imm,
                    rs1,
                    rs2,
                }
            }
            // B fmt
            0b1100011 => {
                use BInstr::*;
                let imm = b_imm(bin, sign);

                let instr = match funct3 {
                    0x0 => Beq,
                    0x1 => Bne,
                    0x4 => Blt,
                    0x5 => Bge,
                    _ => Err(DecodeError::Invalid(bin))?,
                };
                let rs1 = rs1.try_into()?;
                let rs2 = rs2.try_into()?;
                B {
                    instr,
                    rs1,
                    rs2,
                    imm,
                }
            }
            // jal
            0b1101111 => {
                let imm = j_imm(bin, sign);
                let instr = JInstr::Jal;
                let rd = rd.try_into()?;
                J { instr, rd, imm }
            }
            // IO
            0b0001011 => {
                let rd = rd.try_into()?;
                IO(IOInstr::Inw { rd })
            }
            0b0101011 => {
                let rs = rs1.try_into()?;
                IO(IOInstr::Outb { rs })
            }
            0b0001111 => {
                let rd = rd.try_into()?;
                IO(IOInstr::Finw { rd })
            }
            // F
            0b1010011 => {
                if funct3 == 0 {
                    match funct7 {
                        funct7 @ (0b0000 | 0b0100 | 0b1000 | 0b1100 | 0b11000 | 0b11100
                        | 0b100000) => {
                            use EInstr::*;
                            let rd = rd.try_into()?;
                            let rs1 = rs1.try_into()?;
                            let rs2 = rs2.try_into()?;
                            let instr = match funct7 {
                                0b0000 => Fadd,
                                0b0100 => Fsub,
                                0b1000 => Fmul,
                                0b1100 => Fdiv,
                                0b011000 => Fsgnj,
                                0b011100 => Fsgnjn,
                                0b100000 => Fsgnjx,
                                _ => unreachable!(),
                            };
                            F(FInstr::E {
                                instr,
                                rd,
                                rs1,
                                rs2,
                            })
                        }
                        funct7 @ (0b10000 | 0b10100 | 0b1000000) => {
                            use HInstr::*;
                            let rd = rd.try_into()?;
                            let rs1 = rs1.try_into()?;
                            let instr = match funct7 {
                                0b10000 => Fsqrt,
                                0b10100 => Fhalf,
                                0b1000000 => Ffloor,
                                _ => unreachable!(),
                            };
                            F(FInstr::H { instr, rd, rs1 })
                        }
                        0b1000101 => {
                            use YInstr::*;
                            let rd = rd.try_into()?;
                            let rs1 = rs1.try_into()?;
                            let instr = Fftoi;
                            F(FInstr::Y { instr, rd, rs1 })
                        }
                        0b0100110 => {
                            use XInstr::*;
                            let rd = rd.try_into()?;
                            let rs1 = rs1.try_into()?;
                            let instr = Fitof;
                            F(FInstr::X { instr, rd, rs1 })
                        }
                        _ => Err(DecodeError::Invalid(bin))?,
                    }
                } else {
                    assert_eq!(funct7, 0b1010001);
                    if funct3 & 0b100 == 0 {
                        let rd = rd.try_into()?;
                        let rs1 = rs1.try_into()?;
                        let rs2 = rs2.try_into()?;
                        match funct3 {
                            0b001 => F(FInstr::K {
                                instr: KInstr::Flt,
                                rd,
                                rs1,
                                rs2,
                            }),
                            _ => Err(DecodeError::Invalid(bin))?,
                        }
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
            }
            // V, W
            0b1010111 => {
                use VInstr::*;
                use WInstr::*;
                if funct3 & 0b100 == 0 {
                    let rs1 = rs1.try_into()?;
                    let rs2 = rs2.try_into()?;
                    let imm = b_imm(bin, sign);
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
                    let imm = b_imm(bin, sign);
                    let instr = match funct3 {
                        0b100 => Fbeqz,
                        0b111 => Fbnez,
                        _ => Err(DecodeError::Invalid(bin))?,
                    };
                    F(FInstr::V { instr, rs1, imm })
                }
            }
            0b0000111 => {
                let rd = rd.try_into()?;
                let rs1 = rs1.try_into()?;
                let imm = i_imm(sign, imm);
                F(FInstr::Flw { rd, rs1, imm })
            }
            0b0100111 => {
                let rs1 = rs1.try_into()?;
                let rs2 = rs2.try_into()?;
                let imm = s_imm(bin, sign);
                F(FInstr::Fsw { rs1, rs2, imm })
            }
            _ => Err(DecodeError::Invalid(bin))?,
        })
    }
}

fn j_imm(bin: u32, sign: u32) -> u32 {
    let part1 = extract(bin, 12..19);
    let part2 = at(bin, 20);
    let part3 = extract(bin, 21..30);
    let part4 = sign;
    let imm = part1 << 12 | part2 << 11 | part3 << 1 | part4 << 20;
    sign_extend::<21>(sign, imm)
}

fn i_imm(sign: u32, imm: u32) -> u32 {
    sign_extend::<12>(sign, imm)
}

fn s_imm(bin: u32, sign: u32) -> u32 {
    let imm = (extract(bin, 25..31) << 5) | (extract(bin, 7..11));
    sign_extend::<12>(sign, imm)
}

fn b_imm(bin: u32, sign: u32) -> u32 {
    let at_7 = at(bin, 7);
    let lower = extract(bin, 8..11);
    let upper = extract(bin, 25..30);
    let imm = lower << 1 | upper << 5 | at_7 << 11 | sign << 12;
    sign_extend::<13>(sign, imm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode() {
        dbg!(Instr::decode_from(0x7d008113).unwrap());
    }
}
