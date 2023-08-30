use std::{collections::HashMap, fmt};

use bitmask_enum::bitmask;

use crate::{
    breakpoint::BreakPoint,
    memory::{self, Addr},
    register::{FRegId, RegId},
    ty::{Ty, TypedU32},
};

#[derive(Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// to unify displaying value of program counter
pub struct Pc(u32);

impl Pc {
    pub fn new(v: u32) -> Self {
        Self(v)
    }
    pub fn incr(&mut self) {
        self.0 += 4;
    }
    pub fn into_usize(self) -> usize {
        self.0 as usize
    }
    pub fn into_inner(self) -> u32 {
        self.0
    }
    pub fn into_addr(self) -> Addr {
        Addr::new(self.into_usize())
    }
}

impl fmt::Display for Pc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#010x}", self.0)
    }
}

#[derive(Default)]
pub struct RunStep {
    step: Option<usize>,
}

impl RunStep {
    pub fn new(step: Option<usize>) -> Self {
        Self { step }
    }

    pub fn get_step(&self) -> usize {
        self.step.unwrap_or(1)
    }
}

#[derive(Default)]
pub enum ExecuteMode {
    #[default]
    Run,
    SkipUntil {
        pc: Addr,
    },
    RunStep(RunStep),
}

impl fmt::Display for ExecuteMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecuteMode::Run => write!(f, "running simply"),
            ExecuteMode::SkipUntil { pc } => write!(f, "running until {pc}"),
            ExecuteMode::RunStep(r) => {
                write!(f, "step execution by {}", r.get_step())
            }
        }
    }
}

#[derive(Default)]
pub struct SimulationOption {
    pub do_trace: bool,
    pub mode: ExecuteMode,
    pub breakpoints: HashMap<Addr, BreakPoint>,
    pub watchings: Watchings,
}

#[derive(Default)]
pub struct Watchings {
    pub reg: Vec<RegId>,
    pub freg: Vec<FRegId>,
    pub memory: Vec<Addr>,
}

#[bitmask(u8)]
pub enum SpyWatchKind {
    Read,
    Write,
}

impl fmt::Display for SpyWatchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.contains(Self::Read) {
            write!(f, "read")?;
            if self.contains(Self::Write) {
                write!(f, "/write")?;
            }
        } else if self.contains(Self::Write) {
            write!(f, "write")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct Spy {
    pub kind: SpyWatchKind,
    pub target: SpyKind,
}

impl fmt::Display for Spy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} of {}", self.kind, self.target)
    }
}

#[derive(Clone, Copy)]
pub enum SpyKind {
    Memory(memory::SpyUnit),
    RegisterI(RegId),
    RegisterF(FRegId),
}

impl SpyKind {
    pub fn ty(&self) -> Ty {
        match self {
            SpyKind::Memory(_) => Ty::Unknown,
            SpyKind::RegisterI(i) => i.ty(),
            SpyKind::RegisterF(_) => Ty::F32,
        }
    }
}

impl fmt::Display for SpyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpyKind::Memory(u) => write!(f, "M[{}]", Addr::new(u.addr)),
            SpyKind::RegisterI(u) => write!(f, "{u}"),
            SpyKind::RegisterF(u) => write!(f, "{u}"),
        }
    }
}

pub enum SpyWatchResultKind {
    Read,
    Write { before: TypedU32, after: TypedU32 },
}

#[allow(unused)]
pub struct SpyResult {
    pub kind: SpyWatchResultKind,
    pub target: SpyKind,
}

pub enum MemoryRegion {
    DataSection,
    Heap,
    Stack,
}
