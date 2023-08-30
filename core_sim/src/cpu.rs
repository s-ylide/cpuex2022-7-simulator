#[allow(unused)]
use std::cmp;
#[cfg(feature = "time_predict")]
use std::collections::VecDeque;
#[cfg(feature = "time_predict")]
use std::ops::Index;
use std::ops::Range;

use thiserror::Error;

use crate::{
    common::{Pc, SpyResult, SpyWatchKind},
    fpu_wrapper::fpu,
    instr::*,
    io::{Input, Output},
    memory::{Addr, Memory, MemoryAccessError, SpyUnit, RAM_BYTE_SIZE},
    reg_file::{RegFile, RegFileView, ShowRegFileKind},
    register::{FRegId, RegId},
    ty::TypedU32,
};

#[cfg(feature = "time_predict")]
use crate::branch_predictor::{BranchPredictor, NUM_COUNTERS};
#[cfg(feature = "stat")]
use crate::cache::{Cache, CACHE_NUM_LINES};
#[cfg(feature = "stat")]
use crate::stat::{AddStats, Stat, Stats};

#[cfg(feature = "time_predict")]
const DDR2_ACCESS_CYCLES: usize = 90;
#[cfg(feature = "time_predict")]
const BRAM_WORD_SIZE: usize = 16384;
#[cfg(feature = "time_predict")]
const STACK_WORD_SIZE: usize = 256;

#[cfg(feature = "time_predict")]
pub enum PipelineStage {
    InstrFetch,
    InstrDecode,
    Execute,
    MemoryAccess,
    WriteBack,
}

#[cfg(feature = "time_predict")]
pub struct PipelineStat {
    ex_cycles: usize,
    ma_cycles: usize,
    result_ready_stage: Option<PipelineStage>,
    write_back_id: Option<RegId>,
    float_write_back_id: Option<FRegId>,
}

pub struct InstrFetchOutput {
    id_in: InstrDecodeInput,
    old_pc: Pc,
    pc_plus4: Pc,
}

pub struct InstrDecodeInput {
    bin: u32,
}

pub struct RegFetchInput {
    instr: Instr<RegId, RegId, FRegId, FRegId>,
    old_pc: u32,
    pc_plus4: u32,
}

pub struct ExecuteInput {
    instr: Instr<u32, RegId, f32, FRegId>,
    old_pc: u32,
    pc_plus4: u32,
}

#[derive(Default)]
pub struct ExecuteOutput {
    ma_in: Option<MemoryAccessInput>,
    wb_in: Option<WriteBackInput>,
    new_pc: Option<usize>,
    #[cfg(feature = "time_predict")]
    use_fpu: bool,
    #[cfg(feature = "time_predict")]
    flush: bool,
    #[cfg(feature = "time_predict")]
    cycles: usize,
    end: bool,
}

pub enum MemoryAccessInput {
    I { addr: usize, val: u32 },
    F { addr: usize, val: f32 },
    IMem { id: RegId, addr: usize },
    FMem { id: FRegId, addr: usize },
}

#[derive(Clone, Copy)]
pub enum WriteBackInput {
    I { id: RegId, val: u32 },
    F { id: FRegId, val: f32 },
}

#[derive(Default)]
pub struct MemoryAccessOutput {
    #[cfg(feature = "time_predict")]
    cycles: usize,
    #[cfg(feature = "stat")]
    cache_hit: bool,
    #[cfg(feature = "time_predict")]
    use_bram: bool,
    wb_in: Option<WriteBackInput>,
}

pub struct Cpu<I, O> {
    reg_file: RegFile,
    memory: Memory<RAM_BYTE_SIZE>,
    #[cfg(feature = "stat")]
    cache: Cache<CACHE_NUM_LINES>,
    pc: Pc,
    input: I,
    output: O,
    #[cfg(feature = "time_predict")]
    branch_predictor: BranchPredictor<NUM_COUNTERS>,
    #[cfg(feature = "time_predict")]
    pipeline_state: VecDeque<Option<PipelineStat>>,
    #[cfg(feature = "stat")]
    pub i_stat: stat::InstrStat,
    #[cfg(feature = "stat")]
    pub c_stat: stat::CacheStat,
    #[cfg(feature = "stat")]
    pub b_stat: stat::BranchStat,
}

pub struct CpuOutput<O> {
    pub value: O,
}

#[derive(Error, Debug)]
pub enum InputError {
    #[error("failed to parse SLD file: {0}")]
    ParseSld(String),
}

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error(transparent)]
    MemoryAccessError(#[from] MemoryAccessError),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl RuntimeError {
    pub fn level(&self) -> RuntimeErrorLevel {
        RuntimeErrorLevel::Fatal
    }
}

pub enum RuntimeErrorLevel {
    /// cannot restart, but program need not halt
    Fatal,
}

impl RuntimeErrorLevel {
    /// Returns `true` if the runtime error level is [`Fatal`].
    ///
    /// [`Fatal`]: RuntimeErrorLevel::Fatal
    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Fatal)
    }
}

impl From<nom::Err<nom::error::Error<&str>>> for InputError {
    fn from(e: nom::Err<nom::error::Error<&str>>) -> Self {
        InputError::ParseSld(e.to_string())
    }
}

type Result<T, E = RuntimeError> = std::result::Result<T, E>;

