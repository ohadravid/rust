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
use crate::sync::atomic::{Atomic, AtomicU32, Ordering};
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
pub unsafe fn get(key: Key) -> *mut u8 {
    unsafe { c::FlsGetValue(key).cast() }
}

#[inline]
pub unsafe fn destroy(key: Key) {
    let r = unsafe { c::FlsFree(key) };
    debug_assert_eq!(r, 0);
}


/// A type for TLS keys that are statically allocated.
///
/// This is basically a `LazyLock<Key>`, but avoids blocking and circular
/// dependencies with the rest of `std`.
pub struct LazyKey {
    /// Inner static TLS key (internals).
    key: Atomic<u32>,
    /// Destructor for the TLS value.
    dtor: Option<Dtor>,
}

// Define a sentinel value that is likely not to be returned
// as a TLS key.
const KEY_SENTVAL: u32 = c::FLS_OUT_OF_INDEXES;

impl LazyKey {
    pub const fn new(dtor: Option<Dtor>) -> LazyKey {
        LazyKey { key: AtomicU32::new(KEY_SENTVAL), dtor }
    }

    #[inline]
    pub fn force(&self) -> super::Key {
        match self.key.load(Ordering::Acquire) {
            KEY_SENTVAL => self.lazy_init() as super::Key,
            n => n as super::Key,
        }
    }

    fn lazy_init(&self) -> u32 {
        let key1 = create(self.dtor);
        let key = if key1 != KEY_SENTVAL {
            key1
        } else {
            let key2 = create(self.dtor);
            unsafe {
                destroy(key1);
            }
            key2
        };
        rtassert!(key != KEY_SENTVAL);
        match self.key.compare_exchange(
            KEY_SENTVAL,
            key,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            // The CAS succeeded, so we've created the actual key
            Ok(_) => key,
            // If someone beat us to the punch, use their key instead
            Err(n) => unsafe {
                destroy(key);
                n
            },
        }
    }
}
