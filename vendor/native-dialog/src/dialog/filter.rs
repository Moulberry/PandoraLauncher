use formatx::formatx;

/// A list of file filters.
#[derive(Debug, Clone, Default)]
pub struct FileFiltersBag {
    pub items: Vec<FileFilter>,
}

impl FileFiltersBag {
    pub fn add<T, U, V>(&mut self, name: T, extensions: V)
    where
        T: ToString,
        U: ToString,
        V: AsRef<[U]>,
    {
        if let Some(filter) = FileFilter::new(name, extensions) {
            self.items.push(filter);
        }
    }

    #[cfg(unix)]
    pub fn accepts(&self, path: impl AsRef<std::path::Path>) -> bool {
        if self.items.is_empty() {
            return true;
        }

        for filter in &self.items {
            if filter.accepts(&path) {
                return true;
            }
        }

        false
    }
}

/// A file type filter, consisting of its name and a set of file extensions.
#[derive(Debug, Clone)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

impl FileFilter {
    pub fn new<T, U, V>(name: T, extensions: V) -> Option<Self>
    where
        T: ToString,
        U: ToString,
        V: AsRef<[U]>,
    {
        let extensions = extensions.as_ref();
        if extensions.is_empty() {
            return None;
        }

        Some(FileFilter {
            name: name.to_string(),
            extensions: extensions
                .iter()
                .map(ToString::to_string)
                .filter(|ext| !ext.is_empty())
                .map(|mut ext| {
                    if !ext.starts_with('.') {
                        ext.insert(0, '.');
                    }
                    ext
                })
                .collect(),
        })
    }

    #[cfg(unix)]
    pub fn accepts(&self, path: impl AsRef<std::path::Path>) -> bool {
        use std::os::unix::ffi::OsStrExt;

        if let Some(name) = path.as_ref().file_name() {
            for accepting in &self.extensions {
                if name.as_bytes().ends_with(accepting.as_bytes()) {
                    return true;
                }
            }
        }

        false
    }

    pub fn format(&self, fmt_line: &str, fmt_type: &str, delimeter: &str) -> String {
        let types: Vec<String> = self
            .extensions
            .iter()
            .map(|ext| formatx!(fmt_type, ext = ext).unwrap())
            .collect();

        formatx!(fmt_line, name = &self.name, types = types.join(delimeter)).unwrap()
    }
}
