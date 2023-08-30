pub const CACHE_NUM_LINES: usize = 16384usize;

pub struct Cache<const NLINES: usize> {
    inner: Vec<u32>,
}

impl<const NLINES: usize> Cache<NLINES> {
    pub fn new() -> Self {
        Self {
            inner: vec![0; NLINES],
        }
    }
    pub fn access_cache(&mut self, addr: usize) -> bool {
        let line = (addr >> 2) % NLINES;
        let tag = (addr >> 2) / NLINES;
        if self.inner[line] != tag as u32 {
            self.inner[line] = tag as u32;
            false
        } else {
            true
        }
    }
}

impl<const NLINES: usize> Default for Cache<NLINES> {
    fn default() -> Self {
        Self::new()
    }
}
