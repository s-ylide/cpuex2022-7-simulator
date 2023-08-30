use std::{collections::HashMap, fmt::Display, io::Write, ops::Range};

#[cfg(feature = "stat")]
use std::{cell::RefCell, rc::Rc};

use crate::{
    common::{self, Pc, SpyWatchKind, SpyWatchResultKind},
    ty::{Ty, Typed, TypedU32},
};

#[cfg(feature = "stat")]
use crate::stat::{AddStats, Stats};

#[cfg(feature = "stat")]
use crate::reg_file::MemoryRegionStatBuilder;

pub const RAM_BYTE_SIZE: usize = 1000000usize;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Addr(usize);

impl Addr {
    pub fn new(v: usize) -> Self {
        Self(v)
    }
    pub fn inner(self) -> usize {
        self.0
    }
    pub fn disp(&self, amount: usize) -> Self {
        Self(self.0 + amount)
    }
}

impl Display for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#010x}", self.0)
    }
}

#[derive(Clone, Copy)]
pub struct SpyUnit {
    pub addr: usize,
    pub expire_at: Option<usize>,
}

#[derive(Default)]
struct Spy {
    on_read: HashMap<usize, SpyUnit>,
    on_write: HashMap<usize, SpyUnit>,
}

pub struct Memory<const SIZE: usize> {
    inner: Vec<u8>,
    instr_mem_range: Range<usize>,
    #[cfg(feature = "stat")]
    stat_region: Rc<RefCell<MemoryRegionStatBuilder>>,
    #[cfg(feature = "stat")]
    stat_mem: RefCell<stat::MemoryStat>,
    #[cfg(feature = "typed_memory")]
    ty: std::cell::RefCell<Vec<Ty>>,
    spy: Spy,
}

use thiserror::Error;
use Ty::*;

#[derive(Error, Debug)]
pub enum MemoryAccessError {
    #[error("address {accessed_address} out of range for memory")]
    OutOfBounds { accessed_address: usize },
    #[error("pc {pc_address} out of range for instr memory")]
    PcOutOfBounds { pc_address: usize },
    #[cfg(feature = "typed_memory")]
    #[error("attempted to transmute {expected} into {attempt}, which is not allowed in the semantics of OCaml")]
    ViolateTransmutation { expected: Ty, attempt: Ty },
}

pub type Result<T> = std::result::Result<T, MemoryAccessError>;

macro_rules! bounds_check {
    ($addr:ident < $self:ident.$len:ident) => {
        let index = $addr << 2;
        if index >= $len || $self.instr_mem_range.contains(&index) {
            return Err(MemoryAccessError::OutOfBounds {
                accessed_address: $addr,
            });
        }
    };
}

macro_rules! type_check {
    ($self:ident[$addr:ident]: $ty:ident) => {
        if cfg!(feature = "typed_memory") {
            $self.unify($addr, $ty)?
        } else {
            Unknown
        }
    };
    ($self:ident[$addr:ident]: ?) => {
        if cfg!(feature = "typed_memory") {
            $self.unify($addr, Unknown).unwrap()
        } else {
            Unknown
        }
    };
}

macro_rules! reset_type {
    ($self:ident[$addr:ident]: $ty:ident) => {
        if cfg!(feature = "typed_memory") {
            $self.ty.borrow_mut()[$addr] = $ty
        }
    };
}

