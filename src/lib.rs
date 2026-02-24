#![doc = include_str!("../README.md")]
#![cfg_attr(doc, deny(missing_docs))]

use std::{
    alloc::{GlobalAlloc, Layout},
    ffi::{c_int, c_void},
    fmt::{Display, Formatter, Result as FmtResult},
};

// Functions that are always needed for `MiMalloc`.
unsafe extern "C" {
    fn mi_version() -> c_int;

    fn mi_free(ptr: *mut c_void);

    fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void;

    fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void;

    fn mi_realloc_aligned(ptr: *mut c_void, new_size: usize, alignment: usize) -> *mut c_void;
}

#[cfg(any(
    all(
        not(target_os = "windows"),
        any(target_env = "gnu", target_env = "musl")
    ),
    target_os = "android",
))]
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
    extern "C" fn __wrap_malloc(size: usize) -> *mut c_void {
        unsafe { mi_malloc(size) }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_calloc(count: usize, size: usize) -> *mut c_void {
        unsafe { mi_calloc(count, size) }
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

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_posix_memalign(
        out: *mut *mut c_void,
        alignment: usize,
        size: usize,
    ) -> c_int {
        if alignment < size_of::<usize>() || !alignment.is_power_of_two() {
            return libc::EINVAL;
        }

        match unsafe { mi_malloc_aligned(size, alignment) } {
            ptr if ptr.is_null() => libc::ENOMEM,
            ptr => {
                unsafe { out.write(ptr) }
                0
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_wrapper {
    use std::{ffi::c_void, ptr::null_mut};

    use super::*;

    unsafe extern "C" {
        fn mi_usable_size(ptr: *mut c_void) -> usize;

        fn mi_reallocf(ptr: *mut c_void, new_size: usize) -> *mut c_void;
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_memalign(alignment: usize, size: usize) -> *mut c_void {
        unsafe { mi_malloc_aligned(size, alignment) }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_valloc(size: usize) -> *mut c_void {
        match usize::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }) {
            Ok(page_size) if page_size > 0 => unsafe { mi_malloc_aligned(size, page_size) },
            _ => {
                unsafe { libc::__errno_location().write(libc::EINVAL) }
                null_mut()
            }
        }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_pvalloc(size: usize) -> *mut c_void {
        match usize::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }) {
            Ok(page_size) if page_size > 0 => {
                let alloc_size = size.div_ceil(page_size) * page_size;
                unsafe { mi_malloc_aligned(alloc_size, page_size) }
            }
            _ => {
                unsafe { libc::__errno_location().write(libc::EINVAL) }
                null_mut()
            }
        }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_malloc_usable_size(ptr: *mut c_void) -> usize {
        unsafe { mi_usable_size(ptr) }
    }

    #[unsafe(no_mangle)]
    extern "C" fn __wrap_reallocf(ptr: *mut c_void, new_size: usize) -> *mut c_void {
        unsafe { mi_reallocf(ptr, new_size) }
    }
}

/// Version struct returned by [`MiMalloc::get_version`].
#[derive(Debug, Copy, Clone, Default)]
pub struct Version {
    /// The major version.
    pub major: u8,
    /// The minor version.
    pub minor: u8,
    /// The patch version.
    pub patch: u8,
}

impl Display for Version {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Redirection to MiMalloc, usable with `#[global_allocator]` like so:
/// ```
/// use mimalloc_redirect::MiMalloc;
///
/// #[global_allocator]
/// static ALLOCATOR: MiMalloc = MiMalloc;
/// ```
///
/// See the [crate-level documentation](crate) for more information.
#[derive(Debug, Copy, Clone, Default)]
pub struct MiMalloc;
impl MiMalloc {
    /// Obtains the built-in MiMalloc version, which is currently `v3.2.8`.
    #[inline]
    pub fn get_version() -> Version {
        // MiMalloc v3 calculates version as `1000 * major + 100 * minor + patch`.
        let version = unsafe { mi_version() as i32 };
        Version {
            major: (version / 1000) as u8,
            minor: ((version / 100) % 10) as u8,
            patch: (version % 100) as u8,
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
