#[cfg(windows)]
mod libloading {
    type BOOL = i32;
    type DWORD = u32;
    type HANDLE = isize;
    pub type HMODULE = isize;
    type FARPROC = *mut core::ffi::c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryExW(filename: *const u16, file: HANDLE, flags: DWORD) -> HMODULE;
        fn FreeLibrary(module: HMODULE) -> BOOL;
        fn GetProcAddress(module: HMODULE, procname: *const u8) -> FARPROC;
    }

    fn wide_null(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(core::iter::once(0)).collect()
    }

    fn ansi_null(s: &str) -> Vec<u8> {
        assert!(!s.as_bytes().contains(&0), "symbol name must not contain interior NUL");

        s.as_bytes().iter().copied().chain(core::iter::once(0)).collect()
    }

    pub fn load_foo() -> Option<HMODULE> {
        let filename = wide_null("foo.dll");

        let handle = unsafe { LoadLibraryExW(filename.as_ptr(), 0, 0) };

        if handle == 0 { None } else { Some(handle) }
    }

    pub fn get_extern_fn_1(handle: HMODULE) -> Option<FARPROC> {
        let symbol_name = ansi_null("extern_fn_1");

        let symbol = unsafe { GetProcAddress(handle, symbol_name.as_ptr()) };

        if symbol.is_null() { None } else { Some(symbol) }
    }

    pub fn get_extern_fn_2(handle: HMODULE) -> Option<FARPROC> {
        let symbol_name = ansi_null("extern_fn_2");

        let symbol = unsafe { GetProcAddress(handle, symbol_name.as_ptr()) };

        if symbol.is_null() { None } else { Some(symbol) }
    }

    pub fn unload(handle: HMODULE) {
        unsafe {
            FreeLibrary(handle);
        }
    }
}

use std::mem;
type ExternFn = unsafe extern "C" fn(u32, u32) -> u32;

#[cfg(windows)]
fn main() {
    let foo_handle = libloading::load_foo().expect("Failed to load library");
    println!("loaded library");

    let extern_fn_1 = libloading::get_extern_fn_1(foo_handle).expect("Failed to find symbol");
    let extern_fn_1: ExternFn = unsafe { mem::transmute(extern_fn_1) };
    let result = unsafe { extern_fn_1(2, 3) };
    println!("result of extern_fn_1(2, 3): {}", result);

    let extern_fn_2 = libloading::get_extern_fn_2(foo_handle).expect("Failed to find symbol");
    let extern_fn_2: ExternFn = unsafe { mem::transmute(extern_fn_2) };
    let result = unsafe { extern_fn_2(2, 3) };
    println!("result of extern_fn_2(2, 3): {}", result);

    libloading::unload(foo_handle);
    println!("unloaded library");
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This example only runs on Windows.");
}
