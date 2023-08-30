use std::io::{stdin, stdout, Write};

use anyhow::Result;
use bitmask_enum::bitmask;
use core_sim::{
    breakpoint::BreakPoint,
    common::{
        ExecuteMode, RunStep, SimulationOption, Spy, SpyKind, SpyResult, SpyWatchKind,
        SpyWatchResultKind, Watchings,
    },
    debug_symbol::DebugSymbol,
    io::{Input, Output},
    memory::{self, Addr},
    reg_file::ShowRegFileKind,
    register::{FRegId, RegId},
    sim::{BreakReason, ControlFlow, DisassembleOption, OnBreak, Simulator, WatchingValues},
};

#[cfg(feature = "stat")]
use core_sim::stat::AddStats;

use terminal_size::terminal_size;

peg::parser!(grammar command(ds: &DebugSymbol) for str {
    rule unsigned() -> u32
        = n:$(quiet!{['0'..='9']+}) { n.parse().unwrap() }
        / expected!("unsigned")
    rule usize() -> usize
        = n:$(quiet!{['0'..='9']+}) { n.parse().unwrap() }
        / expected!("usize")
    rule radix() -> usize
        = quiet!{"0" ['x' | 'X']} n:$(quiet!{['0'..='9'|'a'..='f'|'A'..='F']+}) {usize::from_str_radix(n, 16).unwrap()}
        / quiet!{"0" ['d' | 'D']} n:$(quiet!{['0'..='9']+}) {n.parse().unwrap()}
        / quiet!{"0"} n:$(quiet!{['0'..='7']+}) {usize::from_str_radix(n, 8).unwrap()}
    rule addr() -> Addr
        = r:radix() { Addr::new(r) }
        / k:ident() {?
            match ds.get_global(k) {
                Some(d) => Ok(Addr::new(d.addr as usize)),
                None => Err("address or global name")
            }
        }

    rule ident() -> &'input str
        = $(quiet!{
            [ c if c.is_ascii_alphabetic() ]
            [ c if c == '_' || c.is_ascii_alphanumeric() ]*
        })
        / expected!("identifier")
    rule reg_name() -> RegId
        = i:ident() {? i.try_into().map_err(|_| "register abi name") }
        / "x" !__ n:unsigned()
        {? n.try_into().map_err(|_| "0-31") }
        / expected!("name of integer register")
    rule freg_name() -> FRegId
        = i:ident() {? i.try_into().map_err(|_| "float register abi name") }
        / "f" !__ n:unsigned()
        {? n.try_into().map_err(|_| "0-31") }
        / expected!("name of floating point register")
    rule bp() = "bp" / "breakpoint"
    rule reg() = "register" / "reg"
    rule add() = "add" / "+="
    rule rm() = "rm" / "remove" / "-="
    rule op() -> Operation
        = add() { Operation::Add } / rm() { Operation::Remove }
    rule allregs() = reg() "s"?
    rule iregs() = ("integer" / "int" / "i") __ reg() "s"?
    rule fregs() = (("floating" __ "point") / "float" / "f") __ reg() "s"?
    rule regfile_kind() -> ShowRegFileKind
        = allregs() { ShowRegFileKind::RegFileAll }
        / iregs() { ShowRegFileKind::RegFileI }
        / fregs() { ShowRegFileKind::RegFileF }
    rule dyn_command() -> ExecuteMode
        = "skip" __ "until" __ pc:addr() { ExecuteMode::SkipUntil { pc } }
        / "run" { ExecuteMode::Run }
        / "step" __ step:(radix() / usize())? { ExecuteMode::RunStep(RunStep::new(step)) }
    rule static_command() -> StaticCommand
        = "trace" __ "off" { StaticCommand::UpdateWhetherTrace(false) }
        / "trace" (__ "on")? { StaticCommand::UpdateWhetherTrace(true) }
        / bp() __ addr:addr() { StaticCommand::AddBp(BreakPoint::new(addr)) }
        / bp() __ rm() __ addr:addr() { StaticCommand::RemoveBp(addr) }
        / "watch" __ wk:watch_kind() { StaticCommand::Watch(Operation::Add, wk) }
        / "unwatch" __ wk:watch_kind() { StaticCommand::Watch(Operation::Remove, wk) }
        / "spy" __ ("on" __)? s:spy() { StaticCommand::Spy(Operation::Add, s) }
        / "spy" __ "off" __ s:spy() { StaticCommand::Spy(Operation::Add, s) }
        / "show" __ sk:show_kind() { StaticCommand::Show(sk) }
    rule watch_kind() -> WatchingKind
        = reg:reg_name() { WatchingKind::Reg(reg) }
        / reg:freg_name() { WatchingKind::FReg(reg) }
        / addr:addr() { WatchingKind::MemAddr(addr) }
        / k:regfile_kind() { WatchingKind::RegFile(k) }
    rule read() -> () = "read" / "r"
    rule write() -> () = "write" / "w"
    rule spy() -> Spy
        = r:read()? w:write()? __ target:spy_kind() {
            let mut kind = if r.is_some() {
                SpyWatchKind::Read
            } else {
                SpyWatchKind::none()
            };
            if w.is_some() {
                kind |= SpyWatchKind::Write
            }
            Spy { kind, target }
        }
    rule spy_kind() -> SpyKind
        = mem() __ addr:addr() {
            SpyKind::Memory(memory::SpyUnit { addr: addr.inner(), expire_at: None })
        }
    rule mem() = "memory" / "mem"
    rule folded() -> bool
        = "fold" "ed"? { true }
        / ("unfold" "ed"? / "full") { false }
        / { true }
    rule show_kind() -> ShowKind
        = "pc" { ShowKind::Pc }
        / k:regfile_kind() { ShowKind::RegFile(k) }
        / bp() { ShowKind::AllBp }
        / ("definition" / "def") __ fold:folded() { ShowKind::LabelDefNearPc { fold } }
        / mem() __ addr:addr() { ShowKind::Memory(addr) }
        / "stat" { ShowKind::Stat(StatKind::Cpu) }
        / reg:reg_name() {? Ok(ShowKind::RegisterI(reg)) }
        / reg:freg_name() {? Ok(ShowKind::RegisterF(reg)) }
    pub(crate) rule parse_command() -> Command
        = _ s:static_command() _ { Command::Static(s) }
        / _ "exit" _ { Command::Exit }
        / _ d:dyn_command()? _ { Command::Dynamic(d) }
        / expected!("command")

    rule ws() = quiet!{[' ' | '\t' | '\r' | '\n']}
        / expected!("whitespace")
    rule _() = ws()*
    rule __() = ws()+
});

