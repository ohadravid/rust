//! Support for Windows TLS destructors.
//!

use core::ffi::c_void;

use crate::cell::Cell;
use crate::ptr;
use crate::sys::c;
use crate::sys::thread_local::destructors;

pub type Key = u32;

unsafe fn create(dtor: c::PFLS_CALLBACK_FUNCTION) -> Key {
    let key_result = unsafe { c::FlsAlloc(dtor) };

    if key_result == c::FLS_OUT_OF_INDEXES {
        rtabort!("out of FLS keys");
    }

    key_result
}

unsafe fn set(key: Key, ptr: *const c_void) {
    let result = unsafe { c::FlsSetValue(key, ptr) };

    if result == c::FALSE {
        rtabort!("failed to set FLS value");
    }
}

pub fn enable() {
    #[thread_local]
    static REGISTERED: Cell<bool> = Cell::new(false);

    if !REGISTERED.replace(true) {
        unsafe {
            let key = create(Some(cleanup));
            set(key, ptr::dangling());
        };
    }
}

unsafe extern "system" fn cleanup(_ptr: *const c_void) {
    unsafe {
        destructors::run();
    }

    crate::rt::thread_cleanup();
}