impl<const SIZE: usize> Memory<SIZE> {
    pub fn new(#[cfg(feature = "stat")] stat_region: Rc<RefCell<MemoryRegionStatBuilder>>) -> Self {
        Self {
            inner: vec![0xCC; SIZE],
            instr_mem_range: 0..0,
            #[cfg(feature = "stat")]
            stat_region,
            #[cfg(feature = "stat")]
            stat_mem: RefCell::default(),
            #[cfg(feature = "typed_memory")]
            ty: std::cell::RefCell::new(vec![Ty::Unknown; SIZE >> 2]),
            spy: Default::default(),
        }
    }
    pub fn init_from_slice(&mut self, mem: &[u8], instr_mem_range: Range<u32>) {
        let mut buf = self.inner.as_mut_slice();
        buf.write_all(mem).unwrap();
        self.instr_mem_range = instr_mem_range.start as usize..instr_mem_range.end as usize;
    }
    pub fn add_spy(&mut self, k: SpyWatchKind, u: SpyUnit) {
        if k.contains(SpyWatchKind::Read) {
            self.spy.on_read.insert(u.addr, u);
        }
        if k.contains(SpyWatchKind::Write) {
            self.spy.on_write.insert(u.addr, u);
        }
    }
    pub fn remove_spy(&mut self, k: SpyWatchKind, u: SpyUnit) {
        if k.contains(SpyWatchKind::Read) {
            self.spy.on_read.remove(&u.addr);
        }
        if k.contains(SpyWatchKind::Write) {
            self.spy.on_write.remove(&u.addr);
        }
    }
    fn on_read(&self, addr: usize, spied: &mut Option<common::SpyResult>) {
        #[cfg(feature = "stat")]
        self.stat_mem
            .borrow_mut()
            .on_read(self.stat_region.borrow().get_region(addr as u32));
        if let Some(spy) = self.spy.on_read.get(&addr) {
            *spied = Some(common::SpyResult {
                kind: SpyWatchResultKind::Read,
                target: common::SpyKind::Memory(*spy),
            });
        }
    }
    fn on_write(&self, addr: usize, val: TypedU32, spied: &mut Option<common::SpyResult>) {
        #[cfg(feature = "stat")]
        self.stat_mem
            .borrow_mut()
            .on_write(self.stat_region.borrow().get_region(addr as u32));
        if let Some(spy) = self.spy.on_write.get(&addr) {
            *spied = Some(common::SpyResult {
                kind: SpyWatchResultKind::Write {
                    before: self
                        .get_raw_addr(addr << 2)
                        .typed(if cfg!(feature = "typed_memory") {
                            self.ty.borrow()[addr]
                        } else {
                            Ty::Unknown
                        }),
                    after: val,
                },
                target: common::SpyKind::Memory(*spy),
            });
        }
    }
    #[inline]
    fn get_raw_addr(&self, addr: usize) -> u32 {
        let mut v: [u8; 4] = [0; 4];
        v[..4].copy_from_slice(&self.inner[addr..(4 + addr)]);
        u32::from_le_bytes(v)
    }
    #[cfg(feature = "typed_memory")]
    fn unify(&self, addr: usize, attempt: Ty) -> Result<Ty> {
        let ty = self.ty.borrow()[addr];
        if ty < attempt {
            self.ty.borrow_mut()[addr] = attempt;
            Ok(attempt)
        } else if ty >= attempt {
            Ok(ty)
        } else {
            Err(MemoryAccessError::ViolateTransmutation {
                expected: ty,
                attempt,
            })
        }
    }
    pub fn get(&self, addr: usize, spied: &mut Option<common::SpyResult>) -> Result<TypedU32> {
        bounds_check!(addr < self.SIZE);
        let ty = type_check!(self[addr]: ?);
        self.on_read(addr, spied);
        Ok(self.get_raw_addr(addr << 2).typed(ty))
    }
    pub fn get_i(&self, addr: usize, spied: &mut Option<common::SpyResult>) -> Result<TypedU32> {
        bounds_check!(addr < self.SIZE);
        let ty = type_check!(self[addr]: I32OrUsize);
        self.on_read(addr, spied);
        Ok(self.get_raw_addr(addr << 2).typed(ty))
    }
    pub fn get_from_pc(&self, pc: Pc) -> Result<u32> {
        let pc_address = pc.into_usize();
        if self.instr_mem_range.contains(&pc_address) {
            Ok(self.get_raw_addr(pc_address))
        } else {
            Err(MemoryAccessError::PcOutOfBounds { pc_address })
        }
    }
    pub fn get_f(&self, addr: usize, spied: &mut Option<common::SpyResult>) -> Result<f32> {
        bounds_check!(addr < self.SIZE);
        type_check!(self[addr]: F32);
        self.on_read(addr, spied);
        let mut v: [u8; 4] = [0; 4];
        let addr = addr << 2;
        v[..4].copy_from_slice(&self.inner[addr..(4 + addr)]);
        Ok(f32::from_le_bytes(v))
    }
    pub fn set(
        &mut self,
        addr: usize,
        val: u32,
        spied: &mut Option<common::SpyResult>,
    ) -> Result<()> {
        bounds_check!(addr < self.SIZE);
        self.on_write(addr, val.typed(I32OrUsize), spied);
        reset_type!(self[addr]: I32OrUsize);
        let v = val.to_le_bytes();
        let addr = addr << 2;
        self.inner[addr..(4 + addr)].copy_from_slice(&v[..4]);
        Ok(())
    }
    pub fn set_f(
        &mut self,
        addr: usize,
        val: f32,
        spied: &mut Option<common::SpyResult>,
    ) -> Result<()> {
        bounds_check!(addr < self.SIZE);
        self.on_write(addr, val.to_bits().typed(F32), spied);
        reset_type!(self[addr]: F32);
        let v = val.to_le_bytes();
        let addr = addr << 2;
        self.inner[addr..(4 + addr)].copy_from_slice(&v[..4]);
        Ok(())
    }
}

