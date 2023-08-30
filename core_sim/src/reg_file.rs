use std::fmt::Display;

use crate::register::{FRegId, RegId, ABINAME_TABLE, F_ABINAME_TABLE, MAX_REG_ID};

#[cfg(feature = "stat")]
use std::{cell::RefCell, rc::Rc};

#[cfg(feature = "stat")]
use crate::stat::{AddStats, Stats};

#[cfg(feature = "stat")]
pub use stat::MemoryRegionStatBuilder;

#[cfg(feature = "stat")]
use stat::{RegFileAllStat, RegFileStat};

pub struct SpyUnit {
    pub addr: usize,
    pub expire_at: Option<usize>,
}

pub struct RegFile {
    inner: [u32; MAX_REG_ID],
    inner_f: [f32; MAX_REG_ID],
    #[cfg(feature = "stat")]
    stat_memregion: Rc<RefCell<MemoryRegionStatBuilder>>,
    #[cfg(feature = "stat")]
    stat_i: RegFileStat,
    #[cfg(feature = "stat")]
    stat_f: RegFileStat,
}

impl RegFile {
    pub fn new() -> Self {
        Self {
            inner: [0; MAX_REG_ID],
            inner_f: [0.0f32; MAX_REG_ID],
            #[cfg(feature = "stat")]
            stat_memregion: Default::default(),
            #[cfg(feature = "stat")]
            stat_i: RegFileStat::new(ABINAME_TABLE),
            #[cfg(feature = "stat")]
            stat_f: RegFileStat::new(F_ABINAME_TABLE),
        }
    }
    pub fn get(&self, id: RegId) -> u32 {
        #[cfg(feature = "stat")]
        self.stat_i.encounter_read(id.inner());
        self.inner[id.inner()]
    }
    pub fn get_f(&self, id: FRegId) -> f32 {
        #[cfg(feature = "stat")]
        self.stat_f.encounter_read(id.inner());
        self.inner_f[id.inner()]
    }
    pub fn set_sp(&mut self, val: u32) {
        self.inner[2] = val;
    }
    pub fn set_hp(&mut self, val: u32) {
        self.inner[4] = val;
    }
    pub fn set(&mut self, id: RegId, val: u32) {
        #[cfg(feature = "stat")]
        {
            self.stat_i.encounter_write(id.inner());
            if id.inner() == 2 {
                self.stat_memregion.borrow_mut().update_sp(val);
            }
        }
        if id.inner() != 0 {
            self.inner[id.inner()] = val;
        }
    }
    pub fn set_f(&mut self, id: FRegId, val: f32) {
        #[cfg(feature = "stat")]
        self.stat_f.encounter_write(id.inner());
        if id.inner() != 0 {
            self.inner_f[id.inner()] = val;
        }
    }
    pub fn end_init(&mut self) {
        #[cfg(feature = "stat")]
        self.stat_memregion
            .borrow_mut()
            .init(self.inner[4], self.inner[2])
    }
    #[cfg(feature = "stat")]
    pub fn get_region(&self, addr: u32) -> crate::common::MemoryRegion {
        self.stat_memregion.borrow().get_region(addr)
    }

    pub fn mem_region(&self) -> Rc<RefCell<MemoryRegionStatBuilder>> {
        self.stat_memregion.clone()
    }
}

#[cfg(feature = "stat")]
impl AddStats for RegFile {
    fn add_stats(&self, buf: &mut Stats) {
        buf.push(Box::new(self.stat_memregion.borrow().finish(self.inner[4])));
        buf.push(Box::new(RegFileAllStat::new(
            self.stat_i.to_owned(),
            self.stat_f.to_owned(),
        )));
    }
}

impl RegFile {
    pub fn get_view(&self, k: ShowRegFileKind, chunk_size: usize) -> RegFileView<'_> {
        RegFileView {
            r: self,
            k,
            chunk_size,
        }
    }
}

pub struct RegFileView<'a> {
    r: &'a RegFile,
    k: ShowRegFileKind,
    chunk_size: usize,
}

impl<'a> Display for RegFileView<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn fmt_inner(
            map: Vec<String>,
            chunk_size: usize,
            f: &mut std::fmt::Formatter,
        ) -> std::fmt::Result {
            for chunk in map.chunks(chunk_size) {
                let s = chunk.join(", ");
                writeln!(f, "  {s},")?;
            }
            Ok(())
        }
        match &self.k {
            ShowRegFileKind::RegFileAll => {
                let map: Vec<_> = ABINAME_TABLE
                    .iter()
                    .zip(self.r.inner)
                    .map(|(n, v)| format!("{n:>6}: {v:>16}"))
                    .collect();
                writeln!(f, "RegFile (All) {{")?;
                fmt_inner(map, self.chunk_size, f)?;
                let map: Vec<_> = F_ABINAME_TABLE
                    .iter()
                    .zip(self.r.inner_f)
                    .map(|(n, v)| format!("{n:>6}: {v:>16}"))
                    .collect();
                fmt_inner(map, self.chunk_size, f)?;
                write!(f, "}}")
            }
            ShowRegFileKind::RegFileI => {
                let map: Vec<_> = ABINAME_TABLE
                    .iter()
                    .zip(self.r.inner)
                    .map(|(n, v)| format!("{n:>6}: {v:>16}"))
                    .collect();
                writeln!(f, "RegFile (Integer) {{")?;
                fmt_inner(map, self.chunk_size, f)?;
                write!(f, "}}")
            }
            ShowRegFileKind::RegFileF => {
                let map: Vec<_> = F_ABINAME_TABLE
                    .iter()
                    .zip(self.r.inner_f)
                    .map(|(n, v)| format!("{n:>6}: {v:>16}"))
                    .collect();
                writeln!(f, "RegFile (Float) {{")?;
                fmt_inner(map, self.chunk_size, f)?;
                write!(f, "}}")
            }
        }
    }
}

