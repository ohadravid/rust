//! Implementation of `LazyKey` for Windows.
//!
//! Windows support is based on the fibers API.
//! A `LazyKey` implementation using racy initialization.
//!
//! Unfortunately, none of the platforms currently supported by `std` allows
//! creating TLS keys at compile-time. Thus we need a way to lazily create keys.
//! Instead of blocking API like `OnceLock`, we use racy initialization, which
//! should be more lightweight and avoids circular dependencies with the rest of
//! `std`.
use crate::sys::c;

pub type Key = u32;
pub type Dtor = unsafe extern "system" fn(*const core::ffi::c_void);

#[inline]
pub fn create(dtor: Option<Dtor>) -> Key {
    let key_result = unsafe { c::FlsAlloc(dtor) };

    if key_result == c::FLS_OUT_OF_INDEXES {
        rtabort!("out of TLS keys");
    }

    key_result
}

#[inline]
pub unsafe fn set(key: Key, val: *mut u8) {
    let r = unsafe { c::FlsSetValue(key, val.cast()) };
    debug_assert_eq!(r, c::TRUE);
}

#[inline]
#[cfg(any(not(target_thread_local), test))]
pub unsafe fn get(key: Key) -> *mut u8 {
    unsafe { c::FlsGetValue(key).cast() }
}

#[inline]
#[cfg(any(not(target_thread_local), test))]
pub unsafe fn destroy(key: Key) {
    let r = unsafe { c::FlsFree(key) };
    debug_assert_eq!(r, 0);
}
