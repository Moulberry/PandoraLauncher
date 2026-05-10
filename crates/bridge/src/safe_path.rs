use std::{path::{Path, PathBuf}, sync::Arc};

use relative_path::RelativePath;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct SafePath(Arc<RelativePath>);

impl SafePath {
    pub fn from_relative_path(relative: &RelativePath) -> Option<SafePath> {
        for component in relative.components() {
            match component {
                relative_path::Component::CurDir => {},
                relative_path::Component::ParentDir => {
                    return None;
                },
                relative_path::Component::Normal(component) => {
                    let sanitized = sanitize_filename::is_sanitized_with_options(component, sanitize_filename::OptionsForCheck {
                        windows: true,
                        truncate: false
                    });
                    if !sanitized {
                        return None;
                    }
                },
            }
        }
        Some(Self(Arc::from(relative.normalize())))
    }

    pub fn from_std_path(path: &Path) -> Option<SafePath> {
        Self::from_relative_path(RelativePath::from_path(path).ok()?)
    }

    pub fn new(path: &str) -> Option<SafePath> {
        let trimmed = path.trim_ascii();
        if trimmed.is_empty() {
            return None;
        }
        Self::from_relative_path(RelativePath::new(trimmed))
    }

    pub fn join(&self, other: &SafePath) -> SafePath {
        Self::from_relative_path(&self.0.join(&other.0)).unwrap()
    }

    pub fn to_path(&self, base: &Path) -> PathBuf {
        self.0.to_path(base)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn strip_extension(&self, extension: &str) -> Option<Self> {
        debug_assert!(!extension.contains('.'));
        if self.0.extension() != Some(extension) {
            return None;
        }
        Some(Self(Arc::from(self.0.with_extension(""))))
    }

    pub fn strip_prefix(&self, prefix: &str) -> Option<Self> {
        Some(Self(Arc::from(self.0.strip_prefix(prefix).ok()?)))
    }

    pub fn starts_with<P: AsRef<RelativePath>>(&self, base: P) -> bool {
        self.0.starts_with(base)
    }

    pub fn extension(&self) -> Option<&str> {
        self.0.extension()
    }

    pub fn file_name(&self) -> Option<&str> {
        self.0.file_name()
    }
}

impl AsRef<RelativePath> for SafePath {
    fn as_ref(&self) -> &RelativePath {
        &self.0
    }
}
