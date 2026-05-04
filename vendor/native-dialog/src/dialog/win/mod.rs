mod file;
mod message;

fn process_init() {
    use std::sync::Once;

    static INIT: Once = Once::new();

    #[allow(unused_unsafe)]
    INIT.call_once(|| unsafe {
        #[cfg(feature = "windows_dpi_awareness")]
        winapi::um::winuser::SetProcessDPIAware();
    });
}

#[cfg(not(feature = "windows_visual_styles"))]
#[inline(always)]
fn with_visual_styles<T>(f: impl Fn() -> T) -> T {
    f()
}

#[cfg(feature = "windows_visual_styles")]
#[inline(always)]
fn with_visual_styles<T>(f: impl Fn() -> T) -> T {
    use crate::ffi::win::ActivationContext;

    match ActivationContext::get() {
        Some(ctx) => ctx.with(f),
        None => f(),
    }
}
