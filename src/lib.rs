#![cfg_attr(feature = "nightly", feature(allocator_api))]

use std::{
    ffi::{c_int, c_void},
    fmt::{Display, Formatter, Result as FmtResult},
    ptr::{slice_from_raw_parts_mut, NonNull},
};

use allocator_api2::alloc::{AllocError, Allocator, GlobalAlloc, Layout};

// Functions that are always needed for `MiMalloc`.
unsafe extern "C" {
    fn mi_version() -> c_int;

    fn mi_free(ptr: *mut c_void);

    fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void;

    fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void;

    fn mi_realloc_aligned(ptr: *mut c_void, new_size: usize, alignment: usize) -> *mut c_void;
}

#[cfg(all(not(target_os = "windows"), any(target_env = "gnu", target_env = "musl")))]
mod gnu_or_musl_wrapper {
    use std::ffi::c_char;

    use super::*;

    unsafe extern "C" {
        fn mi_malloc(size: usize) -> *mut c_void;

        fn mi_calloc(count: usize, size: usize) -> *mut c_void;

        fn mi_realloc(ptr: *mut c_void, new_size: usize) -> *mut c_void;

        fn mi_strdup(s: *const c_char) -> *mut c_char;

        fn mi_strndup(s: *const c_char, n: usize) -> *mut c_char;

        fn mi_realpath(file_name: *const c_char, resolved_name: *mut c_char) -> *mut c_char;
    }

    #[unsafe(no_mangle)]
    #[inline]
    extern "C" fn __wrap_malloc(size: usize) -> *mut c_void {
        unsafe { mi_malloc(size) }
    }

    #[unsafe(no_mangle)]
    #[inline]
    extern "C" fn __wrap_calloc(count: usize, size: usize) -> *mut c_void {
        unsafe { mi_calloc(count, size) }
    }

    #[unsafe(no_mangle)]
    #[inline]
    extern "C" fn __wrap_realloc(ptr: *mut c_void, new_size: usize) -> *mut c_void {
        unsafe { mi_realloc(ptr, new_size) }
    }

    #[unsafe(no_mangle)]
    #[inline]
    extern "C" fn __wrap_free(ptr: *mut c_void) {
        unsafe { mi_free(ptr) }
    }

    #[unsafe(no_mangle)]
    #[inline]
    extern "C" fn __wrap_aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
        unsafe { mi_malloc_aligned(size, alignment) }
    }

    #[unsafe(no_mangle)]
    #[inline]
    extern "C" fn __wrap_strdup(s: *const c_char) -> *mut c_char {
        unsafe { mi_strdup(s) }
    }

    #[unsafe(no_mangle)]
    #[inline]
    extern "C" fn __wrap_strndup(s: *const c_char, n: usize) -> *mut c_char {
        unsafe { mi_strndup(s, n) }
    }

    #[unsafe(no_mangle)]
    #[inline]
    extern "C" fn __wrap_realpath(file_name: *const c_char, resolved_name: *mut c_char) -> *mut c_char {
        unsafe { mi_realpath(file_name, resolved_name) }
    }

    #[unsafe(no_mangle)]
    #[inline]
    extern "C" fn __wrap_posix_memalign(out: *mut *mut c_void, alignment: usize, size: usize) -> c_int {
        if alignment < size_of::<usize>() || !alignment.is_power_of_two() {
            return libc::EINVAL
        }

        match unsafe { mi_malloc_aligned(size, alignment) } {
            ptr if ptr.is_null() => libc::ENOMEM,
            ptr => {
                unsafe { out.write(ptr) }
                0
            }
        }
    }

    #[unsafe(no_mangle)]
    #[inline]
    extern "C" fn __wrap_memalign(alignment: usize, size: usize) -> *mut c_void {
        unsafe { mi_malloc_aligned(size, alignment) }
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl Display for Version {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct MiMalloc;
impl MiMalloc {
    #[inline]
    pub fn get_version() -> Version {
        let version = unsafe { mi_version() as i32 };
        Version {
            major: ((version / 100) % 10) as u8,
            minor: ((version / 10) % 10) as u8,
            patch: (version % 10) as u8,
        }
    }
}

unsafe impl Allocator for MiMalloc {
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        match layout.size() {
            0 => Ok(NonNull::<[u8; 0]>::dangling()),
            size => NonNull::new(slice_from_raw_parts_mut(
                unsafe { mi_malloc_aligned(size, layout.align()) as *mut u8 },
                size,
            ))
            .ok_or(AllocError),
        }
    }

    #[inline]
    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        match layout.size() {
            0 => Ok(NonNull::<[u8; 0]>::dangling()),
            size => NonNull::new(slice_from_raw_parts_mut(
                unsafe { mi_zalloc_aligned(size, layout.align()) as *mut u8 },
                size,
            ))
            .ok_or(AllocError),
        }
    }

    #[inline]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        if layout.size() != 0 {
            unsafe { mi_free(ptr.as_ptr() as *mut c_void) }
        }
    }

    #[inline]
    unsafe fn grow(&self, ptr: NonNull<u8>, old_layout: Layout, new_layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        match (old_layout.size(), new_layout.size()) {
            // Assume `ptr` is dangling if `old_layout.size() == 0`.
            (0, ..) => self.allocate(new_layout),
            // Safety requirements guarantee `new_layout.size() >= old_layout.size()`.
            (.., new_size) => NonNull::new(slice_from_raw_parts_mut(
                unsafe { mi_realloc_aligned(ptr.as_ptr() as *mut c_void, new_size, new_layout.align()) as *mut u8 },
                new_size,
            ))
            .ok_or(AllocError),
        }
    }

    #[inline]
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        match (old_layout.size(), new_layout.size()) {
            // Assume `ptr` is dangling if `old_layout.size() == 0`.
            (0, ..) => self.allocate_zeroed(new_layout),
            // Safety requirements guarantee `new_layout.size() >= old_layout.size()`.
            (old_size, new_size) => {
                match unsafe { mi_realloc_aligned(ptr.as_ptr() as *mut c_void, new_size, new_layout.align()) as *mut u8 } {
                    ptr if ptr.is_null() => Err(AllocError),
                    ptr => unsafe {
                        // `mi_rezalloc_aligned` requires that the pointer was allocated with `mi_rezalloc`.
                        // Unfortunately, that's not required by Rust, so we manually write 0s.
                        ptr.add(old_size).write_bytes(0, new_size.unchecked_sub(old_size));
                        Ok(NonNull::new_unchecked(slice_from_raw_parts_mut(ptr, new_size)))
                    },
                }
            }
        }
    }

    #[inline]
    unsafe fn shrink(&self, ptr: NonNull<u8>, old_layout: Layout, new_layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        match new_layout.size() {
            0 => {
                unsafe { self.deallocate(ptr, old_layout) }
                Ok(NonNull::<[u8; 0]>::dangling())
            }
            // Safety requirements guarantee `new_layout.size() <= old_layout.size()`.
            new_size => NonNull::new(slice_from_raw_parts_mut(
                unsafe { mi_realloc_aligned(ptr.as_ptr() as *mut c_void, new_size, new_layout.align()) as *mut u8 },
                new_size,
            ))
            .ok_or(AllocError),
        }
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
