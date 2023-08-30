use std::{collections::HashMap, fmt};

use anyhow::Result;

use crate::{
    breakpoint::BreakPoint,
    common::{ExecuteMode, Pc, SimulationOption, SpyResult, Watchings},
    cpu::{self, Cpu, CycleResult, ExecutionTrace, RuntimeError},
    debug_symbol::DebugSymbol,
    instr::{self, DecodedInstr, Instr},
    io::{Input, Output},
    memory::Addr,
    reg_file::{RegFileView, ShowRegFileKind},
    register::{FRegId, RegId},
    ty::{Typed, TypedU32},
};

#[cfg(feature = "stat")]
use crate::stat::{AddStats, Stats};

#[cfg(feature = "time_predict")]
const CPU_CLOCK_FREQ: usize = 183_333_333;
#[cfg(feature = "time_predict")]
const CPU_BAUDRATE: usize = 2_304_000;

pub struct Simulator<I, O> {
    cpu: Cpu<I, O>,
    #[cfg(feature = "time_predict")]
    elapsed_clocks: usize,
    cycle: usize,
    debug_symbol: DebugSymbol,
    fatal_error: Option<RuntimeError>,
    #[cfg(feature = "stat")]
    stat_builder: stat::SimStatBuilder,
}

pub struct SimOutput<O> {
    pub cpu_output: O,
}

impl<I: Input, O: Output> Simulator<I, O> {
    pub fn new(mem: &[u8], input: I, output: O) -> Result<Self> {
        #[cfg(feature = "stat")]
        #[cfg(feature = "time_predict")]
        let mut stat_builder = self::stat::SimStatBuilder::new();
        #[cfg(not(feature = "time_predict"))]
        let stat_builder = self::stat::SimStatBuilder::new();
        #[cfg(feature = "time_predict")]
        let (data_sec_size, text_sec_size) = Cpu::<I, O>::get_data_and_text_len(mem);
        #[cfg(feature = "time_predict")]
        stat_builder.instr_file_len(data_sec_size + text_sec_size);
        Ok(Self {
            cpu: Cpu::new(mem, input, output)?,
            #[cfg(feature = "time_predict")]
            elapsed_clocks: 0,
            cycle: 0,
            debug_symbol: Default::default(),
            fatal_error: None,
            #[cfg(feature = "stat")]
            stat_builder,
        })
    }
    pub fn provide_dbg_symb(&mut self, debug_symbol: DebugSymbol) {
        if !debug_symbol.is_empty() {
            log::info!("debug symbol provided.");
            self.debug_symbol.merge(debug_symbol)
        }
    }
    pub fn into_output(self) -> SimOutput<O> {
        let cpu_output = self.cpu.into_output();
        SimOutput {
            cpu_output: cpu_output.value,
        }
    }
}

impl<I, O> Simulator<I, O> {
    #[cfg(feature = "stat")]
    pub fn collect_stat(&self) -> Stats {
        let mut ss = Stats::default();
        self.add_stats(&mut ss);
        ss
    }
}

#[cfg(feature = "stat")]
impl<I, O> AddStats for Simulator<I, O> {
    fn add_stats(&self, buf: &mut Stats) {
        buf.push(Box::new(self.stat_builder.finish()));
        self.cpu.add_stats(buf);
    }
}

#[cfg(feature = "stat")]
mod stat {
    use crate::stat::*;

    use super::*;
    use std::time;

    pub struct SimStatBuilder {
        begin: time::Instant,
        #[cfg(feature = "time_predict")]
        instr_file_len: Option<u32>,
        #[cfg(feature = "time_predict")]
        elapsed_clocks: Option<usize>,
        cycle: Option<usize>,
        elapsed: Option<time::Duration>,
    }

    impl SimStatBuilder {
        pub fn new() -> Self {
            Self {
                begin: time::Instant::now(),
                #[cfg(feature = "time_predict")]
                instr_file_len: None,
                #[cfg(feature = "time_predict")]
                elapsed_clocks: None,
                cycle: None,
                elapsed: None,
            }
        }
        #[cfg(feature = "time_predict")]
        pub fn instr_file_len(&mut self, instr_file_len: u32) {
            self.instr_file_len = Some(instr_file_len)
        }
        #[cfg(feature = "time_predict")]
        pub fn elapsed_clocks(&mut self, elapsed_clocks: usize) {
            self.elapsed_clocks = Some(elapsed_clocks)
        }
        pub fn cycle(&mut self, cycle: usize) {
            self.cycle = Some(cycle)
        }
        pub fn stop_timer(&mut self) {
            self.elapsed = Some(time::Instant::now() - self.begin)
        }
        pub fn finish(&self) -> SimStat {
            SimStat {
                #[cfg(feature = "time_predict")]
                instr_file_len: self.instr_file_len.unwrap(),
                #[cfg(feature = "time_predict")]
                elapsed_clocks: self.elapsed_clocks.unwrap(),
                cycle: self.cycle.unwrap(),
                elapsed: self.elapsed.unwrap(),
            }
        }
    }

