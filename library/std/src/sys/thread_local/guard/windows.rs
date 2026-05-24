//! Support for Windows TLS destructors.
//!
//! Windows has an API to provide a destructor for a FLS (fiber local storage) variable,
//! which behaves similarly to a TLS variable for our purpose [1].
//!
//! All TLS destructors are tracked by *us*, not the Windows runtime.
//! This means that we have a global list of destructors for
//! each TLS key or variable that we know about.
//!
//! [1]: https://devblogs.microsoft.com/oldnewthing/20191011-00/?p=102989

use core::ffi::{c_int, c_void};
use core::sync::atomic::{AtomicU32, Ordering};

use crate::cell::Cell;
use crate::ptr;
use crate::sys::c::{self, FLS_OUT_OF_INDEXES};

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

fn is_thread_a_fiber() -> bool {
    let res = unsafe { c::IsThreadAFiber() };
    res == c::TRUE
}

static KEY: AtomicU32 = AtomicU32::new(FLS_OUT_OF_INDEXES);

pub fn enable() {
    // `#[thread_local]` is unavailable on windows-gnu (`target_thread_local` is off).
    // Use `thread_local!`, which falls back to `TlsAlloc` when needed.
    thread_local! {
        static REGISTERED: Cell<bool> = const { Cell::new(false) };
    }

    if !REGISTERED.replace(true) {
        let current_key = KEY.load(Ordering::Acquire);

        // If we already allocated a key, we only need to set it to a non-null value so that the dtor is run.
        let key = if current_key != FLS_OUT_OF_INDEXES {
            current_key
        } else {
            // Otherwise, we try to allocate a key.
            let new_key = unsafe { create(Some(cleanup)) };

            // Now we need to set this key to be used by everyone else.
            // If we won the race, our key is the right one and we can set it to non-null value.
            // If we lost, we'll use the winning key and free our losing key.
            match KEY.compare_exchange(current_key, new_key, Ordering::Release, Ordering::Acquire) {
                Ok(_) => new_key,
                Err(other_key) => {
                    unsafe { c::FlsFree(new_key) };
                    other_key
                }
            }
        };

        // Handle DLL unloading gracefully by freeing the FLS key manually.
        unsafe extern "C" {
            pub fn atexit(cb: unsafe extern "C" fn()) -> c_int;
        }
        let _ = unsafe { atexit(free_fls_key_at_exit) };

        // Setting the key's value to non-zero will cause the dtor callback to be called when the thread exits.
        // We only set the key once per thread, so the destructors are guaranteed to run at most once (fibers cannot be moved between threads).
        unsafe { set(key, ptr::without_provenance(1)) };
    }
}

extern "C" fn free_fls_key_at_exit() {
    // If the current DLL is unloaded, the registered `cleanup` hook will not be available later during thread exit,
    // triggering a `STATUS_ACCESS_VIOLATION`.
    // Manually free the FLS slot to avoid this.
    
    // If the entire process is shutting down, which is the more common case, we don't need to do that.
    if is_shutdown_in_progress() {
        return;
    }
    
    let current_key = KEY.swap(c::FLS_OUT_OF_INDEXES, Ordering::AcqRel);
    if current_key != c::FLS_OUT_OF_INDEXES {
        unsafe { c::FlsFree(current_key) };
    }
}

#[cfg(not(target_vendor = "win7"))]
fn is_shutdown_in_progress() -> bool {
    #[link(name = "ntdll")]
    unsafe extern "system" {
        /// Returns TRUE (non-zero) if the process is terminating.
        /// Returns FALSE (0) if we are dynamically unloading a DLL.
        pub fn RtlDllShutdownInProgress() -> u8;
    }

    unsafe { RtlDllShutdownInProgress() != 0 }
}


#[cfg(target_vendor = "win7")]
fn is_shutdown_in_progress() -> bool { 
    // `RtlDllShutdownInProgress` is unavailable before Windows 10,
    // so assume the process is shutting down and free the FLS key manually
    // to avoid a potential crash during DLL unloading.
    true 
}

unsafe extern "system" fn cleanup(_ptr: *const c_void) {
    // Avoid running the hook if we are in a fiber.
    // This will cause destructors of thread locals to not run, leaking them.
    // Thread-local runtime state will not be cleaned.
    //
    // We need to verify that we won't run the destructors *before* the thread exits,
    // but if the fiber that registered the callback is deleted, the thread might still be running other fibers.
    //
    // By checking that we are not running in a fiber here, we are guaranteed that the hook is only running during the thread's exit.
    // See also the `fiber_does_not_trigger_dtor` test.
    if is_thread_a_fiber() {
        return;
    }

    unsafe {
        #[cfg(target_thread_local)]
        super::super::destructors::run();
        #[cfg(not(target_thread_local))]
        super::super::key::run_dtors();
    }

    crate::rt::thread_cleanup();
}
