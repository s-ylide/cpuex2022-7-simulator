#[cfg(feature = "fpu_sim")]
#[link(name = "fpu")]
#[allow(warnings)]
mod binding {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[cfg(feature = "fpu_sim")]
#[allow(unused)]
pub mod fpu {
    use super::*;
    #[inline]
    pub fn fmul(arg1: f32, arg2: f32) -> f32 {
        unsafe { binding::fmul(arg1, arg2) }
    }

    #[inline]
    pub fn fdiv(arg1: f32, arg2: f32) -> f32 {
        unsafe { binding::fdiv(arg1, arg2) }
    }

    #[inline]
    pub fn fsqrt(arg1: f32) -> f32 {
        unsafe { binding::fsqrt(arg1) }
    }

    #[inline]
    pub fn fcvtsw(arg1: i32) -> f32 {
        unsafe { binding::fcvtsw(arg1) }
    }

    #[inline]
    pub fn fcvtws(arg1: f32) -> i32 {
        unsafe { binding::fcvtws(arg1) }
    }

    #[inline]
    pub fn ffloor(arg1: f32) -> f32 {
        unsafe { binding::ffloor(arg1) }
    }

    #[inline]
    pub fn fhalf(arg1: f32) -> f32 {
        unsafe { binding::fmul(arg1, 0.5) }
    }

    #[inline]
    pub fn ffrac(arg1: f32) -> f32 {
        unsafe { arg1 - binding::ffloor(arg1) }
    }

    #[inline]
    pub fn finv(arg1: f32) -> f32 {
        unsafe { binding::fdiv(1.0, arg1) }
    }
}

#[cfg(not(feature = "fpu_sim"))]
#[allow(unused)]
pub mod fpu {
    #[inline]
    pub fn fmul(arg1: f32, arg2: f32) -> f32 {
        arg1 * arg2
    }

    #[inline]
    pub fn fdiv(arg1: f32, arg2: f32) -> f32 {
        arg1 / arg2
    }

    #[inline]
    pub fn fsqrt(arg1: f32) -> f32 {
        arg1.sqrt()
    }

    #[inline]
    pub fn fcvtsw(arg1: i32) -> f32 {
        arg1 as f32
    }

    #[inline]
    pub fn fcvtws(arg1: f32) -> i32 {
        arg1.round() as i32
    }

    #[inline]
    pub fn ffloor(arg1: f32) -> f32 {
        arg1.floor()
    }

    #[inline]
    pub fn fhalf(arg1: f32) -> f32 {
        arg1 * 0.5
    }
}
