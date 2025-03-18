use std::ffi::{c_char, CStr};
use std::io::{stdout, Write};
use std::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
};

// Functions that are always needed for `MiMalloc`.
unsafe extern "C" {
    fn mi_stats_print_out(
        out: unsafe extern "C" fn(msg: *const c_char, arg: *mut c_void),
        arg: *mut c_void,
    );

    fn mi_free(ptr: *mut c_void);

    fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void;

    fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void;

    fn mi_realloc_aligned(ptr: *mut c_void, new_size: usize, alignment: usize) -> *mut c_void;
}

#[cfg(any(target_env = "gnu", target_env = "musl"))]
mod gnu_or_musl_wrapper {
    use super::*;

    unsafe extern "C" {
        fn mi_malloc(size: usize) -> *mut c_void;

        fn mi_calloc(size: usize, n: usize) -> *mut c_void;

        fn mi_realloc(ptr: *mut c_void, new_size: usize) -> *mut c_void;

        fn mi_strdup(s: *const c_char) -> *mut c_char;

        fn mi_strndup(s: *const c_char, n: usize) -> *mut c_char;

        fn mi_realpath(file_name: *const c_char, resolved_name: *mut c_char) -> *mut c_char;
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_malloc(size: usize) -> *mut c_void {
        unsafe { mi_malloc(size) }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_calloc(size: usize, n: usize) -> *mut c_void {
        unsafe { mi_calloc(size, n) }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_realloc(ptr: *mut c_void, new_size: usize) -> *mut c_void {
        unsafe { mi_realloc(ptr, new_size) }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_free(ptr: *mut c_void) {
        unsafe { mi_free(ptr) }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
        unsafe { mi_malloc_aligned(size, alignment) }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_strdup(s: *const c_char) -> *mut c_char {
        unsafe { mi_strdup(s) }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_strndup(s: *const c_char, n: usize) -> *mut c_char {
        unsafe { mi_strndup(s, n) }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_realpath(
        file_name: *const c_char,
        resolved_name: *mut c_char,
    ) -> *mut c_char {
        unsafe { mi_realpath(file_name, resolved_name) }
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct MiMalloc;
impl MiMalloc {
    #[inline]
    pub fn print_stats() {
        Self::print_stats_to(&mut stdout().lock())
    }

    #[inline]
    pub fn print_stats_to<W: Write>(out: &mut W) {
        unsafe extern "C" fn print_out<W: Write>(msg: *const c_char, out: *mut c_void) {
            let msg = unsafe { CStr::from_ptr(msg) }.to_string_lossy();
            let out = unsafe { &mut *(out as *mut W) };
            _ = write!(out, "{msg}");
        }

        unsafe { mi_stats_print_out(print_out::<W>, out as *mut W as *mut c_void) }
    }
}

unsafe impl GlobalAlloc for MiMalloc {
    #[inline(always)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { mi_malloc_aligned(layout.size(), layout.align()) as *mut u8 }
    }

    #[inline(always)]
    unsafe fn dealloc(&self, ptr: *mut u8, _: Layout) {
        unsafe { mi_free(ptr as *mut c_void) }
    }

    #[inline(always)]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { mi_zalloc_aligned(layout.size(), layout.align()) as *mut u8 }
    }

    #[inline(always)]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { mi_realloc_aligned(ptr as *mut c_void, new_size, layout.align()) as *mut u8 }
    }
}
