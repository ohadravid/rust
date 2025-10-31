use crate::sys::thread_local::key::{create, set, Dtor};

pub unsafe fn register(t: *mut u8, dtor: Dtor) {
    let key = create(Some(dtor));
    let _ = unsafe { set(key, t) };
}