impl<I: Input, O: Output> Cpu<I, O> {
    pub fn new(mem: &[u8], input: I, output: O) -> Result<Self, InputError> {
        let (data_len, text_len) = Cpu::<I, O>::get_data_and_text_len(mem);
        log::info!(".data: {d} bytes ({d:#010x} as hex)", d = data_len << 2);
        log::info!(".text: {t} bytes ({t:#010x} as hex)", t = text_len << 2);
        let mut reg_file = RegFile::new();
        reg_file.set_hp(data_len + text_len);
        reg_file.set_sp((RAM_BYTE_SIZE >> 2) as u32 - 1);
        reg_file.set_f(FRegId::try_from(1).unwrap(), 1.0);
        reg_file.end_init();
        let mut s = Self {
            memory: Memory::<RAM_BYTE_SIZE>::new(reg_file.mem_region()),
            #[cfg(feature = "stat")]
            cache: Cache::<CACHE_NUM_LINES>::new(),
            reg_file,
            pc: Pc::new(data_len << 2),
            input,
            output,
            #[cfg(feature = "time_predict")]
            branch_predictor: BranchPredictor::<NUM_COUNTERS>::new(),
            #[cfg(feature = "stat")]
            i_stat: Default::default(),
            #[cfg(feature = "stat")]
            b_stat: Default::default(),
            #[cfg(feature = "stat")]
            c_stat: Default::default(),
            #[cfg(feature = "time_predict")]
            pipeline_state: VecDeque::from([None, None, None, None, None]),
        };
        let text_begin = data_len << 2;
        let text_end = text_begin + (text_len << 2);
        s.init_memory(&mem[8..], text_begin..text_end);
        Ok(s)
    }
    pub fn get_data_and_text_len(mem: &[u8]) -> (u32, u32) {
        let data_len = u32::from_le_bytes({
            let mut v: [u8; 4] = [0; 4];
            v[..4].copy_from_slice(&mem[0..4]);
            v
        });
        let text_len = u32::from_le_bytes({
            let mut v: [u8; 4] = [0; 4];
            v[..4].copy_from_slice(&mem[4..8]);
            v
        });
        (data_len, text_len)
    }
    pub fn into_output(self) -> CpuOutput<O> {
        CpuOutput { value: self.output }
    }
}

#[cfg(feature = "stat")]
impl<I, O> AddStats for Cpu<I, O> {
    fn add_stats(&self, buf: &mut Stats) {
        self.memory.add_stats(buf);
        self.reg_file.add_stats(buf);
        buf.push(Box::new(self.i_stat));
        buf.push(Box::new(self.b_stat));
        buf.push(Box::new(self.c_stat));
    }
}

#[cfg(feature = "stat")]
mod stat {
    use std::fmt;

    use super::*;
    use crate::{instr::InstrId, stat::*};

    const MAX_INSTR_ID: usize = InstrId::MAX;

    #[derive(Clone, Copy)]
    pub struct InstrStat {
        /// index by Instr::id()
        instr_executed: [usize; MAX_INSTR_ID],
    }

    impl Stat for InstrStat {
        fn view(&self, max_width: usize) -> Box<dyn StatView + '_> {
            Box::new(InstrStatView::new(self, max_width))
        }
    }

    pub struct InstrStatView<'a> {
        stat: &'a InstrStat,
        chunk_size: usize,
    }

    impl<'a> InstrStatView<'a> {
        pub fn new(stat: &'a InstrStat, max_width: usize) -> Self {
            Self {
                stat,
                chunk_size: Self::chunk_size(max_width),
            }
        }
    }

    impl StatView for InstrStatView<'_> {
        fn header(&self) -> &'static str {
            "executed instructions"
        }
        fn width(&self) -> usize {
            Self::width_by_chunk_size(self.chunk_size)
        }
    }

    impl<'a> Width for InstrStatView<'a> {
        fn width_by_chunk_size(chunk_size: usize) -> usize {
            chunk_size * 21 + (chunk_size - 1) * 2 + 2
        }
    }

    impl fmt::Display for InstrStatView<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fn fmt_inner(
                map: Vec<String>,
                chunk_size: usize,
                f: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                for chunk in map.chunks(chunk_size) {
                    let s = chunk.join(", ");
                    writeln!(f, "  {s}")?;
                }
                Ok(())
            }
            let map: Vec<_> = (0..MAX_INSTR_ID)
                .filter_map(|index| {
                    let id = InstrId::try_from(index as u8).ok()?;
                    let str = format!("{id}");
                    let count = self.stat.instr_executed[index];
                    Some(format!("{str:>8}: {count:>11}"))
                })
                .collect();
            fmt_inner(map, self.chunk_size, f)?;
            Ok(())
        }
    }

    impl InstrStat {
        pub fn new() -> Self {
            Self {
                instr_executed: [0; MAX_INSTR_ID],
            }
        }
        pub fn encounter_instr(&mut self, d: &DecodedInstr) {
            self.instr_executed[d.id().inner() as usize] += 1;
        }
    }

    impl Default for InstrStat {
        fn default() -> Self {
            Self::new()
        }
    }

    #[derive(Clone, Copy, Default)]
    pub struct BranchStat {
        taken_pred_taken_count: usize,
        taken_pred_untaken_count: usize,
        untaken_pred_taken_count: usize,
        untaken_pred_untaken_count: usize,
    }

    impl BranchStat {
        pub fn update_stat(&mut self, predicted: bool, actual: bool) {
            if predicted {
                if actual {
                    self.taken_pred_taken_count += 1;
                } else {
                    self.untaken_pred_taken_count += 1;
                }
            } else if actual {
                self.taken_pred_untaken_count += 1;
            } else {
                self.untaken_pred_untaken_count += 1;
            }
        }
    }

    impl Stat for BranchStat {
        fn view(&self, _: usize) -> Box<dyn StatView + '_> {
            Box::new(BranchStatView::new(self))
        }
    }

    pub struct BranchStatView<'a> {
        stat: &'a BranchStat,
    }

    impl<'a> BranchStatView<'a> {
        pub fn new(stat: &'a BranchStat) -> Self {
            Self { stat }
        }
    }

    impl StatView for BranchStatView<'_> {
        fn header(&self) -> &'static str {
            "branch count"
        }
        fn width(&self) -> usize {
            33
        }
    }

    impl fmt::Display for BranchStatView<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let tt = self.stat.taken_pred_taken_count;
            let tu = self.stat.taken_pred_untaken_count;
            let ut = self.stat.untaken_pred_taken_count;
            let uu = self.stat.untaken_pred_untaken_count;
            let taken = tt + tu;
            let untaken = ut + uu;
            let pred_correct = tt + uu;
            let total = taken + untaken;
            let t_pct = format!("{:.6}", 100. * taken as f64 / total as f64);
            let u_pct = format!("{:.6}", 100. * untaken as f64 / total as f64);
            let p_pct = format!("{:.6}", 100. * pred_correct as f64 / total as f64);
            writeln!(f, "         taken: {taken:>10} ({t_pct:>8}%)")?;
            writeln!(f, "       untaken: {untaken:>10} ({u_pct:>8}%)")?;
            writeln!(f, "  pred correct: {pred_correct:>10} ({p_pct:>8}%)")
        }
    }

    #[derive(Default, Clone, Copy)]
    pub struct CacheStat {
        hit_count: usize,
        miss_count: usize,
    }

    impl CacheStat {
        pub fn update_stat(&mut self, result: bool) {
            if result {
                self.hit_count += 1;
            } else {
                self.miss_count += 1;
            }
        }
    }

    impl Stat for CacheStat {
        fn view(&self, _: usize) -> Box<dyn StatView + '_> {
            Box::new(CacheStatView::new(self))
        }
    }

    pub struct CacheStatView<'a> {
        stat: &'a CacheStat,
    }

    impl<'a> CacheStatView<'a> {
        pub fn new(stat: &'a CacheStat) -> Self {
            Self { stat }
        }
    }

    impl StatView for CacheStatView<'_> {
        fn header(&self) -> &'static str {
            "cache stat"
        }
        fn width(&self) -> usize {
            33
        }
    }

    impl fmt::Display for CacheStatView<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let hit = self.stat.hit_count;
            let miss = self.stat.miss_count;
            let total = hit + miss;
            let hit_pct = format!("{:.6}", 100. * hit as f64 / total as f64);
            let miss_pct = format!("{:.6}", 100. * miss as f64 / total as f64);
            writeln!(f, "      hit: {hit:>10} ({hit_pct:>8}%)")?;
            writeln!(f, "     miss: {miss:>10} ({miss_pct:>8}%)")
        }
    }
}

