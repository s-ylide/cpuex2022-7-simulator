use std::fmt::Display;

use anyhow::{anyhow, Result};
use nom::{
    character::complete::{i32, multispace0},
    number::complete::recognize_float,
    IResult,
};

use crate::{
    io::Input,
    ty::{Ty::*, Typed, TypedU32},
};

pub struct SldData {
    seq: Vec<TypedU32>,
    read_index: usize,
    info: SldInfo,
}

impl std::ops::Deref for SldData {
    type Target = SldInfo;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl Display for SldData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sld ")?;
        f.debug_list().entry(&self.seq).finish()
    }
}

impl Input for SldData {
    fn inw(&mut self) -> Result<u32> {
        let v = self.seq[self.read_index];
        let Some(v) = v.as_i32() else {
            return Err(anyhow!(
                "attempted to read {} data as {I32}: at index {}",
                v.ty,
                self.read_index
            ));
        };
        self.read_index += 1;
        Ok(v as u32)
    }
    fn finw(&mut self) -> Result<f32> {
        let v = self.seq[self.read_index];
        let Some(v) = v.as_f32() else {
            return Err(anyhow!(
                "attempted to read {} data as {F32}: at index {}",
                v.ty,
                self.read_index
            ));
        };
        self.read_index += 1;
        Ok(v)
    }
}

impl SldData {
    pub fn parse(sld_str: &str) -> Result<Self> {
        let SldDataBuilder { seq, info } =
            SldDataBuilder::from_sld(sld_str).map_err(|n| anyhow!("failed to parse: {n}"))?;
        Ok(Self {
            seq,
            read_index: 0,
            info: info.unwrap(),
        })
    }
}

pub struct SldInfo {
    pub num_objects: usize,
}

struct SldDataBuilder {
    seq: Vec<TypedU32>,
    info: Option<SldInfo>,
}

