#[cfg(all(windows, target_arch = "x86"))]
#[link(
    name = "kernel32.dll",
    kind = "raw-dylib",
    modifiers = "+verbatim",
    import_name_type = "undecorated"
)]
extern "system" {
    pub fn GetBinaryTypeW(app_name: *const u16, bin_type: *mut u32) -> i32;
    pub fn GetLastError() -> u32;
}

#[cfg(all(windows, not(target_arch = "x86")))]
#[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
extern "system" {
    pub fn GetBinaryTypeW(app_name: *const u16, bin_type: *mut u32) -> i32;
    pub fn GetLastError() -> u32;
}
