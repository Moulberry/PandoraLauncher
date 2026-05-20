use std::path::{Component, Path, PathBuf};

use dirs::home_dir;

pub fn resolve_tilde<P: AsRef<Path> + ?Sized>(path: &P) -> Option<PathBuf> {
    let mut result = PathBuf::new();

    let mut components = path.as_ref().components();
    match components.next() {
        Some(Component::Normal(c)) if c == "~" => result.push(home_dir()?),
        Some(c) => result.push(c),
        None => {}
    };
    result.extend(components);

    Some(result)
}
