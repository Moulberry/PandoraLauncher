use std::path::{Path, PathBuf};

use raw_window_handle::HasWindowHandle;

use crate::dialog::{
    FileFilter, FileFiltersBag, OpenMultipleFile, OpenSingleDir, OpenSingleFile, SaveSingleFile,
};
use crate::ffi::UnsafeWindowHandle;

/// Builder for file dialogs.
#[derive(Debug, Clone, Default)]
pub struct FileDialogBuilder {
    pub filename: Option<String>,
    pub location: Option<PathBuf>,
    pub filters: FileFiltersBag,
    pub owner: UnsafeWindowHandle,
    pub title: Option<String>,
}

impl FileDialogBuilder {
    /// Sets the window title for the dialog.
    pub fn set_title(mut self, title: impl ToString) -> Self {
        self.title = Some(title.to_string());
        self
    }

    /// Sets the default value of the filename text field in the dialog. For open dialogs of macOS
    /// and zenity, this is a no-op because there's no such text field on the dialog.
    pub fn set_filename(mut self, filename: impl ToString) -> Self {
        self.filename = Some(filename.to_string());
        self
    }

    /// Resets the default value of the filename field in the dialog.
    pub fn reset_filename(mut self) -> Self {
        self.filename = None;
        self
    }

    /// Sets the default directory that the dialog shows at open.
    pub fn set_location<P: AsRef<Path> + ?Sized>(mut self, path: &P) -> Self {
        self.location = Some(path.as_ref().to_path_buf());
        self
    }

    /// Resets the default directory that the dialog shows at open.
    /// If a location is not set, the dialog will probably go to the current working directory.
    pub fn reset_location(mut self) -> Self {
        self.location = None;
        self
    }

    /// Adds a file type filter. The filter must contains at least one extension, otherwise this
    /// method will be a no-op. For dialogs that open directories, this is also a no-op.
    pub fn add_filter<T, U, V>(mut self, name: T, extensions: V) -> Self
    where
        T: ToString,
        U: ToString,
        V: AsRef<[U]>,
    {
        self.filters.add(name, extensions);
        self
    }

    /// Adds a bunch of file type filters.
    pub fn add_filters<I>(mut self, filters: I) -> Self
    where
        I: IntoIterator<Item = (String, Vec<String>)>,
    {
        let filters = filters.into_iter().flat_map(|(x, y)| FileFilter::new(x, y));
        self.filters.items.extend(filters);
        self
    }

    /// Removes all file type filters.
    pub fn reset_filters(mut self) -> Self {
        self.filters.items.clear();
        self
    }

    /// Sets the owner of the dialog.
    pub fn set_owner<W: HasWindowHandle>(mut self, window: &W) -> Self {
        self.owner = UnsafeWindowHandle::new(window);
        self
    }

    /// Resets the owner of the dialog to nothing.
    pub fn reset_owner(mut self) -> Self {
        self.owner = UnsafeWindowHandle::default();
        self
    }

    /// Builds a dialog that let users to open one file.
    pub fn open_single_file(self) -> OpenSingleFile {
        OpenSingleFile {
            filename: self.filename,
            location: self.location,
            filters: self.filters,
            owner: self.owner,
            title: self.title.unwrap_or("Open a File".to_string()),
        }
    }

    /// Builds a dialog that let users to open multiple files.
    pub fn open_multiple_file(self) -> OpenMultipleFile {
        OpenMultipleFile {
            filename: self.filename,
            location: self.location,
            filters: self.filters,
            owner: self.owner,
            title: self.title.unwrap_or("Open Files".to_string()),
        }
    }

    /// Builds a dialog that let users to open one directory.
    pub fn open_single_dir(self) -> OpenSingleDir {
        OpenSingleDir {
            filename: self.filename,
            location: self.location,
            owner: self.owner,
            title: self.title.unwrap_or("Open a Folder".to_string()),
        }
    }

    /// Builds a dialog that let users to save one file.
    pub fn save_single_file(self) -> SaveSingleFile {
        SaveSingleFile {
            filename: self.filename,
            location: self.location,
            filters: self.filters,
            owner: self.owner,
            title: self.title.unwrap_or("Save As".to_string()),
        }
    }
}
