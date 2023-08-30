use anyhow::Result;

use crate::io::Output;

pub type PPMData = PPMDataV6;

pub struct PPMDataV6 {
    inner: Vec<u8>,
}

impl Output for PPMDataV6 {
    fn outb(&mut self, c: u8) -> Result<()> {
        self.inner.push(c);
        Ok(())
    }
}

#[derive(Debug)]
pub struct PPMHeaderInfo {
    pub width: u32,
    pub height: u32,
    pub color: u32,
}

impl PPMDataV6 {
    pub fn into_inner(self) -> Vec<u8> {
        self.inner
    }
    pub fn new() -> Self {
        Self {
            inner: Vec::with_capacity(128 * 128 * 3),
        }
    }
    pub fn verify_header(&self) -> Result<PPMHeaderInfo> {
        Ok(Self::parse_ppmv6_header(self.inner.as_slice())
            .map_err(|e| {
                anyhow::anyhow!("invalid header had been generated. failed to parse header: {e}")
            })?
            .1)
    }
    fn parse_ppmv6_header(input: &[u8]) -> nom::IResult<&[u8], PPMHeaderInfo> {
        use nom::bytes::complete::*;
        use nom::character::complete::*;
        use nom::sequence::Tuple;
        let (input, (_, _, width, _, height, _, color, _)) = (
            tag(b"P6"),
            multispace1,
            // width
            u32,
            multispace1,
            // height
            u32,
            multispace1,
            // color
            u32,
            multispace1,
        )
            .parse(input)?;

        Ok((
            input,
            PPMHeaderInfo {
                width,
                height,
                color,
            },
        ))
    }
}

impl Default for PPMDataV6 {
    fn default() -> Self {
        Self::new()
    }
}
