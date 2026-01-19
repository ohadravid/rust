//@only-target: windows # this directly tests windows-only functions

use std::ffi::c_void;
use std::{ptr, thread};

pub type BOOL = i32;
pub const FALSE: BOOL = 0i32;
pub const TRUE: BOOL = 1i32;

extern "system" {
    fn TlsAlloc() -> u32;
    fn TlsSetValue(key: u32, val: *mut c_void) -> BOOL;
    fn TlsGetValue(key: u32) -> *mut c_void;
    fn TlsFree(key: u32) -> BOOL;

    fn FlsAlloc(lpcallback: Option<unsafe extern "system" fn(lpflsdata: *mut c_void)>) -> u32;
    fn FlsSetValue(key: u32, val: *mut c_void) -> BOOL;
    fn FlsGetValue(key: u32) -> *mut c_void;

    fn IsThreadAFiber() -> BOOL;
}

fn main() {
    let key = unsafe { TlsAlloc() };
    assert_eq!(unsafe { TlsSetValue(key, ptr::without_provenance_mut(1)) }, TRUE);
    assert_eq!(unsafe { TlsGetValue(key).addr() }, 1);
    assert_eq!(unsafe { TlsFree(key) }, TRUE);

    extern "system" fn dtor1(val: *mut c_void) {
        assert_eq!(val.addr(), 1);
        println!("dtor1");
    }

    extern "system" fn dtor2(val: *mut c_void) {
        assert_eq!(val.addr(), 1);
        println!("dtor2");
    }

    assert_eq!(unsafe { IsThreadAFiber() }, FALSE);

    thread::spawn(|| {
        let fls_key_1 = unsafe { FlsAlloc(Some(dtor1)) };
        assert_eq!(unsafe { FlsSetValue(fls_key_1, ptr::without_provenance_mut(1)) }, TRUE);
        assert_eq!(unsafe { FlsGetValue(fls_key_1).addr() }, 1);
        assert_eq!(unsafe { FlsSetValue(fls_key_1, ptr::without_provenance_mut(0)) }, TRUE);

        let fls_key_2 = unsafe { FlsAlloc(Some(dtor2)) };
        assert_eq!(unsafe { FlsSetValue(fls_key_2, ptr::without_provenance_mut(1)) }, TRUE);
        assert_eq!(unsafe { FlsGetValue(fls_key_2).addr() }, 1);
        println!("exiting thread");
    })
    .join()
    .unwrap();
}