    impl Default for SimStatBuilder {
        fn default() -> Self {
            Self::new()
        }
    }

    pub struct SimStat {
        #[cfg(feature = "time_predict")]
        instr_file_len: u32,
        #[cfg(feature = "time_predict")]
        elapsed_clocks: usize,
        cycle: usize,
        elapsed: time::Duration,
    }

    impl Stat for SimStat {
        fn view(&self, _: usize) -> Box<dyn StatView + '_> {
            Box::new(self)
        }
    }

    impl StatView for &'_ SimStat {
        fn header(&self) -> &'static str {
            "simulator stat"
        }
        fn width(&self) -> usize {
            33
        }
    }

    impl fmt::Display for &'_ SimStat {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let ms = format!("{} ms", self.elapsed.as_millis());
            writeln!(f, "  elapsed total: {ms:>9}")?;
            let cycle = format!("#{}", self.cycle);
            #[cfg(feature = "time_predict")]
            {
                writeln!(f, "  cycles total: {cycle:>10}")?;
                let elapsed_clocks = format!("#{}", self.elapsed_clocks);
                writeln!(f, "  clocks total: {elapsed_clocks:>10}")?;
                let estimated_input_time = (self.instr_file_len * 10) as f64 / CPU_BAUDRATE as f64;
                let estimated_cpu_time = self.elapsed_clocks as f64 / CPU_CLOCK_FREQ as f64;
                let cpu_time = format!("{:.6} s", estimated_input_time + estimated_cpu_time);
                writeln!(f, "  estimated CPU time: {cpu_time:>9}")
            }
            #[cfg(not(feature = "time_predict"))]
            return writeln!(f, "  cycles total: {cycle:>10}");
        }
    }
}

impl<I: Input, O: Output> Simulator<I, O> {
    fn gather_watchings(&self, Watchings { reg, freg, memory }: &Watchings) -> WatchingValues {
        let mut watchings: WatchingValues = Default::default();
        for &reg in reg {
            watchings.reg_map.insert(reg, self.get_reg(reg));
        }
        for &freg in freg {
            watchings.freg_map.insert(freg, self.get_freg(freg));
        }
        for &addr in memory {
            watchings
                .memory_map
                .insert(addr, self.get_mem(addr).unwrap());
        }
        watchings
    }
    fn do_break(&self, _: &BreakPoint) -> bool {
        true
    }
    fn break_sim(&self, opt: &SimulationOption, reason: BreakReason) -> Result<ControlFlow> {
        Ok(ControlFlow::Break(OnBreak {
            watchings: self.gather_watchings(&opt.watchings),
            reason,
        }))
    }
    pub fn exit_sim(&mut self) {
        self.stat_builder.cycle(self.cycle);
        #[cfg(feature = "time_predict")]
        self.stat_builder.elapsed_clocks(self.elapsed_clocks);
        self.stat_builder.stop_timer();
    }
    pub fn single_cycle(&mut self, opt: &SimulationOption) -> Result<ControlFlow> {
        macro_rules! break_sim {
            ($reason:expr) => {
                return self.break_sim(opt, $reason)
            };
        }
        if self.fatal_error.is_some() {
            break_sim!(BreakReason::CannotRestart)
        }
        let mut is_enter = true;
        macro_rules! execute {
            () => {
                if is_enter {
                    is_enter = false;
                } else {
                    if let Some(bp) = opt.breakpoints.get(&self.cpu.get_pc_addr()) {
                        if self.do_break(bp) {
                            break_sim!(BreakReason::BreakPoint(bp.addr));
                        }
                    }
                }
                let r = self.cpu.cycle_one_full(opt.do_trace);
                let r = match r {
                    Ok(r) => r,
                    Err(e) => {
                        if e.level().is_fatal() {
                            self.fatal_error = Some(e)
                        }
                        break_sim!(BreakReason::Failed);
                    }
                };
                #[cfg(feature = "time_predict")]
                {
                    self.elapsed_clocks += r.cycles as usize;
                }
                self.cycle += 1;
                match r.flow {
                    cpu::ControlFlow::Continue => print_trace(self.cycle, &r),
                    cpu::ControlFlow::Break(reason) => break_sim!(reason.into()),
                    cpu::ControlFlow::Exit => {
                        #[cfg(feature = "stat")]
                        self.exit_sim();
                        return Ok(ControlFlow::Exit);
                    }
                }
            };
        }
        #[inline]
        fn print_trace(cycle: usize, r: &CycleResult) {
            if let Some(ExecutionTrace {
                pc,
                undecoded_instr,
                decoded_instr,
            }) = &r.trace
            {
                println!(
                    "#{cycle:010}, pc: {pc},\tinstr: {undecoded_instr:#010x}\t{decoded_instr}",
                );
            }
        }

        match &opt.mode {
            ExecuteMode::SkipUntil { pc } => loop {
                if !is_enter && pc == &self.cpu.get_pc().into_addr() {
                    break_sim!(BreakReason::Reached(*pc));
                }
                execute!();
            },
            ExecuteMode::Run => loop {
                execute!();
            },
            ExecuteMode::RunStep(r) => {
                for _ in 0..r.get_step() {
                    execute!();
                }
                break_sim!(BreakReason::StepEnded)
            }
        }
    }

