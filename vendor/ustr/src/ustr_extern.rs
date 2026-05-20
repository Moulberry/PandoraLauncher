use ustr::Ustr;

#[no_mangle]
pub extern "C" fn ustr(chars: *const std::os::raw::c_char) -> Ustr {
    let cs = unsafe { std::ffi::CStr::from_ptr(chars).to_string_lossy() };
    Ustr::from(&cs)
}

#[no_mangle]
pub extern "C" fn ustr_len(u: Ustr) -> usize {
    u.len()
}

#[no_mangle]
pub extern "C" fn ustr_hash(u: Ustr) -> u64 {
    u.precomputed_hash()
}
