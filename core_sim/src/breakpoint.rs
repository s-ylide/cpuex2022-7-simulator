use std::fmt::Display;

use crate::{
    memory::Addr,
    register::{FRegId, RegId},
};

pub struct BreakPoint {
    pub addr: Addr,
    pub cond: Option<BreakPointCond>,
}

impl Display for BreakPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.addr)
    }
}

impl BreakPoint {
    pub fn new(addr: Addr) -> Self {
        Self {
            addr,
            cond: Default::default(),
        }
    }
}

impl std::borrow::Borrow<Addr> for BreakPoint {
    fn borrow(&self) -> &Addr {
        &self.addr
    }
}

pub enum BreakPointCond {
    OrdChain(OrdChain),
    NotEq(BreakPointExpr, BreakPointExpr),
    Neg(Box<Self>),
    Conj(Vec<Self>),
    Disj(Vec<Self>),
}

pub struct OrdChain {
    pub chain_kind: OrdChainKind,
    pub head: BreakPointExpr,
    pub tail: Vec<(OrdOpKind, BreakPointExpr)>,
}

impl Display for BreakPointCond {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BreakPointCond::OrdChain(OrdChain {
                chain_kind,
                head,
                tail,
            }) => {
                write!(f, "{head}")?;
                for (op, e) in tail {
                    let op = op.str(chain_kind);
                    write!(f, " {op} {e}")?;
                }
                Ok(())
            }
            BreakPointCond::NotEq(e1, e2) => write!(f, "{e1} != {e2}"),
            BreakPointCond::Neg(s) => write!(f, "!({s})"),
            BreakPointCond::Conj(v) => write!(
                f,
                "{}",
                v.iter()
                    .map(|c| format!("{c}"))
                    .collect::<Vec<_>>()
                    .join(" && ")
            ),
            BreakPointCond::Disj(v) => write!(
                f,
                "{}",
                v.iter()
                    .map(|c| format!("{c}"))
                    .collect::<Vec<_>>()
                    .join(" || ")
            ),
        }
    }
}

#[derive(Default)]
pub enum OrdChainKind {
    #[default]
    Less,
    Greater,
}

pub enum OrdOpKind {
    Eq,
    Weak,
    Strong,
}

impl OrdOpKind {
    fn str(&self, k: &OrdChainKind) -> &'static str {
        use OrdChainKind::*;
        use OrdOpKind::*;
        match (k, self) {
            (Less, Eq) => "==",
            (Less, Weak) => "<=",
            (Less, Strong) => "<",
            (Greater, Eq) => "==",
            (Greater, Weak) => ">=",
            (Greater, Strong) => ">",
        }
    }
}

pub enum BreakPointExpr {
    Int(u32),
    Float(f32),
    Reg(RegId),
    FReg(FRegId),
    Mem(Addr),
}

impl Display for BreakPointExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BreakPointExpr::Int(x) => write!(f, "{x}"),
            BreakPointExpr::Float(x) => write!(f, "{x}"),
            BreakPointExpr::Reg(r) => write!(f, "{r}"),
            BreakPointExpr::FReg(r) => write!(f, "{r}"),
            BreakPointExpr::Mem(a) => write!(f, "M[{a}]"),
        }
    }
}

impl BreakPointExpr {
    pub fn ty(&self) -> BreakPointExprTy {
        use BreakPointExprTy::*;
        match self {
            BreakPointExpr::Int(..) => Int,
            BreakPointExpr::Float(..) => Float,
            BreakPointExpr::Reg(..) => Int,
            BreakPointExpr::FReg(..) => Float,
            BreakPointExpr::Mem(..) => None,
        }
    }
}

pub enum BreakPointExprTy {
    Int,
    Float,
    None,
}
