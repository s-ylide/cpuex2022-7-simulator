use std::ops::Range;

/// sign extends immediate. assumes `sign` is MSB of `imm`.
pub const fn sign_extend<const BIT: u32>(sign: u32, imm: u32) -> u32 {
    if sign != 0 {
        bit_range(BIT..31) | imm
    } else {
        imm
    }
}

#[inline]
pub const fn bit_range(r: Range<u32>) -> u32 {
    let large: u32 = if r.end != 31 { 1 << (r.end + 1) } else { 0 };
    large.wrapping_sub(1 << r.start)
}

#[inline]
pub const fn bit_range_lower(index: u32) -> u32 {
    let large: u32 = if index != 31 { 1 << (index + 1) } else { 0 };
    large.wrapping_sub(1)
}

#[inline]
pub const fn mask(bin: u32, r: Range<u32>) -> u32 {
    bin & bit_range(r)
}

#[inline]
pub const fn mask_lower(bin: u32, index: u32) -> u32 {
    bin & bit_range_lower(index)
}

#[inline]
pub const fn extract(bin: u32, r: Range<u32>) -> u32 {
    let left = r.start;
    mask(bin, r) >> left
}

#[inline]
pub const fn at(bin: u32, index: u32) -> u32 {
    (bin >> index) & 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_range() {
        assert_eq!(0b1, bit_range(0..0));
        assert_eq!(0b110000, bit_range(4..5));
        assert_eq!(0b1111111, bit_range(0..6));
    }
    #[test]
    fn test_sign_extend() {
        // 12-bits (I-fmt imm, etc)
        let l = -4i32 as u32;
        let r = sign_extend::<12>(1, 0xffc);
        assert_eq!(l, r, "left: {l:#X}; right: {r:#X}");
        // 13-bits (B-fmt imm)
        let l = -4i32 as u32;
        let r = sign_extend::<12>(1, 0x1ffc);
        assert_eq!(l, r, "left: {l:#X}; right: {r:#X}");
    }
}
