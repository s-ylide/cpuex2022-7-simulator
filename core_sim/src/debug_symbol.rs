use std::collections::HashMap;

use anyhow::Result;
use serde::Deserialize;

#[derive(Default, Deserialize)]
struct DebugSymbolRaw {
    globals: Vec<SymbolDefRaw>,
    labels: Vec<SymbolDefRaw>,
}

impl DebugSymbolRaw {
    pub fn deser(file: impl std::io::Read) -> Result<DebugSymbolRaw> {
        Ok(serde_json::from_reader(file)?)
    }
}

#[derive(Default)]
pub struct DebugSymbol {
    pub globals: HashMap<String, SymbolDefRaw>,
    pub sorted: Vec<SymbolDefRaw>,
}

impl DebugSymbol {
    pub fn is_empty(&self) -> bool {
        self.globals.is_empty() && self.sorted.is_empty()
    }
    fn sort(&mut self) {
        self.sorted.sort_by_key(SymbolDefRaw::addr);
    }
    pub fn get_nearest_symbol_addr(&self, addr: u32) -> Result<SymbolTableIndex> {
        if self.sorted.is_empty() {
            return Err(anyhow::anyhow!("debug symbol not provided."));
        }
        Ok(
            match self.sorted.binary_search_by_key(&addr, SymbolDefRaw::addr) {
                Ok(index) => SymbolTableIndex(index),
                Err(index) => {
                    if addr < self.sorted[index].addr {
                        if index == 0 {
                            return Err(anyhow::anyhow!(
                                "address {addr:#010} do not have any debug information"
                            ));
                        }
                        SymbolTableIndex(index - 1)
                    } else {
                        SymbolTableIndex(index)
                    }
                }
            },
        )
    }
    pub fn get_exact_symbol_addr(&self, addr: u32) -> Result<SymbolTableIndex> {
        if self.sorted.is_empty() {
            return Err(anyhow::anyhow!("debug symbol not provided."));
        }
        match self.sorted.binary_search_by_key(&addr, SymbolDefRaw::addr) {
            Ok(index) => Ok(SymbolTableIndex(index)),
            Err(_) => Err(anyhow::anyhow!("not available")),
        }
    }
    pub fn get_symbol(&self, index: SymbolTableIndex) -> SymbolDef {
        let raw = &self.sorted[index.0];
        let size = self.sorted.get(index.0 + 1).map(|a| a.addr - raw.addr);
        SymbolDef { raw, size }
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.globals.extend(other.globals);
        self.sorted.extend(other.sorted);
        self.sort();
    }

    pub fn deser(file: impl std::io::Read) -> Result<Self> {
        let raw = DebugSymbolRaw::deser(file)?;
        Ok(Self {
            sorted: raw.labels,
            globals: raw
                .globals
                .into_iter()
                .map(|v| (v.label.clone(), v))
                .collect(),
        })
    }

    pub fn get_global(&self, k: &str) -> Option<&SymbolDefRaw> {
        self.globals.get(k)
    }
}

pub struct SymbolTableIndex(usize);

pub struct SymbolDef<'a> {
    raw: &'a SymbolDefRaw,
    pub size: Option<u32>,
}

impl<'a> std::ops::Deref for SymbolDef<'a> {
    type Target = &'a SymbolDefRaw;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

#[derive(Deserialize, Debug)]
pub struct SymbolDefRaw {
    pub addr: u32,
    pub label: String,
}

impl SymbolDefRaw {
    fn addr(&self) -> u32 {
        self.addr
    }
}
