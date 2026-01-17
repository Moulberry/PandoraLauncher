#[cfg(windows)]
fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let icon_path = std::path::Path::new(&manifest_dir)
        .join("../../assets/icons/logo.ico");

    let mut res = winres::WindowsResource::new();
    res.set_icon(icon_path.to_str().unwrap());
    res.compile().unwrap();
}

#[cfg(not(windows))]
fn main() {}