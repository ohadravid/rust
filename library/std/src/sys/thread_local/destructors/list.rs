use crate::cell::RefCell;
use crate::sys::thread_local::guard;

#[cfg(not(target_os = "windows"))]
type Dtor = unsafe extern "C" fn(*mut u8);
#[cfg(target_os = "windows")]
type Dtor = unsafe extern "system" fn(*const core::ffi::c_void);

#[thread_local]
static DTORS: RefCell<Vec<(*mut u8, Dtor)>> = RefCell::new(Vec::new());

pub unsafe fn register(t: *mut u8, dtor: Dtor) {
    let Ok(mut dtors) = DTORS.try_borrow_mut() else {
        // This point can only be reached if the global allocator calls this
        // function again.
        // FIXME: maybe use the system allocator instead?
        rtabort!("the global allocator may not use TLS with destructors");
    };

    guard::enable();

    dtors.push((t, dtor));
}

/// The [`guard`] module contains platform-specific functions which will run this
/// function on thread exit if [`guard::enable`] has been called.
///
/// # Safety
///
/// May only be run on thread exit to guarantee that there are no live references
/// to TLS variables while they are destroyed.
pub unsafe fn run() {
    loop {
        let mut dtors = DTORS.borrow_mut();
        match dtors.pop() {
            Some((t, dtor)) => {
                drop(dtors);
                unsafe {
                    dtor(t as *mut _);
                }
            }
            None => {
                // Free the list memory.
                *dtors = Vec::new();
                break;
            }
        }
    }
}