impl<I: Input, O: Output> Cpu<I, O> {
    fn init_memory(&mut self, mem: &[u8], instr_mem_range: Range<u32>) {
        self.memory.init_from_slice(mem, instr_mem_range);
    }
    pub fn get_pc(&self) -> Pc {
        self.pc
    }
    pub fn get_pc_addr(&self) -> Addr {
        self.pc.into_addr()
    }
    fn instr_fetch(&mut self) -> Result<InstrFetchOutput> {
        let old_pc = self.pc;
        let bin = self.memory.get_from_pc(old_pc)?;
        self.pc.incr();
        let pc_plus4 = self.pc;
        Ok(InstrFetchOutput {
            id_in: InstrDecodeInput { bin },
            old_pc,
            pc_plus4,
        })
    }
    fn instr_decode(
        &self,
        InstrDecodeInput { bin }: &InstrDecodeInput,
    ) -> Result<Instr<RegId, RegId, FRegId, FRegId>> {
        Ok(Instr::decode_from(*bin)?)
    }
    fn reg_fetch(
        &self,
        RegFetchInput {
            instr,
            old_pc,
            pc_plus4,
        }: RegFetchInput,
    ) -> ExecuteInput {
        use IOInstr::*;
        use Instr::*;
        let instr = match instr {
            R {
                instr,
                rd,
                rs1,
                rs2,
            } => R {
                instr,
                rd,
                rs1: self.reg_file.get(rs1),
                rs2: self.reg_file.get(rs2),
            },
            I {
                instr,
                rd,
                rs1,
                imm,
            } => I {
                instr,
                rd,
                rs1: self.reg_file.get(rs1),
                imm,
            },
            S {
                instr,
                rs1,
                rs2,
                imm,
            } => S {
                instr,
                rs1: self.reg_file.get(rs1),
                rs2: self.reg_file.get(rs2),
                imm,
            },
            B {
                instr,
                rs1,
                rs2,
                imm,
            } => B {
                instr,
                rs1: self.reg_file.get(rs1),
                rs2: self.reg_file.get(rs2),
                imm,
            },
            P {
                instr,
                rs1,
                imm,
                imm2,
            } => P {
                instr,
                rs1: self.reg_file.get(rs1),
                imm,
                imm2,
            },
            J { instr, rd, imm } => J { instr, rd, imm },
            IO(Outb { rs }) => IO(Outb {
                rs: self.reg_file.get(rs),
            }),
            IO(Inw { rd }) => IO(Inw { rd }),
            IO(Finw { rd }) => IO(Finw { rd }),
            F(f) => {
                use FInstr::*;
                F(match f {
                    E {
                        instr,
                        rd,
                        rs1,
                        rs2,
                    } => E {
                        instr,
                        rd,
                        rs1: self.reg_file.get_f(rs1),
                        rs2: self.reg_file.get_f(rs2),
                    },
                    G {
                        instr,
                        rd,
                        rs1,
                        rs2,
                        rs3,
                    } => G {
                        instr,
                        rd,
                        rs1: self.reg_file.get_f(rs1),
                        rs2: self.reg_file.get_f(rs2),
                        rs3: self.reg_file.get_f(rs3),
                    },
                    H { instr, rd, rs1 } => H {
                        instr,
                        rd,
                        rs1: self.reg_file.get_f(rs1),
                    },
                    K {
                        instr,
                        rd,
                        rs1,
                        rs2,
                    } => K {
                        instr,
                        rd,
                        rs1: self.reg_file.get_f(rs1),
                        rs2: self.reg_file.get_f(rs2),
                    },
                    X { instr, rd, rs1 } => X {
                        instr,
                        rd,
                        rs1: self.reg_file.get(rs1),
                    },
                    Y { instr, rd, rs1 } => Y {
                        instr,
                        rd,
                        rs1: self.reg_file.get_f(rs1),
                    },
                    W {
                        instr,
                        rs1,
                        rs2,
                        imm,
                    } => W {
                        instr,
                        rs1: self.reg_file.get_f(rs1),
                        rs2: self.reg_file.get_f(rs2),
                        imm,
                    },
                    V { instr, rs1, imm } => V {
                        instr,
                        rs1: self.reg_file.get_f(rs1),
                        imm,
                    },
                    Flw { rd, rs1, imm } => Flw {
                        rd,
                        rs1: self.reg_file.get(rs1),
                        imm,
                    },
                    Fsw { rs2, rs1, imm } => Fsw {
                        rs2: self.reg_file.get_f(rs2),
                        rs1: self.reg_file.get(rs1),
                        imm,
                    },
                })
            }
            Misc(MiscInstr::End) => Misc(MiscInstr::End),
        };
        ExecuteInput {
            instr,
            old_pc,
            pc_plus4,
        }
    }
    fn execute(&mut self, ex_in: ExecuteInput) -> Result<ExecuteOutput> {
        use Instr::*;
        let ExecuteInput {
            instr,
            old_pc,
            pc_plus4,
        } = ex_in;
        Ok(match instr {
            R {
                instr,
                rd,
                rs1,
                rs2,
            } => {
                use RInstr::*;
                cfg_if::cfg_if! {
                    if #[cfg(feature = "isa_2nd")] {
                        let val = match instr {
                            Add => rs1.wrapping_add(rs2),
                            Xor => rs1 ^ rs2,
                            Min => cmp::min(rs1 as i32, rs2 as i32) as u32,
                            Max => cmp::max(rs1 as i32, rs2 as i32) as u32,
                        };
                    }
                    else {
                        let val = match instr {
                            Add => rs1.wrapping_add(rs2),
                            Sub => rs1.wrapping_sub(rs2),
                            Xor => rs1 ^ rs2,
                            Or => rs1 | rs2,
                            And => rs1 & rs2,
                            Sll => rs1 << rs2,
                            Sra => rs1 >> rs2,
                            Slt => u32::from((rs1 as i32) < (rs2 as i32)),
                        };
                    }
                }

                ExecuteOutput {
                    wb_in: Some(WriteBackInput::I { id: rd, val }),
                    #[cfg(feature = "time_predict")]
                    cycles: 1,
                    ..Default::default()
                }
            }
            I {
                instr,
                rd,
                rs1,
                imm,
            } => {
                let mut ret = ExecuteOutput {
                    ..Default::default()
                };
                use IInstr::*;
                let val = match instr {
                    Addi => rs1.wrapping_add(imm),
                    Xori => rs1 ^ imm,
                    #[cfg(not(feature = "isa_2nd"))]
                    Ori => rs1 | imm,
                    #[cfg(not(feature = "isa_2nd"))]
                    Andi => rs1 & imm,
                    Slli => rs1 << imm,
                    #[cfg(not(feature = "isa_2nd"))]
                    Slti => u32::from((rs1 as i32) < (imm as i32)),
                    Lw => {
                        ret.ma_in = Some(MemoryAccessInput::IMem {
                            id: rd,
                            addr: rs1.wrapping_add(imm) as usize,
                        });
                        return Ok(ret);
                    }
                    Jalr => {
                        #[cfg(feature = "time_predict")]
                        {
                            ret.flush = true;
                        }
                        ret.new_pc = Some(rs1.wrapping_add(imm) as usize);
                        pc_plus4
                    }
                };
                ret.wb_in = Some(WriteBackInput::I { id: rd, val });
                ret
            }
            S {
                instr,
                rs1,
                rs2,
                imm,
            } => {
                use SInstr::*;
                let val = match instr {
                    Sw => rs2,
                };
                let addr = rs1.wrapping_add(imm) as usize;
                ExecuteOutput {
                    ma_in: Some(MemoryAccessInput::I { addr, val }),
                    #[cfg(feature = "time_predict")]
                    cycles: 1,
                    ..Default::default()
                }
            }
            B {
                instr,
                rs1,
                rs2,
                imm,
            } => {
                use BInstr::*;
                let cond = match instr {
                    Beq => rs1 == rs2,
                    Bne => rs1 != rs2,
                    Blt => (rs1 as i32) < (rs2 as i32),
                    Bge => (rs1 as i32) >= (rs2 as i32),
                    #[cfg(feature = "isa_2nd")]
                    Bxor => (rs1 ^ rs2) != 0,
                    #[cfg(feature = "isa_2nd")]
                    Bxnor => (rs1 ^ rs2) == 0,
                };
                let new_pc = if cond {
                    Some(old_pc.wrapping_add(imm) as usize)
                } else {
                    None
                };
                #[cfg(feature = "stat")]
                let prediction_result = self.branch_predictor.predict(self.pc.into_usize());
                #[cfg(feature = "stat")]
                self.branch_predictor
                    .update_state(self.pc.into_usize(), cond);
                #[cfg(feature = "stat")]
                self.b_stat.update_stat(prediction_result, cond);
                ExecuteOutput {
                    new_pc,
                    #[cfg(feature = "time_predict")]
                    flush: prediction_result != cond,
                    #[cfg(feature = "time_predict")]
                    cycles: 1,
                    ..Default::default()
                }
            }
            #[cfg(feature = "isa_2nd")]
            P {
                instr,
                rs1,
                imm,
                imm2,
            } => {
                use PInstr::*;
                let cond = match instr {
                    Beqi => rs1 == imm2,
                    Bnei => rs1 != imm2,
                    Blti => (rs1 as i32) < (imm2 as i32),
                    Bgei => (rs1 as i32) >= (imm2 as i32),
                    Bgti => (rs1 as i32) > (imm2 as i32),
                    Blei => (rs1 as i32) <= (imm2 as i32),
                };
                let new_pc = if cond {
                    Some(old_pc.wrapping_add(imm) as usize)
                } else {
                    None
                };
                #[cfg(feature = "stat")]
                let prediction_result = self.branch_predictor.predict(self.pc.into_usize());
                #[cfg(feature = "stat")]
                self.branch_predictor
                    .update_state(self.pc.into_usize(), cond);
                #[cfg(feature = "stat")]
                self.b_stat.update_stat(prediction_result, cond);
                ExecuteOutput {
                    new_pc,
                    #[cfg(feature = "time_predict")]
                    flush: prediction_result != cond,
                    #[cfg(feature = "time_predict")]
                    cycles: 1,
                    ..Default::default()
                }
            }
            #[cfg(not(feature = "isa_2nd"))]
            P { .. } => {
                unreachable!()
            }
            J { instr, rd, imm } => {
                use JInstr::*;
                match instr {
                    Jal => {
                        let new_pc = Some(old_pc.wrapping_add(imm) as usize);
                        ExecuteOutput {
                            wb_in: Some(WriteBackInput::I {
                                id: rd,
                                val: pc_plus4,
                            }),
                            new_pc,
                            #[cfg(feature = "time_predict")]
                            cycles: 1,
                            ..Default::default()
                        }
                    }
                }
            }
            IO(io) => {
                use IOInstr::*;
                match io {
                    Outb { rs } => {
                        self.output.outb(rs as u8)?;
                        ExecuteOutput {
                            #[cfg(feature = "time_predict")]
                            cycles: 1,
                            ..Default::default()
                        }
                    }
                    Inw { rd } => {
                        let val = self.input.inw()?;
                        ExecuteOutput {
                            wb_in: Some(WriteBackInput::I { id: rd, val }),
                            #[cfg(feature = "time_predict")]
                            cycles: 1,
                            ..Default::default()
                        }
                    }
                    Finw { rd } => {
                        let val = self.input.finw()?;
                        ExecuteOutput {
                            wb_in: Some(WriteBackInput::F { id: rd, val }),
                            #[cfg(feature = "time_predict")]
                            cycles: 1,
                            ..Default::default()
                        }
                    }
                }
            }
            F(f) => {
                use FInstr::*;
                match f {
                    E {
                        instr,
                        rd,
                        rs1,
                        rs2,
                    } => {
                        use EInstr::*;
                        let val = match instr {
                            Fadd => rs1 + rs2,
                            Fsub => rs1 - rs2,
                            Fmul => fpu::fmul(rs1, rs2),
                            Fdiv => fpu::fdiv(rs1, rs2),
                            Fsgnj => rs1.copysign(rs2),
                            Fsgnjn => rs1.copysign(-rs2),
                            Fsgnjx => rs1.copysign(rs1.signum() * rs2.signum()),
                        };

                        ExecuteOutput {
                            wb_in: Some(WriteBackInput::F { id: rd, val }),
                            #[cfg(feature = "time_predict")]
                            use_fpu: true,
                            #[cfg(feature = "time_predict")]
                            cycles: match instr {
                                Fadd => 5,
                                Fsub => 5,
                                Fmul => 2,
                                Fdiv => 11,
                                Fsgnj => 1,
                                Fsgnjn => 1,
                                Fsgnjx => 1,
                            },
                            ..Default::default()
                        }
                    }
                    #[cfg(feature = "isa_2nd")]
                    G {
                        instr,
                        rd,
                        rs1,
                        rs2,
                        rs3,
                    } => {
                        use GInstr::*;
                        let val = match instr {
                            Fmadd => rs1 * rs2 + rs3,
                            Fmsub => rs1 * rs2 - rs3,
                            Fnmadd => -rs1 * rs2 + rs3,
                            Fnmsub => -rs1 * rs2 - rs3,
                        };

                        ExecuteOutput {
                            wb_in: Some(WriteBackInput::F { id: rd, val }),
                            #[cfg(feature = "time_predict")]
                            use_fpu: true,
                            #[cfg(feature = "time_predict")]
                            cycles: 7,
                            ..Default::default()
                        }
                    }
                    #[cfg(not(feature = "isa_2nd"))]
                    G { .. } => {
                        unreachable!()
                    }
                    H { instr, rd, rs1 } => {
                        use HInstr::*;
                        let val = match instr {
                            Fsqrt => fpu::fsqrt(rs1),
                            Fhalf => fpu::fhalf(rs1),
                            Ffloor => fpu::ffloor(rs1),
                            #[cfg(feature = "isa_2nd")]
                            Ffrac => fpu::ffrac(rs1),
                            #[cfg(feature = "isa_2nd")]
                            Finv => fpu::finv(rs1),
                        };
                        ExecuteOutput {
                            wb_in: Some(WriteBackInput::F { id: rd, val }),
                            #[cfg(feature = "time_predict")]
                            use_fpu: true,
                            #[cfg(feature = "time_predict")]
                            cycles: match instr {
                                Fsqrt => 8,
                                Fhalf => 1,
                                Ffloor => 8,
                                #[cfg(feature = "isa_2nd")]
                                Ffrac => unreachable!(), // frac is not supported by core
                                #[cfg(feature = "isa_2nd")]
                                Finv => 8,
                            },
                            ..Default::default()
                        }
                    }
                    K {
                        instr,
                        rd,
                        rs1,
                        rs2,
                    } => {
                        use KInstr::*;
                        let val = match instr {
                            Flt => u32::from(rs1 < rs2),
                        };
                        ExecuteOutput {
                            wb_in: Some(WriteBackInput::I { id: rd, val }),
                            #[cfg(feature = "time_predict")]
                            cycles: 1,
                            ..Default::default()
                        }
                    }
                    X { instr, rd, rs1 } => {
                        use XInstr::*;
                        let val = match instr {
                            Fitof => fpu::fcvtsw(rs1 as i32),
                        };
                        ExecuteOutput {
                            wb_in: Some(WriteBackInput::F { id: rd, val }),
                            #[cfg(feature = "time_predict")]
                            use_fpu: true,
                            #[cfg(feature = "time_predict")]
                            cycles: 4,
                            ..Default::default()
                        }
                    }
                    Y { instr, rd, rs1 } => {
                        use YInstr::*;
                        let val = match instr {
                            Fiszero => u32::from(rs1 == 0.0),
                            Fispos => u32::from(rs1 > 0.0),
                            Fisneg => u32::from(rs1 < 0.0),
                            Fftoi => fpu::fcvtws(rs1) as u32,
                        };
                        ExecuteOutput {
                            wb_in: Some(WriteBackInput::I { id: rd, val }),
                            #[cfg(feature = "time_predict")]
                            use_fpu: true,
                            #[cfg(feature = "time_predict")]
                            cycles: match instr {
                                Fiszero => 1,
                                Fispos => 1,
                                Fisneg => 1,
                                Fftoi => 2,
                            },
                            ..Default::default()
                        }
                    }
                    W {
                        instr,
                        rs1,
                        rs2,
                        imm,
                    } => {
                        use WInstr::*;
                        let cond = match instr {
                            Fblt => rs1 < rs2,
                            Fbge => rs1 >= rs2,
                        };
                        let new_pc = if cond {
                            Some(old_pc.wrapping_add(imm) as usize)
                        } else {
                            None
                        };
                        #[cfg(feature = "stat")]
                        let prediction_result = self.branch_predictor.predict(self.pc.into_usize());
                        #[cfg(feature = "stat")]
                        self.branch_predictor
                            .update_state(self.pc.into_usize(), cond);
                        #[cfg(feature = "stat")]
                        self.b_stat.update_stat(prediction_result, cond);
                        ExecuteOutput {
                            new_pc,
                            #[cfg(feature = "time_predict")]
                            flush: prediction_result != cond,
                            #[cfg(feature = "time_predict")]
                            cycles: 1,
                            ..Default::default()
                        }
                    }
                    V { instr, rs1, imm } => {
                        use VInstr::*;
                        let cond = match instr {
                            Fbeqz => rs1 == 0.0,
                            Fbnez => rs1 != 0.0,
                        };
                        let new_pc = if cond {
                            Some(old_pc.wrapping_add(imm) as usize)
                        } else {
                            None
                        };
                        #[cfg(feature = "stat")]
                        let prediction_result = self.branch_predictor.predict(self.pc.into_usize());
                        #[cfg(feature = "stat")]
                        self.branch_predictor
                            .update_state(self.pc.into_usize(), cond);
                        #[cfg(feature = "stat")]
                        self.b_stat.update_stat(prediction_result, cond);
                        ExecuteOutput {
                            new_pc,
                            #[cfg(feature = "time_predict")]
                            flush: prediction_result != cond,
                            #[cfg(feature = "time_predict")]
                            cycles: 1,
                            ..Default::default()
                        }
                    }
                    Flw { rd, rs1, imm } => ExecuteOutput {
                        ma_in: Some(MemoryAccessInput::FMem {
                            id: rd,
                            addr: rs1.wrapping_add(imm) as usize,
                        }),
                        ..Default::default()
                    },
                    Fsw { rs2, rs1, imm } => {
                        let val = rs2;
                        let addr = rs1.wrapping_add(imm) as usize;
                        ExecuteOutput {
                            ma_in: Some(MemoryAccessInput::F { addr, val }),
                            #[cfg(feature = "time_predict")]
                            cycles: 1,
                            ..Default::default()
                        }
                    }
                }
            }
            Misc(MiscInstr::End) => ExecuteOutput {
                #[cfg(feature = "time_predict")]
                cycles: 1,
                end: true,
                ..Default::default()
            },
        })
    }
    fn memory_access(
        &mut self,
        ma_in: MemoryAccessInput,
        spied: &mut Option<SpyResult>,
    ) -> Result<MemoryAccessOutput> {
        #[cfg(feature = "time_predict")]
        fn use_bram(addr: usize) -> bool {
            !(BRAM_WORD_SIZE..(RAM_BYTE_SIZE >> 2) - STACK_WORD_SIZE).contains(&addr)
        }
        let mut res = MemoryAccessOutput {
            ..Default::default()
        };
        match ma_in {
            MemoryAccessInput::I { addr, val } => {
                #[cfg(feature = "time_predict")]
                {
                    res.use_bram = use_bram(addr);
                    if !res.use_bram {
                        res.cache_hit = self.cache.access_cache(addr)
                    };
                }
                #[cfg(not(feature = "time_predict"))]
                #[cfg(feature = "stat")]
                {
                    res.cache_hit = self.cache.access_cache(addr);
                }
                self.memory.set(addr, val, spied)?;
            }
            MemoryAccessInput::F { addr, val } => {
                #[cfg(feature = "time_predict")]
                {
                    res.use_bram = use_bram(addr);
                    if !res.use_bram {
                        res.cache_hit = self.cache.access_cache(addr)
                    };
                }
                #[cfg(not(feature = "time_predict"))]
                #[cfg(feature = "stat")]
                {
                    res.cache_hit = self.cache.access_cache(addr);
                }
                self.memory.set_f(addr, val, spied)?;
            }
            MemoryAccessInput::IMem { id, addr } => {
                #[cfg(feature = "time_predict")]
                {
                    res.use_bram = use_bram(addr);
                    if !res.use_bram {
                        res.cache_hit = self.cache.access_cache(addr)
                    };
                }
                #[cfg(not(feature = "time_predict"))]
                #[cfg(feature = "stat")]
                {
                    res.cache_hit = self.cache.access_cache(addr);
                }
                let val = self.memory.get_i(addr, spied)?.get_unchecked();
                res.wb_in = Some(WriteBackInput::I { id, val });
            }
            MemoryAccessInput::FMem { id, addr } => {
                #[cfg(feature = "time_predict")]
                {
                    res.use_bram = use_bram(addr);
                    if !res.use_bram {
                        res.cache_hit = self.cache.access_cache(addr)
                    };
                }
                #[cfg(not(feature = "time_predict"))]
                #[cfg(feature = "stat")]
                {
                    res.cache_hit = self.cache.access_cache(addr);
                }
                let val = self.memory.get_f(addr, spied)?;
                res.wb_in = Some(WriteBackInput::F { id, val });
            }
        }
        #[cfg(feature = "time_predict")]
        if !res.use_bram {
            self.c_stat.update_stat(res.cache_hit);
        }
        #[cfg(feature = "time_predict")]
        {
            res.cycles = if res.use_bram {
                1
            } else if res.cache_hit {
                2
            } else {
                DDR2_ACCESS_CYCLES
            };
        }

        #[cfg(not(feature = "time_predict"))]
        #[cfg(feature = "stat")]
        self.c_stat.update_stat(res.cache_hit);

        Ok(res)
    }
    fn write_back(&mut self, wb_in: WriteBackInput) {
        use WriteBackInput::*;
        match wb_in {
            I { id, val } => self.reg_file.set(id, val),
            F { id, val } => self.reg_file.set_f(id, val),
        }
    }
    #[cfg(feature = "time_predict")]
    fn calc_stall_cycles(&self, instr: &Instr<RegId, RegId, FRegId, FRegId>) -> usize {
        fn regid_is_included_in_srcs(
            instr: &Instr<RegId, RegId, FRegId, FRegId>,
            regid: &RegId,
        ) -> bool {
            match instr {
                Instr::R {
                    instr: _,
                    rd: _,
                    rs1,
                    rs2,
                } => rs1 == regid || rs2 == regid,
                Instr::I {
                    instr: _,
                    rd: _,
                    rs1,
                    imm: _,
                } => rs1 == regid,
                Instr::S {
                    instr: _,
                    rs1,
                    rs2,
                    imm: _,
                } => rs1 == regid || rs2 == regid,
                Instr::B {
                    instr: _,
                    rs1,
                    rs2,
                    imm: _,
                } => rs1 == regid || rs2 == regid,
                Instr::P {
                    instr: _,
                    rs1,
                    imm: _,
                    imm2: _,
                } => rs1 == regid,
                Instr::J {
                    instr: _,
                    rd: _,
                    imm: _,
                } => false,
                Instr::IO(ioinstr) => match ioinstr {
                    IOInstr::Outb { rs } => rs == regid,
                    IOInstr::Inw { rd: _ } => false,
                    IOInstr::Finw { rd: _ } => false,
                },
                Instr::F(finstr) => match finstr {
                    FInstr::X {
                        instr: _,
                        rd: _,
                        rs1,
                    } => rs1 == regid,
                    FInstr::Flw { rd: _, rs1, imm: _ } => rs1 == regid,
                    FInstr::Fsw {
                        rs2: _,
                        rs1,
                        imm: _,
                    } => rs1 == regid,
                    _ => false,
                },
                Instr::Misc(_) => false,
            }
        }

        fn fregid_is_included_in_srcs(
            instr: &Instr<RegId, RegId, FRegId, FRegId>,
            fregid: &FRegId,
        ) -> bool {
            match instr {
                Instr::F(finstr) => match finstr {
                    FInstr::E {
                        instr: _,
                        rd: _,
                        rs1,
                        rs2,
                    } => rs1 == fregid || rs2 == fregid,
                    FInstr::G {
                        instr: _,
                        rd: _,
                        rs1,
                        rs2,
                        rs3,
                    } => rs1 == fregid || rs2 == fregid || rs3 == fregid,
                    FInstr::H {
                        instr: _,
                        rd: _,
                        rs1,
                    } => rs1 == fregid,
                    FInstr::K {
                        instr: _,
                        rd: _,
                        rs1,
                        rs2,
                    } => rs1 == fregid || rs2 == fregid,
                    FInstr::X {
                        instr: _,
                        rd: _,
                        rs1: _,
                    } => false,
                    FInstr::Y {
                        instr: _,
                        rd: _,
                        rs1,
                    } => rs1 == fregid,
                    FInstr::W {
                        instr: _,
                        rs1,
                        rs2,
                        imm: _,
                    } => rs1 == fregid || rs2 == fregid,
                    FInstr::V {
                        instr: _,
                        rs1,
                        imm: _,
                    } => rs1 == fregid,
                    FInstr::Flw {
                        rd: _,
                        rs1: _,
                        imm: _,
                    } => false,
                    FInstr::Fsw {
                        rs2,
                        rs1: _,
                        imm: _,
                    } => rs2 == fregid,
                },
                _ => false,
            }
        }

        let ex_pipeline_stat = self.pipeline_state.index(0);
        let stall_cycles_with_ex: usize = if let Some(ex_pipeline_stat) = ex_pipeline_stat {
            if let Some(result_ready_stage) = &ex_pipeline_stat.result_ready_stage {
                let hazard = (ex_pipeline_stat.write_back_id.is_some()
                    && regid_is_included_in_srcs(instr, &ex_pipeline_stat.write_back_id.unwrap()))
                    || (ex_pipeline_stat.float_write_back_id.is_some()
                        && fregid_is_included_in_srcs(
                            instr,
                            &ex_pipeline_stat.float_write_back_id.unwrap(),
                        ));

                if hazard {
                    match result_ready_stage {
                        PipelineStage::WriteBack => 2,
                        PipelineStage::MemoryAccess => 1,
                        _ => 0,
                    }
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        let ma_pipeline_stat = self.pipeline_state.index(1);
        let stall_cycles_with_ma: usize = if let Some(ma_pipeline_stat) = ma_pipeline_stat {
            if let Some(result_ready_stage) = &ma_pipeline_stat.result_ready_stage {
                let hazard = (ma_pipeline_stat.write_back_id.is_some()
                    && regid_is_included_in_srcs(instr, &ma_pipeline_stat.write_back_id.unwrap()))
                    || (ma_pipeline_stat.float_write_back_id.is_some()
                        && fregid_is_included_in_srcs(
                            instr,
                            &ma_pipeline_stat.float_write_back_id.unwrap(),
                        ));

                if hazard {
                    match result_ready_stage {
                        PipelineStage::WriteBack => 1,
                        _ => 0,
                    }
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        usize::max(stall_cycles_with_ex, stall_cycles_with_ma)
    }
    #[cfg(feature = "time_predict")]
    fn push_instr_to_pipeline_and_get_cycles(&mut self, instr: Option<PipelineStat>) -> usize {
        let ex_cycles_of_first_instr = if let Some(instr_inner) = &instr {
            instr_inner.ex_cycles
        } else {
            1
        };
        let ma_cycles_of_first_instr = if let Some(instr_inner) = &self.pipeline_state.index(0) {
            instr_inner.ma_cycles
        } else {
            1
        };
        self.pipeline_state.pop_back();
        self.pipeline_state.push_front(instr);
        assert_eq!(5, self.pipeline_state.len(), "Pipeline is not filled");
        usize::max(ex_cycles_of_first_instr, ma_cycles_of_first_instr)
    }
    pub fn cycle_one_full(&mut self, do_trace: bool) -> Result<CycleResult> {
        let mut res = CycleResult {
            ..Default::default()
        };
        let mut spied = None;
        let id_rf_in = self.instr_fetch()?;
        let instr = self.instr_decode(&id_rf_in.id_in)?;
        if do_trace {
            res.trace = Some(ExecutionTrace {
                pc: id_rf_in.old_pc,
                undecoded_instr: id_rf_in.id_in.bin,
                decoded_instr: instr.clone(),
            })
        }

        #[cfg(feature = "stat")]
        self.i_stat.encounter_instr(&instr);

        let ex_in = self.reg_fetch(RegFetchInput {
            instr: instr.clone(),
            old_pc: id_rf_in.old_pc.into_inner(),
            pc_plus4: id_rf_in.pc_plus4.into_inner(),
        });
        let ExecuteOutput {
            ma_in,
            mut wb_in,
            new_pc,
            end,
            #[cfg(feature = "time_predict")]
            flush,
            #[cfg(feature = "time_predict")]
                cycles: ex_cycles,
            #[cfg(feature = "time_predict")]
            use_fpu,
        } = self.execute(ex_in)?;
        if end {
            res.flow = ControlFlow::Exit;
            return Ok(res);
        }
        if let Some(val) = new_pc {
            self.pc = Pc::new(val as u32);
        }
        #[cfg(feature = "time_predict")]
        let mut result_ready_stage = if use_fpu {
            PipelineStage::MemoryAccess
        } else {
            PipelineStage::Execute
        };
        #[cfg(feature = "time_predict")]
        let mut ma_cycles: usize = 1;
        if let Some(ma_in) = ma_in {
            let ma_out = self.memory_access(ma_in, &mut spied)?;
            #[cfg(feature = "time_predict")]
            {
                ma_cycles = ma_out.cycles;
            }
            if let Some(spied) = spied {
                res.flow = ControlFlow::Break(BreakReason::Spy(spied));
            }
            if ma_out.wb_in.is_some() {
                #[cfg(feature = "time_predict")]
                {
                    result_ready_stage = if ma_out.use_bram {
                        PipelineStage::WriteBack
                    } else {
                        PipelineStage::MemoryAccess
                    };
                }
                wb_in = ma_out.wb_in;
            }
        }
        if let Some(wb_in) = wb_in {
            self.write_back(wb_in);
        }

        #[cfg(feature = "time_predict")]
        {
            // Update pipeline state
            let stall_cycles = self.calc_stall_cycles(&instr);
            for _ in 0..stall_cycles {
                res.cycles += self.push_instr_to_pipeline_and_get_cycles(None);
            }
            res.cycles += self.push_instr_to_pipeline_and_get_cycles(Some(PipelineStat {
                ex_cycles,
                ma_cycles,
                result_ready_stage: Some(result_ready_stage),
                write_back_id: if let Some(WriteBackInput::I { id, val: _ }) = wb_in {
                    Some(id)
                } else {
                    None
                },
                float_write_back_id: if let Some(WriteBackInput::F { id, val: _ }) = wb_in {
                    Some(id)
                } else {
                    None
                },
            }));
            if flush {
                res.cycles += self.push_instr_to_pipeline_and_get_cycles(None);
                res.cycles += self.push_instr_to_pipeline_and_get_cycles(None);
            }
        }
        Ok(res)
    }

    pub fn get_freg(&self, id: FRegId) -> f32 {
        self.reg_file.get_f(id)
    }

    pub fn get_reg(&self, id: RegId) -> u32 {
        self.reg_file.get(id)
    }

    pub fn get_mem(&self, addr: Addr) -> std::result::Result<TypedU32, MemoryAccessError> {
        self.memory.get(addr.inner(), &mut None)
    }

    pub fn get_mem_pc(&self, addr: Pc) -> Result<u32> {
        Ok(self.memory.get_from_pc(addr)?)
    }

    pub fn get_regfile_view(&self, k: ShowRegFileKind, chunk_size: usize) -> RegFileView<'_> {
        self.reg_file.get_view(k, chunk_size)
    }

    pub fn add_mem_spy(&mut self, k: SpyWatchKind, u: SpyUnit) {
        self.memory.add_spy(k, u)
    }

    pub fn remove_mem_spy(&mut self, k: SpyWatchKind, u: SpyUnit) {
        self.memory.remove_spy(k, u)
    }
}

pub struct ExecutionTrace {
    pub pc: Pc,
    pub undecoded_instr: u32,
    pub decoded_instr: DecodedInstr,
}

#[derive(Default)]
pub struct CycleResult {
    pub cycles: usize,
    pub trace: Option<ExecutionTrace>,
    pub flow: ControlFlow,
}

pub enum BreakReason {
    Spy(SpyResult),
}

#[derive(Default)]
pub enum ControlFlow {
    #[default]
    Continue,
    Break(BreakReason),
    Exit,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_ftoi() {
        assert_eq!(24i32, (23.7f32.round() as i32));
    }
}
