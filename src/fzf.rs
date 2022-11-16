use std::slice;

use crate::fzf_sys as ffi;

#[derive(Debug)]
pub struct Slab(*mut ffi::fzf_slab_t);

impl Slab {
    #[inline]
    pub fn new(size_16: usize, size_32: usize) -> Self {
        let config = ffi::fzf_slab_config_t { size_16, size_32 };
        let slab = unsafe { ffi::fzf_make_slab(config) };
        Self(slab)
    }
}

impl Default for Slab {
    #[inline]
    fn default() -> Self {
        let slab = unsafe { ffi::fzf_make_default_slab() };
        Self(slab)
    }
}

impl Drop for Slab {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            ffi::fzf_free_slab(self.0);
        }
    }
}

#[derive(Debug)]
pub struct Pattern(*mut ffi::fzf_pattern_t);

impl Pattern {
    #[inline]
    pub fn new(pattern: &str, case_mode: CaseMode, fuzzy: bool) -> Self {
        let pattern_obj = unsafe {
            ffi::fzf_parse_pattern(
                case_mode as u32,
                false,
                pattern.as_ptr() as *mut i8,
                pattern.len(),
                fuzzy,
            )
        };
        Self(pattern_obj)
    }
}

impl Drop for Pattern {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            ffi::fzf_free_pattern(self.0);
        }
    }
}

#[repr(u32)]
#[derive(Debug)]
pub enum CaseMode {
    Smart = ffi::fzf_case_types_CaseSmart,
    Ignore = ffi::fzf_case_types_CaseIgnore,
    Respect = ffi::fzf_case_types_CaseRespect,
}

#[derive(Debug)]
pub struct Positions(*mut ffi::fzf_position_t);

impl Positions {
    #[inline]
    pub fn as_slice(&self) -> &[u32] {
        unsafe { slice::from_raw_parts((*self.0).data, (*self.0).size) }
    }
}

impl Drop for Positions {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            ffi::fzf_free_positions(self.0);
        }
    }
}

#[inline]
pub fn get_score(line: &str, pattern: &Pattern, slab: &Slab) -> i32 {
    unsafe { ffi::fzf_get_score(line.as_ptr() as *const i8, line.len(), pattern.0, slab.0) }
}

#[inline]
pub fn get_pos(line: &str, pattern: &Pattern, slab: &Slab) -> Positions {
    let positions = unsafe {
        ffi::fzf_get_positions(line.as_ptr() as *const i8, line.len(), pattern.0, slab.0)
    };
    Positions(positions)
}