    pub fn get_pc(&self) -> Pc {
        self.cpu.get_pc()
    }

    pub fn get_freg(&self, id: FRegId) -> f32 {
        self.cpu.get_freg(id)
    }

    pub fn get_reg(&self, id: RegId) -> TypedU32 {
        self.cpu.get_reg(id).typed(id.ty())
    }

    pub fn get_mem(&self, addr: Addr) -> Result<TypedU32> {
        Ok(self.cpu.get_mem(addr)?)
    }

    pub fn get_mem_pc(&self, addr: Pc) -> Result<u32> {
        Ok(self.cpu.get_mem_pc(addr)?)
    }

    pub fn get_regfile_view(&self, k: ShowRegFileKind, chunk_size: usize) -> RegFileView<'_> {
        self.cpu.get_regfile_view(k, chunk_size)
    }

    pub fn cycle(&self) -> usize {
        self.cycle
    }

    pub fn debug_symbol(&self) -> &DebugSymbol {
        &self.debug_symbol
    }

    pub fn cpu_mut(&mut self) -> &mut Cpu<I, O> {
        &mut self.cpu
    }

    pub fn get_error_msg(&self) -> Option<String> {
        self.fatal_error.as_ref().map(|e| format!("{e}"))
    }
}

pub enum ControlFlow {
    Break(OnBreak),
    Exit,
}

pub enum ExitCode {
    Success,
    Failure,
}

impl ExitCode {
    /// Returns `true` if the exit code is [`Success`].
    ///
    /// [`Success`]: ExitCode::Success
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

impl ControlFlow {
    pub fn exit_code(&self) -> Option<ExitCode> {
        if let Self::Exit = self {
            Some(ExitCode::Success)
        } else if let Self::Break(OnBreak {
            reason: BreakReason::CannotRestart | BreakReason::Failed,
            ..
        }) = self
        {
            Some(ExitCode::Failure)
        } else {
            None
        }
    }
}

pub enum BreakReason {
    CannotRestart,
    Failed,
    Reached(Addr),
    StepEnded,
    BreakPoint(Addr),
    Spy(SpyResult),
}

impl From<cpu::BreakReason> for BreakReason {
    fn from(v: cpu::BreakReason) -> Self {
        match v {
            cpu::BreakReason::Spy(s) => Self::Spy(s),
        }
    }
}

pub struct OnBreak {
    pub watchings: WatchingValues,
    pub reason: BreakReason,
}

#[derive(Default)]
pub struct WatchingValues {
    pub reg_map: HashMap<RegId, TypedU32>,
    pub freg_map: HashMap<FRegId, f32>,
    pub memory_map: HashMap<Addr, TypedU32>,
}

pub struct Assembly<'a> {
    label: &'a String,
    label_addr: Pc,
    omitted_head: bool,
    rows: Vec<AssemblyRow>,
    omitted_tail: bool,
}

impl Assembly<'_> {
    pub fn did_omit_output(&self) -> bool {
        self.omitted_head || self.omitted_tail
    }
}