impl SldDataBuilder {
    fn new() -> Self {
        Self {
            seq: Vec::new(),
            info: None,
        }
    }
    fn push_int(&mut self, v: i32) {
        self.seq.push((v as u32).typed(I32));
    }
    fn push_float(&mut self, v: f32) {
        self.seq.push(v.to_bits().typed(F32));
    }
    fn read_int<'a>(&mut self, input: &'a str) -> IResult<&'a str, i32> {
        let (input, _) = multispace0(input)?;
        let (input, v) = i32(input)?;
        self.push_int(v);
        Ok((input, v))
    }
    fn read_float<'a>(&mut self, input: &'a str) -> IResult<&'a str, ()> {
        let (input, _) = multispace0(input)?;
        let (input, v) = recognize_float(input)?;
        self.push_float(v.parse().unwrap());
        Ok((input, ()))
    }
    fn read_vec3<'a>(&mut self, input: &'a str) -> IResult<&'a str, ()> {
        let (input, _) = self.read_float(input)?;
        let (input, _) = self.read_float(input)?;
        self.read_float(input)
    }
    fn read_sld_env<'a>(&mut self, input: &'a str) -> IResult<&'a str, ()> {
        let (input, _) = self.read_vec3(input)?;
        let (input, _) = self.read_float(input)?;
        let (input, _) = self.read_float(input)?;
        let (input, _) = self.read_int(input)?;
        let (input, _) = self.read_float(input)?;
        let (input, _) = self.read_float(input)?;
        self.read_float(input)
    }
    fn read_objects<'a>(&mut self, input: &'a str) -> IResult<&'a str, usize> {
        let mut input_ = input;
        let mut index = 0;
        loop {
            let (input, id) = self.read_int(input_)?;
            if id == -1 {
                return Ok((input, index));
            } else {
                index += 1;
                let (input, _) = self.read_int(input)?;
                let (input, _) = self.read_int(input)?;
                let (input, is_rot) = self.read_int(input)?;
                let (input, _) = self.read_vec3(input)?;
                let (input, _) = self.read_vec3(input)?;
                let (input, _) = self.read_float(input)?;
                let (input, _) = self.read_float(input)?;
                let (input, _) = self.read_float(input)?;
                let (input, _) = self.read_vec3(input)?;
                input_ = if is_rot != 0 {
                    self.read_vec3(input)?.0
                } else {
                    input
                };
            }
        }
    }
    fn read_and_net<'a>(&mut self, input: &'a str) -> IResult<&'a str, ()> {
        let mut input_ = input;
        loop {
            let (input, id) = self.read_int(input_)?;
            input_ = input;
            if id == -1 {
                break;
            }
            loop {
                let (input, id) = self.read_int(input_)?;
                input_ = input;
                if id == -1 {
                    break;
                }
            }
        }
        Ok((input_, ()))
    }
    fn read_or_net<'a>(&mut self, input: &'a str) -> IResult<&'a str, ()> {
        self.read_and_net(input)
    }
    fn read_sld<'a>(&mut self, input: &'a str) -> IResult<&'a str, SldInfo> {
        let (input, _) = self.read_sld_env(input)?;
        let (input, num_objects) = self.read_objects(input)?;
        let (input, _) = self.read_and_net(input)?;
        let (input, _) = self.read_or_net(input)?;
        let info = SldInfo { num_objects };
        Ok((input, info))
    }
    pub fn from_sld(input: &str) -> Result<Self, nom::Err<nom::error::Error<&str>>> {
        let mut s = Self::new();
        let (_, info) = s.read_sld(input)?;
        s.info = Some(info);
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sld_float() {
        let mut b = SldDataBuilder::new();
        b.read_float("-200.0").unwrap();
        b.read_float("1.0").unwrap();
        assert_eq!(b.seq[0].as_f32().unwrap(), "-200.0".parse::<f32>().unwrap());
        assert_eq!(b.seq[1].as_f32().unwrap(), "1.0".parse::<f32>().unwrap());
    }
    #[test]
    fn test_sld_parse() {
        let sld_str = "-70  35 -20      20 30
1 50 50
255
0 1 1 0    20  20  65    0  20  45  1 1.0 250 128 210   0
0 3 1 0    25  40  70    0   0  40  1 1.0 250 128 210   0
0 3 1 0     0  30  30    0  -5   0 -1 1.0 250 128 211   0
0 1 1 0    20  10  30    0 -10  80  1 1.0 250 128 211   0
0 2 1 0     0 -1.5 -1    0   0  50  1 1.0 250 128 211   0
0 1 1 0    22  28  28    0  -5   0  1 1.0 250   0 211 211
0 3 1 0    40  28  28    0  -5   0  1 1.0 250   0 211 211
0 3 1 0     0  15  15    0  -5   0 -1 1.0 250   0 211 211
0 3 1 0    15  25  25    0  -5  70  1 1.0 250 211   0   0
0 1 1 0     5  11  45    0  35  40  1 1.0 250 211 128   0
0 3 1 0    30  45  75    0   0  40  1 1.0 250 211 128   0
0 1 1 0    25  41  70    0   5  40  1 1.0 250   0   0   0
1 1 1 0   100   5 200    0 -35 150  1 1.0 250 200 200 200
0 3 1 0    25  10  10    0  -5   0  1 1.0 250 211 128 128
0 3 2 0    25  20  20    0   0  70  1 0.3   0   0   0 255
2 3 1 0	   20  20  20  100  40 120  1 1.0 150 255 255 255
0 2 2 0     0   0  -1    0   0 200  1 0.2   0 255   0   0     
-1
0 1 2 -1
3 1 4 -1
5 6 7 -1
8 -1
9 10 -1
12 -1
13 -1
14 -1
15 -1
16 -1
-1
11 0 1 2 3 4 6 -1
99 9 8 7 5 -1
-1
";
        let b = SldData::parse(sld_str).unwrap();
        assert_eq!(b.seq.len(), 325);
        assert_eq!(b.seq[1].as_f32().unwrap(), "35".parse::<f32>().unwrap());
        assert_eq!(
            b.seq.last().unwrap().as_i32().unwrap(),
            "-1".parse::<i32>().unwrap()
        );
    }
}