pub enum ShowRegFileKind {
    RegFileAll,
    RegFileI,
    RegFileF,
}

impl Default for RegFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "stat")]
mod stat {
    use std::{cell::RefCell, fmt};

    use super::*;
    use crate::{common::MemoryRegion, stat::*};

    pub struct RegFileAllStat {
        i: RegFileStat,
        f: RegFileStat,
    }

    impl RegFileAllStat {
        pub fn new(i: RegFileStat, f: RegFileStat) -> Self {
            Self { i, f }
        }
    }

    #[derive(Clone)]
    pub struct RegFileStat {
        write: [usize; MAX_REG_ID],
        read: RefCell<[usize; MAX_REG_ID]>,
        abiname_table: [&'static str; MAX_REG_ID],
    }

    impl Stat for RegFileAllStat {
        fn view(&self, max_width: usize) -> Box<dyn StatView + '_> {
            Box::new(RegFileAllStatView::new(self, max_width))
        }
    }

    pub struct RegFileAllStatView<'a> {
        stat: &'a RegFileAllStat,
        chunk_size: usize,
    }

    impl<'a> RegFileAllStatView<'a> {
        pub fn new(stat: &'a RegFileAllStat, max_width: usize) -> Self {
            Self {
                stat,
                chunk_size: Self::chunk_size(max_width),
            }
        }
    }

    impl StatView for RegFileAllStatView<'_> {
        fn header(&self) -> &'static str {
            "register load (format: `# of read / # of write`)"
        }
        fn width(&self) -> usize {
            Self::width_by_chunk_size(self.chunk_size)
        }
    }

    impl<'a> Width for RegFileAllStatView<'a> {
        fn width_by_chunk_size(chunk_size: usize) -> usize {
            chunk_size * 31 + (chunk_size - 1) * 2 + 2
        }
    }

    impl fmt::Display for RegFileAllStatView<'_> {
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
            for rf in [&self.stat.i, &self.stat.f] {
                let map: Vec<_> = rf
                    .abiname_table
                    .iter()
                    .zip(rf.read.borrow().into_iter())
                    .zip(rf.write)
                    .map(|((n, r), w)| format!("{n:>6}:{r:>11} /{w:>11}"))
                    .collect();
                fmt_inner(map, self.chunk_size, f)?;
            }
            Ok(())
        }
    }

    impl RegFileStat {
        pub fn new(abiname_table: [&'static str; MAX_REG_ID]) -> Self {
            Self {
                write: [0; MAX_REG_ID],
                read: RefCell::new([0; MAX_REG_ID]),
                abiname_table,
            }
        }
        pub fn encounter_write(&mut self, id: usize) {
            self.write[id] += 1;
        }
        pub fn encounter_read(&self, id: usize) {
            self.read.borrow_mut()[id] += 1;
        }
    }

    #[derive(Default)]
    pub struct MemoryRegionStatBuilder {
        hp_min: u32,
        sp_min: u32,
        sp_max: u32,
    }

    impl MemoryRegionStatBuilder {
        pub fn init(&mut self, hp_min: u32, sp_max: u32) {
            self.hp_min = hp_min;
            self.sp_max = sp_max;
            self.sp_min = sp_max;
        }
        #[inline]
        pub fn update_sp(&mut self, value: u32) {
            if self.sp_min > value {
                self.sp_min = value;
            }
        }
        pub fn finish(&self, hp_max: u32) -> MemoryRegionStat {
            MemoryRegionStat {
                hp_min: self.hp_min,
                hp_max,
                sp_min: self.sp_min,
                sp_max: self.sp_max,
            }
        }
        pub fn get_region(&self, addr: u32) -> MemoryRegion {
            if addr < self.hp_min {
                MemoryRegion::DataSection
            } else if addr >= self.sp_min - 1000 {
                MemoryRegion::Stack
            } else {
                MemoryRegion::Heap
            }
        }
    }

    pub struct MemoryRegionStat {
        hp_min: u32,
        hp_max: u32,
        sp_min: u32,
        sp_max: u32,
    }

    impl Stat for MemoryRegionStat {
        fn view(&self, _: usize) -> Box<dyn StatView + '_> {
            Box::new(HeapStackStatView::new(self))
        }
    }

    pub struct HeapStackStatView<'a> {
        stat: &'a MemoryRegionStat,
    }

    impl<'a> HeapStackStatView<'a> {
        pub fn new(stat: &'a MemoryRegionStat) -> Self {
            Self { stat }
        }
    }

    impl StatView for HeapStackStatView<'_> {
        fn header(&self) -> &'static str {
            "used memory"
        }
        fn width(&self) -> usize {
            24
        }
    }

    impl fmt::Display for HeapStackStatView<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let MemoryRegionStat {
                hp_min,
                hp_max,
                sp_min,
                sp_max,
            } = self.stat;
            let heap = hp_max - hp_min;
            let stack = sp_max - sp_min;
            let heap = format!("{}", heap << 2);
            let stack = format!("{}", stack << 2);
            writeln!(f, "  heap: {heap:>10} bytes")?;
            writeln!(f, "  stack: {stack:>9} bytes")
        }
    }
}