impl fmt::Display for Assembly<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            label,
            label_addr,
            omitted_head,
            rows,
            omitted_tail,
        } = self;
        writeln!(f, "{label}: /* {label_addr} */")?;
        if *omitted_head {
            writeln!(f, "{:8}... /* omitted */", "")?;
        }
        for row in rows {
            writeln!(f, "{row}")?;
        }
        if *omitted_tail {
            writeln!(f, "{:8}... /* omitted */", "")?;
        }
        Ok(())
    }
}

pub struct AssemblyRow {
    special: Option<String>,
    addr: Pc,
    bin: u32,
    decoded: Option<PrettyInstr>,
}

impl fmt::Display for AssemblyRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            special,
            addr,
            bin,
            decoded,
        } = self;
        let special = special.to_owned().unwrap_or_default();
        let decoded = match decoded {
            Some(s) => s.to_string(),
            None => "???? (failed to decode)".to_string(),
        };
        write!(f, "{special:>7} {addr}  {bin:#010x}    {decoded}")
    }
}

#[derive(Default)]
pub struct DisassembleOption {
    pub addr: Option<Pc>,
    pub do_fold: bool,
    pub window_size_half: Option<u32>,
}

impl<I: Input, O: Output> Simulator<I, O> {
    pub fn disassemble_near(
        &self,
        DisassembleOption {
            addr,
            do_fold,
            window_size_half,
        }: DisassembleOption,
    ) -> Result<Assembly> {
        let window_size_half = window_size_half.unwrap_or(4) << 2;
        let window_size = window_size_half * 2 + 4;
        let cursor = addr.unwrap_or_else(|| self.get_pc());
        let pc_addr = cursor.into_addr().inner() as u32;
        let index = self.debug_symbol.get_nearest_symbol_addr(pc_addr)?;
        let def = self.debug_symbol.get_symbol(index);

        let omitted_head = do_fold && def.addr + window_size_half < pc_addr;
        let begin = if omitted_head {
            pc_addr - window_size_half
        } else {
            def.addr
        };
        let len = def.size.unwrap_or(window_size + 1);
        let end = def.addr + len;
        let omitted_tail = do_fold && begin + window_size < end;
        let len = if do_fold && begin + window_size < end {
            window_size
        } else {
            end - begin
        };
        let mut rows = Vec::with_capacity((len >> 2) as usize);
        for disp in 0..(len >> 2) {
            let addr = Pc::new(begin + (disp << 2));
            let bin = if let Ok(bin) = self.get_mem_pc(addr) {
                bin
            } else {
                // exceed instr mem
                break;
            };
            let instr = Instr::decode_from(bin)
                .ok()
                .map(|i| self.pretty_instr(addr, i));
            let special = (cursor == addr).then(|| "***".to_string());
            rows.push(AssemblyRow {
                special,
                addr,
                bin,
                decoded: instr,
            });
        }
        Ok(Assembly {
            label: &def.label,
            label_addr: Pc::new(def.addr),
            omitted_head,
            rows,
            omitted_tail,
        })
    }
}

enum PrettyInstr {
    Sugared(String),
    Unsugared(DecodedInstr),
}

impl fmt::Display for PrettyInstr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrettyInstr::Sugared(s) => write!(f, "{s}"),
            PrettyInstr::Unsugared(i) => write!(f, "{i}"),
        }
    }
}

impl<I: Input, O: Output> Simulator<I, O> {
    fn _get_label_name(&self, addr: u32) -> Option<&String> {
        let index = self.debug_symbol.get_exact_symbol_addr(addr).ok()?;
        Some(&self.debug_symbol.get_symbol(index).label)
    }
    fn pretty_instr(&self, _addr: Pc, instr: DecodedInstr) -> PrettyInstr {
        use instr::IInstr::*;
        use Instr::*;
        PrettyInstr::Sugared(match instr {
            I {
                instr: Addi,
                rd,
                rs1,
                imm,
            } if rs1.is_zero() => {
                if imm == 0 {
                    if rd.is_zero() {
                        "nop".to_string()
                    } else {
                        format!("mv {rd}, {rs1}")
                    }
                } else {
                    format!("li {rd}, {imm}")
                }
            }
            I {
                instr: Addi,
                rd,
                rs1,
                imm,
            } if (imm as i32) < 0 => format!("subi {rd}, {rs1}, {}", -(imm as i32)),
            I {
                instr: Xori,
                rd,
                rs1,
                imm: 1,
            } => format!("not {rd}, {rs1}"),
            i => return PrettyInstr::Unsugared(i),
        })
    }
}
