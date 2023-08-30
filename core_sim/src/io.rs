use anyhow::{anyhow, Result};

pub trait Input {
    fn inw(&mut self) -> Result<u32>;
    fn finw(&mut self) -> Result<f32>;
}

pub trait Output {
    fn outb(&mut self, c: u8) -> Result<()>;
}

pub struct EmptyIO {}

impl EmptyIO {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for EmptyIO {
    fn default() -> Self {
        Self::new()
    }
}

impl Input for EmptyIO {
    fn inw(&mut self) -> Result<u32> {
        Err(anyhow!("inw called"))
    }

    fn finw(&mut self) -> Result<f32> {
        Err(anyhow!("finw called"))
    }
}

impl Output for EmptyIO {
    fn outb(&mut self, _: u8) -> Result<()> {
        Err(anyhow!("outb called"))
    }
}

pub struct BinaryInput {
    content: Vec<u8>,
    read_index: usize,
}

impl Input for BinaryInput {
    fn inw(&mut self) -> Result<u32> {
        let mut v: [u8; 4] = [0; 4];
        let addr = self.read_index;
        v[..4].copy_from_slice(&self.content[addr..(4 + addr)]);
        Ok(u32::from_le_bytes(v))
    }

    fn finw(&mut self) -> Result<f32> {
        let mut v: [u8; 4] = [0; 4];
        let addr = self.read_index;
        v[..4].copy_from_slice(&self.content[addr..(4 + addr)]);
        Ok(f32::from_le_bytes(v))
    }
}

impl BinaryInput {
    pub fn new(content: Vec<u8>) -> Self {
        Self {
            content,
            read_index: 0,
        }
    }
}

pub struct BinaryOutput {
    content: Vec<u8>,
}

impl BinaryOutput {
    pub fn into_inner(self) -> Vec<u8> {
        self.content
    }
    pub fn new() -> Self {
        Self {
            content: Vec::new(),
        }
    }
}

impl Default for BinaryOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl Output for BinaryOutput {
    fn outb(&mut self, c: u8) -> Result<()> {
        self.content.push(c);
        Ok(())
    }
}