pub(crate) enum Command {
    Dynamic(Option<ExecuteMode>),
    Static(StaticCommand),
    Exit,
}

pub(crate) enum StaticCommand {
    UpdateWhetherTrace(bool),
    Show(ShowKind),
    AddBp(BreakPoint),
    RemoveBp(Addr),
    Watch(Operation, WatchingKind),
    Spy(Operation, Spy),
}

pub(crate) enum Operation {
    Add,
    Remove,
}

pub(crate) enum WatchingKind {
    RegFile(ShowRegFileKind),
    Reg(RegId),
    FReg(FRegId),
    MemAddr(Addr),
}

pub(crate) enum ShowKind {
    Pc,
    LabelDefNearPc { fold: bool },
    Stat(StatKind),
    AllBp,
    IsTraceEnabled,
    Watchings,
    AddedSpy(Spy),
    RemovedSpy(Spy),
    Memory(Addr),
    RegFile(ShowRegFileKind),
    RegisterI(RegId),
    RegisterF(FRegId),
}

pub(crate) enum StatKind {
    Cpu,
}

#[bitmask(u8)]
enum WatchRegFile {
    RegFileI,
    RegFileF,
}

fn get_terminal_width() -> Option<u16> {
    terminal_size().map(|(w, _)| w.0 - 20)
}

