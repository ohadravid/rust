use crate::sync::atomic::{Atomic, AtomicU32, Ordering};
use crate::sys::c;
use crate::sys::thread_local::key::{Dtor, create, destroy};

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
        match self.key.compare_exchange(KEY_SENTVAL, key, Ordering::Release, Ordering::Acquire) {
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
