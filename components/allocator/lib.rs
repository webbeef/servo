/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Selecting the default global allocator for Servo, and exposing common
//! allocator introspection APIs for memory profiling.

use std::os::raw::c_void;

#[cfg(not(feature = "allocation-tracking"))]
#[global_allocator]
static ALLOC: Allocator = Allocator;

#[cfg(feature = "allocation-tracking")]
#[global_allocator]
static ALLOC: crate::tracking::AccountingAlloc<Allocator> =
    crate::tracking::AccountingAlloc::with_allocator(Allocator);

#[cfg(feature = "allocation-tracking")]
mod tracking;

pub fn is_tracking_unmeasured() -> bool {
    cfg!(feature = "allocation-tracking")
}

pub fn dump_unmeasured(_writer: impl std::io::Write) {
    #[cfg(feature = "allocation-tracking")]
    ALLOC.dump_unmeasured_allocations(_writer);
}

pub use crate::platform::*;

type EnclosingSizeFn = unsafe extern "C" fn(*const c_void) -> usize;

/// # Safety
/// No restrictions. The passed pointer is never dereferenced.
/// This function is only marked unsafe because the MallocSizeOfOps APIs
/// requires an unsafe function pointer.
#[cfg(feature = "allocation-tracking")]
unsafe extern "C" fn enclosing_size_impl(ptr: *const c_void) -> usize {
    let (adjusted, size) = crate::ALLOC.enclosing_size(ptr);
    if size != 0 {
        crate::ALLOC.note_allocation(adjusted, size);
    }
    size
}

#[expect(non_upper_case_globals)]
#[cfg(feature = "allocation-tracking")]
pub static enclosing_size: Option<EnclosingSizeFn> = Some(crate::enclosing_size_impl);

#[expect(non_upper_case_globals)]
#[cfg(not(feature = "allocation-tracking"))]
pub static enclosing_size: Option<EnclosingSizeFn> = None;

#[cfg(not(any(windows, feature = "use-system-allocator", target_env = "ohos")))]
mod platform {
    use std::os::raw::c_void;

    pub use tikv_jemallocator::Jemalloc as Allocator;

    /// Get the size of a heap block.
    ///
    /// # Safety
    ///
    /// Passing a non-heap allocated pointer to this function results in undefined behavior.
    pub unsafe extern "C" fn usable_size(ptr: *const c_void) -> usize {
        let size = unsafe { tikv_jemallocator::usable_size(ptr) };
        #[cfg(feature = "allocation-tracking")]
        crate::ALLOC.note_allocation(ptr, size);
        size
    }

    /// Memory allocation APIs compatible with libc
    pub mod libc_compat {
        pub use tikv_jemalloc_sys::{calloc, free, malloc, realloc};
    }

    pub unsafe fn memalign(alignment: usize, size: usize) -> *mut c_void {
        let mut ptr: *mut c_void = std::ptr::null_mut();
        if unsafe { tikv_jemalloc_sys::posix_memalign(&mut ptr, alignment, size) } == 0 {
            ptr
        } else {
            std::ptr::null_mut()
        }
    }
}

#[cfg(all(
    not(windows),
    any(feature = "use-system-allocator", target_env = "ohos")
))]
mod platform {
    pub use std::alloc::System as Allocator;
    use std::os::raw::c_void;

    /// Get the size of a heap block.
    ///
    /// # Safety
    ///
    /// Passing a non-heap allocated pointer to this function results in undefined behavior.
    pub unsafe extern "C" fn usable_size(ptr: *const c_void) -> usize {
        #[cfg(target_vendor = "apple")]
        unsafe {
            let size = libc::malloc_size(ptr);
            #[cfg(feature = "allocation-tracking")]
            crate::ALLOC.note_allocation(ptr, size);
            size
        }

        #[cfg(not(target_vendor = "apple"))]
        unsafe {
            let size = libc::malloc_usable_size(ptr as *mut _);
            #[cfg(feature = "allocation-tracking")]
            crate::ALLOC.note_allocation(ptr, size);
            size
        }
    }

    pub mod libc_compat {
        pub use libc::{calloc, free, malloc, realloc};
    }

    pub unsafe fn memalign(alignment: usize, size: usize) -> *mut c_void {
        let mut ptr: *mut c_void = std::ptr::null_mut();
        if unsafe { libc::posix_memalign(&mut ptr, alignment, size) } == 0 {
            ptr
        } else {
            std::ptr::null_mut()
        }
    }
}

#[cfg(windows)]
mod platform {
    pub use std::alloc::System as Allocator;
    use std::os::raw::c_void;

    use windows_sys::Win32::Foundation::FALSE;
    use windows_sys::Win32::System::Memory::{GetProcessHeap, HeapSize, HeapValidate};

    /// Get the size of a heap block.
    ///
    /// # Safety
    ///
    /// Passing a non-heap allocated pointer to this function results in undefined behavior.
    pub unsafe extern "C" fn usable_size(mut ptr: *const c_void) -> usize {
        unsafe {
            let heap = GetProcessHeap();

            if HeapValidate(heap, 0, ptr) == FALSE {
                ptr = *(ptr as *const *const c_void).offset(-1)
            }

            let size = HeapSize(heap, 0, ptr) as usize;
            #[cfg(feature = "allocation-tracking")]
            crate::ALLOC.note_allocation(ptr, size);
            size
        }
    }

    pub mod libc_compat {
        pub use libc::{calloc, free, malloc, realloc};
    }

    pub unsafe fn memalign(alignment: usize, size: usize) -> *mut c_void {
        unsafe { libc::aligned_malloc(size, alignment) }
    }
}

/// Bridge symbols for SpiderMonkey — route SM C++ allocations through
/// the same allocator that Servo uses for Rust allocations.
///
/// These symbols are declared `extern "C"` in mozjs-sys's
/// `mozjs_sys_alloc.h` and referenced throughout SpiderMonkey's
/// allocation paths.  The mozjs-sys `custom-allocator` feature
/// suppresses its default (libc) implementations so that these
/// definitions are used instead.
mod mozjs_bridge {
    use std::os::raw::c_void;

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn mozjs_sys_malloc(size: usize) -> *mut c_void {
        unsafe { crate::libc_compat::malloc(size) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn mozjs_sys_calloc(n: usize, size: usize) -> *mut c_void {
        unsafe { crate::libc_compat::calloc(n, size) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn mozjs_sys_realloc(p: *mut c_void, size: usize) -> *mut c_void {
        unsafe { crate::libc_compat::realloc(p, size) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn mozjs_sys_free(p: *mut c_void) {
        unsafe { crate::libc_compat::free(p) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn mozjs_sys_memalign(alignment: usize, size: usize) -> *mut c_void {
        unsafe { crate::memalign(alignment, size) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn mozjs_sys_malloc_usable_size(p: *const c_void) -> usize {
        unsafe { crate::usable_size(p) }
    }
}