pub fn execute_interactive(sim: &mut Simulator<impl Input, impl Output>) -> Result<()> {
    let mut opt = SimulationOption::default();
    let mut watching_regfile = WatchRegFile::none();
    #[cfg(feature = "stat")]
    let width = get_terminal_width();
    let regfile_chunk_size = get_terminal_width().map(|w| w / 30).unwrap_or(2).max(2) as usize;
    println!("entering interactive.");
    'interactive: loop {
        let mut show = None;
        let update_mode = 'input: loop {
            if let Some(show) = show.take() {
                match show {
                    ShowKind::Pc => {
                        println!("pc: {}, cycle #{}", sim.get_pc(), sim.cycle());
                    }
                    ShowKind::IsTraceEnabled => {
                        println!(
                            "trace {}",
                            if opt.do_trace { "enabled" } else { "disabled" }
                        );
                    }
                    #[cfg(feature = "stat")]
                    ShowKind::Stat(StatKind::Cpu) => {
                        let mut stats = Default::default();
                        sim.cpu_mut().add_stats(&mut stats);
                        println!("{}", stats.view(width.unwrap_or(60) as usize));
                    }
                    #[cfg(not(feature = "stat"))]
                    ShowKind::Stat(StatKind::Cpu) => {
                        panic!("try compile with `--features stat`");
                    }
                    ShowKind::Memory(addr) => match sim.get_mem(addr) {
                        Ok(v) => {
                            println!("M[{addr}] == {v}");
                        }
                        Err(e) => {
                            println!("{e}");
                        }
                    },
                    ShowKind::RegFile(k) => {
                        let view = sim.get_regfile_view(k, regfile_chunk_size);
                        println!("{view}");
                    }
                    ShowKind::RegisterI(id) => {
                        print!("{id} == ");
                        let val = sim.get_reg(id);
                        println!("{val}");
                    }
                    ShowKind::RegisterF(id) => {
                        print!("{id} == ");
                        let val = sim.get_freg(id);
                        println!("{val}");
                    }
                    ShowKind::AllBp => {
                        let mut v: Vec<_> = opt.breakpoints.iter().collect();
                        if v.is_empty() {
                            println!("no breakpoints set.");
                        } else {
                            print!("breakpoints: ");
                            v.sort_by_key(|(f, _)| *f);
                            println!(
                                "[{}]",
                                v.into_iter()
                                    .map(|(_, b)| format!("{b}"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                    }
                    ShowKind::Watchings => {
                        let mut printed = false;
                        macro_rules! print_unless_printed_yet {
                            () => {
                                if !printed {
                                    println!("watching these values:");
                                    printed = true;
                                }
                            };
                        }
                        let Watchings { reg, freg, memory } = &opt.watchings;
                        if !reg.is_empty() {
                            print_unless_printed_yet!();
                            print!("\tregisters: ");
                            println!(
                                "{}",
                                reg.iter()
                                    .map(|b| format!("{b}"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                        if !freg.is_empty() {
                            print_unless_printed_yet!();
                            print!("\tfloating pointer registers: ");
                            println!(
                                "{}",
                                freg.iter()
                                    .map(|b| format!("{b}"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                        if !memory.is_empty() {
                            print_unless_printed_yet!();
                            print!("\tmemory: ");
                            println!(
                                "{}",
                                memory
                                    .iter()
                                    .map(|a| format!("{a}"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                        if watching_regfile.is_all() {
                            print_unless_printed_yet!();
                            println!("\tRegFile (All)");
                        } else if watching_regfile.contains(WatchRegFile::RegFileI) {
                            print_unless_printed_yet!();
                            println!("\tRegFile (Integer)");
                        } else if watching_regfile.contains(WatchRegFile::RegFileF) {
                            print_unless_printed_yet!();
                            println!("\tRegFile (Float)");
                        }
                        if !printed {
                            println!("nothing to watch.");
                        }
                    }
                    ShowKind::LabelDefNearPc { fold } => {
                        println!("loading assembly near pc...");
                        match sim.disassemble_near(DisassembleOption {
                            do_fold: fold,
                            ..Default::default()
                        }) {
                            Ok(asm) => {
                                if fold && asm.did_omit_output() {
                                    println!("some output is omitted; type \"show def full\" to full output.");
                                }
                                println!();
                                println!("{asm}");
                            }
                            Err(e) => {
                                println!("{e}");
                            }
                        }
                    }
                    ShowKind::AddedSpy(s) => {
                        println!("added spy on {s}");
                    }
                    ShowKind::RemovedSpy(s) => {
                        println!("removed spy on {s}");
                    }
                }
            }
            // prompt string
            match &opt.mode {
                ExecuteMode::Run => print!("run "),
                ExecuteMode::SkipUntil { pc } => print!("until {pc} "),
                ExecuteMode::RunStep(n) => print!("step {} ", n.get_step()),
            }
            if opt.do_trace {
                print!("[trace] ");
            }
            print!("> ");
            stdout().flush().unwrap();
            let mut str = String::new();
            stdin().read_line(&mut str)?;
            let parsed = match command::parse_command(&str, sim.debug_symbol()) {
                Ok(p) => p,
                Err(e) => {
                    println!("parse error: expected {}", e.expected);
                    continue;
                }
            };
            match parsed {
                Command::Dynamic(d) => {
                    break 'input d;
                }
                Command::Static(s) => {
                    use Operation::*;
                    match s {
                        StaticCommand::Show(s) => show = Some(s),
                        StaticCommand::Spy(Add, s @ Spy { kind, target }) => {
                            match target {
                                SpyKind::Memory(uni) => sim.cpu_mut().add_mem_spy(kind, uni),
                                SpyKind::RegisterI(_) => todo!(),
                                SpyKind::RegisterF(_) => todo!(),
                            }
                            show = Some(ShowKind::AddedSpy(s));
                        }
                        StaticCommand::Spy(Remove, s @ Spy { kind, target }) => {
                            match target {
                                SpyKind::Memory(uni) => sim.cpu_mut().remove_mem_spy(kind, uni),
                                SpyKind::RegisterI(_) => todo!(),
                                SpyKind::RegisterF(_) => todo!(),
                            }
                            show = Some(ShowKind::RemovedSpy(s));
                        }
                        StaticCommand::UpdateWhetherTrace(b) => {
                            opt.do_trace = b;
                            show = Some(ShowKind::IsTraceEnabled);
                        }
                        StaticCommand::AddBp(bp) => {
                            opt.breakpoints.insert(bp.addr, bp);
                            show = Some(ShowKind::AllBp);
                        }
                        StaticCommand::RemoveBp(pc) => {
                            opt.breakpoints.remove(&pc);
                            show = Some(ShowKind::AllBp);
                        }
                        StaticCommand::Watch(Add, w) => {
                            match w {
                                WatchingKind::Reg(r) => {
                                    if !opt.watchings.reg.contains(&r) {
                                        opt.watchings.reg.push(r);
                                    }
                                }
                                WatchingKind::FReg(r) => {
                                    if !opt.watchings.freg.contains(&r) {
                                        opt.watchings.freg.push(r);
                                    }
                                }
                                WatchingKind::MemAddr(a) => {
                                    if !opt.watchings.memory.contains(&a) {
                                        opt.watchings.memory.push(a);
                                    }
                                }
                                WatchingKind::RegFile(k) => match k {
                                    ShowRegFileKind::RegFileAll => {
                                        watching_regfile = WatchRegFile::all();
                                    }
                                    ShowRegFileKind::RegFileI => {
                                        watching_regfile |= WatchRegFile::RegFileI;
                                    }
                                    ShowRegFileKind::RegFileF => {
                                        watching_regfile |= WatchRegFile::RegFileF;
                                    }
                                },
                            };
                            show = Some(ShowKind::Watchings)
                        }
                        StaticCommand::Watch(Remove, w) => {
                            match w {
                                WatchingKind::Reg(r) => {
                                    if let Some(index) =
                                        opt.watchings.reg.iter().position(|rr| *rr == r)
                                    {
                                        opt.watchings.reg.remove(index);
                                    }
                                }
                                WatchingKind::FReg(r) => {
                                    if let Some(index) =
                                        opt.watchings.freg.iter().position(|rr| *rr == r)
                                    {
                                        opt.watchings.freg.remove(index);
                                    }
                                }
                                WatchingKind::MemAddr(a) => {
                                    if let Some(index) =
                                        opt.watchings.memory.iter().position(|aa| *aa == a)
                                    {
                                        opt.watchings.memory.remove(index);
                                    }
                                }
                                WatchingKind::RegFile(k) => match k {
                                    ShowRegFileKind::RegFileAll => {
                                        watching_regfile = WatchRegFile::none();
                                    }
                                    ShowRegFileKind::RegFileI => {
                                        watching_regfile ^= WatchRegFile::RegFileI;
                                    }
                                    ShowRegFileKind::RegFileF => {
                                        watching_regfile ^= WatchRegFile::RegFileF;
                                    }
                                },
                            }
                            show = Some(ShowKind::Watchings)
                        }
                    };
                    continue 'input;
                }
                Command::Exit => {
                    sim.exit_sim();
                    break 'interactive;
                }
            }
        };
        if let Some(mode) = update_mode {
            println!("mode: {mode}");
            opt.mode = mode;
        }
        match sim.single_cycle(&opt)? {
            ControlFlow::Break(OnBreak {
                watchings:
                    WatchingValues {
                        reg_map,
                        freg_map,
                        memory_map,
                    },
                reason,
            }) => {
                use BreakReason::*;
                match reason {
                    Reached(..) => (),
                    StepEnded => (),
                    BreakPoint(a) => {
                        println!("reached {a}")
                    }
                    Spy(SpyResult { kind, target }) => {
                        use SpyWatchResultKind::*;
                        match &kind {
                            Read => println!("detect read out of {target}"),
                            Write { .. } => println!("detect write to {target}"),
                        }
                        if let Write { before, after } = kind {
                            println!("\tvalue updated {before} -> {after}")
                        }
                    }
                    CannotRestart => {
                        let e = sim.get_error_msg().unwrap();
                        println!("cannot restart simulator due to previous error: {e}")
                    }
                    Failed => {
                        let e = sim.get_error_msg().unwrap();
                        let cy = sim.cycle();
                        println!("failed at #{cy}: {e}")
                    }
                }
                let Watchings { reg, freg, memory } = &opt.watchings;
                let show_ireg = watching_regfile.contains(WatchRegFile::RegFileI);
                let show_freg = watching_regfile.contains(WatchRegFile::RegFileF);
                if show_ireg && show_freg {
                    let view =
                        sim.get_regfile_view(ShowRegFileKind::RegFileAll, regfile_chunk_size);
                    println!("{view}");
                } else {
                    if show_ireg {
                        let view =
                            sim.get_regfile_view(ShowRegFileKind::RegFileI, regfile_chunk_size);
                        println!("{view}");
                    } else {
                        let mut showed = false;
                        for reg in reg {
                            if showed {
                                print!(", ");
                            } else {
                                showed = true;
                            }
                            print!("{} == {}", reg, reg_map[reg]);
                        }
                        if showed {
                            println!();
                        }
                    }
                    if show_freg {
                        let view =
                            sim.get_regfile_view(ShowRegFileKind::RegFileF, regfile_chunk_size);
                        println!("{view}");
                    } else {
                        let mut showed = false;
                        for freg in freg {
                            if showed {
                                print!(", ");
                            } else {
                                showed = true;
                            }
                            print!("{} == {}", freg, freg_map[freg]);
                        }
                        if showed {
                            println!();
                        }
                    }
                }
                {
                    let mut showed = false;
                    for addr in memory {
                        if showed {
                            print!(", ");
                        } else {
                            showed = true;
                        }
                        let i = &memory_map[addr];
                        print!("M[{addr}] == {i}");
                    }
                    if showed {
                        println!();
                    }
                }
                continue;
            }
            ControlFlow::Exit => {
                println!("program halts");
                break 'interactive;
            }
        }
    }
    println!("exiting interactive.");
    Ok(())
}