#[cfg(feature = "stat")]
impl<const SIZE: usize> AddStats for Memory<SIZE> {
    fn add_stats(&self, buf: &mut Stats) {
        buf.push(Box::new(self.stat_mem.borrow().to_owned()));
    }
}

#[cfg(feature = "stat")]
mod stat {
    use std::fmt;

    use crate::{common::MemoryRegion, stat::*};

    #[derive(Clone, Copy, Default)]
    pub struct MemoryStat {
        write: MemoryRegionCount,
        read: MemoryRegionCount,
    }

    impl MemoryStat {
        pub fn on_write(&mut self, r: MemoryRegion) {
            self.write.incr(r);
        }
        pub fn on_read(&mut self, r: MemoryRegion) {
            self.read.incr(r);
        }
    }

    #[derive(Clone, Copy, Default)]
    struct MemoryRegionCount {
        data_section: usize,
        heap: usize,
        stack: usize,
    }

    impl MemoryRegionCount {
        fn incr(&mut self, r: MemoryRegion) {
            match r {
                MemoryRegion::DataSection => self.data_section += 1,
                MemoryRegion::Heap => self.heap += 1,
                MemoryRegion::Stack => self.stack += 1,
            }
        }
    }

    impl Stat for MemoryStat {
        fn view(&self, _: usize) -> Box<dyn StatView + '_> {
            Box::new(MemoryStatView::new(self))
        }
    }

    pub struct MemoryStatView<'a> {
        stat: &'a MemoryStat,
    }

    impl<'a> MemoryStatView<'a> {
        pub fn new(stat: &'a MemoryStat) -> Self {
            Self { stat }
        }
    }

    impl StatView for MemoryStatView<'_> {
        fn header(&self) -> &'static str {
            "access count of memory region (format: `# of read / # of write`)"
        }
        fn width(&self) -> usize {
            40
        }
    }

    impl fmt::Display for MemoryStatView<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            macro_rules! output {
                ($kind:ident => $name:expr) => {{
                    let r = self.stat.read.$kind;
                    let w = self.stat.write.$kind;
                    writeln!(f, "  {:>13}:{r:>11} /{w:>11}", $name)
                }};
            }
            output!(data_section => "data section")?;
            output!(heap => "heap")?;
            output!(stack => "stack")
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory() {
        let mut m = Memory::<4>::new(Default::default());
        m.set(0, 0xDEADBEEF, &mut None).unwrap();
        assert_eq!(
            0xDEADBEEFu32,
            m.get_i(0, &mut None).unwrap().get_unchecked()
        );
    }
}
