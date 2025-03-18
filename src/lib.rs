use std::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
};

// Functions that are always needed for `MiMalloc`.
unsafe extern "C" {
    fn mi_free(ptr: *mut c_void);

    fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void;

    fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void;

    fn mi_realloc_aligned(ptr: *mut c_void, new_size: usize, alignment: usize) -> *mut c_void;
}

#[derive(Debug, Copy, Clone, Default)]
pub struct MiMalloc;
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
