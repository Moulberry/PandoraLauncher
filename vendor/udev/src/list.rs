use std::ffi::OsStr;
use std::marker::PhantomData;

use ffi;
use util;

/// Rust wrapper for the `udev_list_entry` struct, which provides sequential
/// access to an associative list of string names and values.
///
/// Each `List<T>` is parametrized on the Rust wrapper type that owns its
/// underlying data. For example, `List<Hwdb>` indicates a list owned by
/// some open handle to the `udev` hardware database.
pub struct List<'a, T: 'a, E: 'a> {
    pub(crate) entry: *mut ffi::udev_list_entry,
    pub(crate) phantom: PhantomData<&'a (T, E)>,
}

#[cfg(feature = "send")]
unsafe impl<T, E> Send for List<'_, T, E> {}
#[cfg(feature = "sync")]
unsafe impl<T, E> Sync for List<'_, T, E> {}

pub type EntryList<'a, T> = List<'a, T, Entry<'a>>;

impl<'a, T> Iterator for EntryList<'a, T> {
    type Item = Entry<'a>;

    fn next(&mut self) -> Option<Entry<'a>> {
        if self.entry.is_null() {
            None
        } else {
            let name =
                unsafe { util::ptr_to_os_str_unchecked(ffi::udev_list_entry_get_name(self.entry)) };
            let value = unsafe { util::ptr_to_os_str(ffi::udev_list_entry_get_value(self.entry)) };

            self.entry = unsafe { ffi::udev_list_entry_get_next(self.entry) };

            Some(Entry { name, value })
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

/// Rust wrapper for each entry in `List`, each of which contains a name and a value.
pub struct Entry<'a> {
    pub(crate) name: &'a OsStr,
    pub(crate) value: Option<&'a OsStr>,
}

impl<'a> Entry<'a> {
    /// Returns the entry name.
    pub fn name(&self) -> &OsStr {
        self.name
    }

    /// Returns the entry value.
    pub fn value(&self) -> &OsStr {
        self.value.unwrap_or_else(|| OsStr::new(""))
    }
}
