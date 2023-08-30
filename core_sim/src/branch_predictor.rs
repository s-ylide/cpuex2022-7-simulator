pub const NUM_COUNTERS: usize = 64usize;

#[derive(Clone, Copy, PartialEq)]
enum SaturatingCounter {
    StronglyUntaken,
    WeaklyUntaken,
    WeaklyTaken,
    StronglyTaken,
}

impl SaturatingCounter {
    fn next(self) -> Self {
        match self {
            SaturatingCounter::StronglyUntaken => SaturatingCounter::WeaklyUntaken,
            SaturatingCounter::WeaklyUntaken => SaturatingCounter::WeaklyTaken,
            SaturatingCounter::WeaklyTaken => SaturatingCounter::StronglyTaken,
            SaturatingCounter::StronglyTaken => SaturatingCounter::StronglyTaken,
        }
    }

    fn prev(self) -> Self {
        match self {
            SaturatingCounter::StronglyUntaken => SaturatingCounter::StronglyUntaken,
            SaturatingCounter::WeaklyUntaken => SaturatingCounter::StronglyUntaken,
            SaturatingCounter::WeaklyTaken => SaturatingCounter::WeaklyUntaken,
            SaturatingCounter::StronglyTaken => SaturatingCounter::WeaklyTaken,
        }
    }
}

pub struct BranchPredictor<const NCOUNTERS: usize> {
    inner: Vec<SaturatingCounter>,
}

impl<const NCOUNTERS: usize> BranchPredictor<NCOUNTERS> {
    pub fn new() -> Self {
        Self {
            inner: vec![SaturatingCounter::WeaklyTaken; NCOUNTERS],
        }
    }

    pub fn predict(&self, addr: usize) -> bool {
        let counter_state = self.inner[addr % NCOUNTERS];
        counter_state == SaturatingCounter::StronglyTaken
            || counter_state == SaturatingCounter::WeaklyTaken
    }

    pub fn update_state(&mut self, addr: usize, result: bool) {
        self.inner[addr % NCOUNTERS] = if result {
            self.inner[addr % NCOUNTERS].next()
        } else {
            self.inner[addr % NCOUNTERS].prev()
        }
    }
}

impl<const NCOUNTERS: usize> Default for BranchPredictor<NCOUNTERS> {
    fn default() -> Self {
        Self::new()
    }
}
