//! This crate provides safe methods for using Open and Save dialog boxes on Windows.
extern crate libc;
extern crate winapi;

use crate::winapi::Interface;

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::slice;

use libc::wcslen;
use winapi::{
    ctypes::c_void,
    shared::{
        minwindef::LPVOID,
        ntdef::LPWSTR,
        winerror::{HRESULT, SUCCEEDED},
    },
    um::{
        combaseapi::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL},
        objbase::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE},
        shobjidl::{IFileDialog, IFileOpenDialog, IFileSaveDialog, IShellItemArray},
        shobjidl_core::{
            CLSID_FileOpenDialog, CLSID_FileSaveDialog, IShellItem, SHCreateItemFromParsingName,
            SFGAOF, SIGDN_FILESYSPATH,
        },
        shtypes::COMDLG_FILTERSPEC,
    },
};

// Re-exports
pub use winapi::shared::windef::HWND;
pub use winapi::um::shobjidl::{
    FOS_ALLNONSTORAGEITEMS, FOS_ALLOWMULTISELECT, FOS_CREATEPROMPT, FOS_DEFAULTNOMINIMODE,
    FOS_DONTADDTORECENT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_FORCEPREVIEWPANEON,
    FOS_FORCESHOWHIDDEN, FOS_HIDEMRUPLACES, FOS_HIDEPINNEDPLACES, FOS_NOCHANGEDIR,
    FOS_NODEREFERENCELINKS, FOS_NOREADONLYRETURN, FOS_NOTESTFILECREATE, FOS_NOVALIDATE,
    FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FOS_SHAREAWARE, FOS_STRICTFILETYPES,
    FOS_SUPPORTSTREAMABLEITEMS,
};

macro_rules! com {
    ($com_expr:expr, $method_name:expr ) => {
        com(|| unsafe { $com_expr }, $method_name)
    };
}

trait NullTermUTF16 {
    fn as_null_term_utf16(&self) -> Vec<u16>;
}

impl NullTermUTF16 for str {
    fn as_null_term_utf16(&self) -> Vec<u16> {
        self.encode_utf16().chain(Some(0)).collect()
    }
}

const SFGAO_FILESYSTEM: u32 = 0x4000_0000;

type FileExtensionFilterPair<'a> = (&'a str, &'a str);

/// The parameters used when displaying a dialog box. All fields are optional and have appropriate
/// default values
#[derive(Debug)]
pub struct DialogParams<'a> {
    /// The default file extension to add to the returned file name when a file extension
    /// is not entered. Note that if this is not set no extensions will be present on returned
    /// filenames even when a specific file type filter is selected.
    pub default_extension: &'a str,
    /// The path to the default folder that the dialog will navigate to on first usage. Subsequent
    /// usages of the dialog will remember the directory of the last selected file/folder.
    pub default_folder: &'a str,
    /// The filename to pre-populate in the dialog box
    pub file_name: &'a str,
    /// The label to display to the left of the filename input box in the dialog
    pub file_name_label: &'a str,
    /// Specifies the (1-based) index of the file type that is selected by default.
    pub file_type_index: u32,
    /// The file types that are displayed in the File Type dropdown box in the dialog. The first
    /// element is the text description, i.e `"Text Files (*.txt)"` and the second element is the
    /// file extension filter pattern, with multiple entries separated by a semi-colon
    /// i.e `"*.txt;*.log"`
    pub file_types: Vec<(&'a str, &'a str)>,
    /// The path to the folder that is always selected when a dialog is opened, regardless of
    /// previous user action. This is not recommended for general use, instead `default_folder`
    /// should be used.
    pub folder: &'a str,
    /// The text label to replace the default "Open" or "Save" text on the "OK" button of the dialog
    pub ok_button_label: &'a str,
    /// A set of bit flags to apply to the dialog. Setting invalid flags will result in the dialog
    /// failing to open. Flags should be a combination of `FOS_*` constants, the documentation for
    /// which can be found [here](https://docs.microsoft.com/en-us/windows/win32/api/shobjidl_core/ne-shobjidl_core-_fileopendialogoptions)
    pub options: u32,
    /// A set of bit flags to REMOVE from the defaults provided by the system. This can be used
    /// to disable default options such as `FOS_NOVALIDATE` which is enabled by default.
    /// Flags should be a combination of `FOS_*` constants, the documentation for which can be found
    /// [here](https://docs.microsoft.com/en-us/windows/win32/api/shobjidl_core/ne-shobjidl_core-_fileopendialogoptions)
    pub un_options: u32,
    /// The HWND of the window that the dialog will be owned by. If not provided the dialog will be
    /// an independent top-level window.
    pub owner: Option<HWND>,
    /// The path to the existing file to use when opening a Save As dialog. Acts as a combination of
    /// `folder` and `file_name`, displaying the file name in the edit box, and selecting the
    /// containing folder as the initial folder in the dialog.
    pub save_as_item: &'a str,
    /// The text displayed in the title bar of the dialog box
    pub title: &'a str,
}

impl<'a> Default for DialogParams<'a> {
    fn default() -> Self {
        DialogParams {
            default_extension: "",
            default_folder: "",
            file_name: "",
            file_name_label: "",
            file_type_index: 1,
            file_types: vec![("All types (*.*)", "*.*")],
            folder: "",
            ok_button_label: "",
            options: 0,
            un_options: 0,
            owner: None,
            save_as_item: "",
            title: "",
        }
    }
}

/// The result of an Open Dialog after the user has selected one or more files (or a folder)
#[derive(Debug)]
pub struct OpenDialogResult {
    /// The first file path that the user selected. Provided as a convenience for use when
    /// `FOS_ALLOWMULTISELECT` is not enabled. If multiple files are selected this field contains
    /// the first selected file path.
    pub selected_file_path: PathBuf,
    /// The file paths that the user selected. Will only ever contain a single file path if
    /// `FOS_ALLOWMULTISELECT` is not enabled.
    pub selected_file_paths: Vec<PathBuf>,
    /// The 1-based index of the file type that was selected in the File Type dropdown
    pub selected_file_type_index: u32,
}

/// The result of a Save Dialog after the user has selected a file
#[derive(Debug)]
pub struct SaveDialogResult {
    /// The file path that the user selected
    pub selected_file_path: PathBuf,
    /// The 1-based index of the file type that was selected in the File Type dropdown
    pub selected_filter_index: u32,
}

/// Error returned when showing a dialog fails
#[derive(Debug)]
pub enum DialogError {
    /// The user cancelled the dialog
    UserCancelled,
    /// The filepath of the selected folder or item is not supported. This occurs when the selected path
    /// does not have the SFGAO_FILESYSTEM attribute. Selecting items without a regular filesystem path
    /// such as "This Computer" or a file or folder within a WPD device like a phone will cause this error.
    UnsupportedFilepath,
    /// An error occurred when showing the dialog, the HRESULT that caused the error is included.
    /// This error most commonly occurs when invalid combinations of parameters are provided
    HResultFailed {
        /// The COM method that failed
        error_method: String,
        /// The HRESULT error code
        hresult: i32,
    },
}

/// Displays an Open Dialog using the provided parameters.
///
/// # Examples
///
/// ```
/// // An entirely default Open File dialog box with no customization
/// let result = wfd::open_dialog(Default::default());
/// ```
/// ```
/// // A folder-picker Open dialog box with a custom dialog title
/// # use std::io;
/// # fn main() -> Result<(), wfd::DialogError> {
/// let params = wfd::DialogParams {
///    options: wfd::FOS_PICKFOLDERS,
///    title: "My custom open folder dialog",
///    ..Default::default()
/// };
/// let result = wfd::open_dialog(params)?;
/// let path = result.selected_file_path;
/// #    Ok(())
/// # }
/// ```
/// ```
/// // An Open dialog box with a custom dialog title and file types
/// # use std::io;
/// # fn main() -> Result<(), wfd::DialogError> {
/// let params = wfd::DialogParams {
///    title: "My custom open file dialog",
///    file_types: vec![("JPG Files", "*.jpg;*.jpeg"), ("PDF Files", "*.pdf")],
///    ..Default::default()
/// };
/// let result = wfd::open_dialog(params)?;
/// let path = result.selected_file_path;
/// #    Ok(())
/// # }
/// ```
///
/// # Errors
/// If a user cancels the dialog, the [`UserCancelled`] error is returned. The only other kinds of
/// errors that can be retured are COM [`HRESULT`] failure codes - usually as the result of invalid
/// combinations of options. These are returned in a [`HResultFailed`] error
///
/// [`UserCancelled`]: enum.FileDialogError.html#variant.UserCancelled
/// [`HResultFailed`]: enum.FileDialogError.html#variant.HResultFailed
/// [`HRESULT`]: https://en.wikipedia.org/wiki/HRESULT
pub fn open_dialog(params: DialogParams) -> Result<OpenDialogResult, DialogError> {
    // Initialize COM
    com!(
        CoInitializeEx(
            null_mut(),
            COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE,
        ),
        "CoInitializeEx"
    )?;

    // Create IFileOpenDialog instance
    let mut file_open_dialog: *mut IFileOpenDialog = null_mut();
    com!(
        CoCreateInstance(
            &CLSID_FileOpenDialog,
            null_mut(),
            CLSCTX_ALL,
            &IFileOpenDialog::uuidof(),
            &mut file_open_dialog as *mut *mut IFileOpenDialog as *mut *mut c_void,
        ),
        "CoCreateInstance - IFileOpenDialog"
    )?;
    let file_open_dialog = unsafe { &*file_open_dialog };

    // Perform non open-specific dialog configuration
    configure_file_dialog(file_open_dialog, &params)?;

    show_dialog(file_open_dialog, params.owner)?;

    // Get the item(s) that the user selected in the dialog
    // IFileOpenDialog::GetResults
    let mut shell_item_array: *mut IShellItemArray = null_mut();
    com!(
        file_open_dialog.GetResults(&mut shell_item_array),
        "IFileOpenDialog::GetResults"
    )?;

    let shell_item_array = unsafe { &*shell_item_array };

    // IShellItemArray::GetCount
    let mut item_count: u32 = 0;
    com!(
        shell_item_array.GetCount(&mut item_count),
        "IShellItemArray::GetCount"
    )?;

    let mut file_paths: Vec<PathBuf> = vec![];
    for i in 0..item_count {
        // IShellItemArray::GetItemAt
        let mut shell_item: *mut IShellItem = null_mut();
        com!(
            shell_item_array.GetItemAt(i, &mut shell_item),
            "IShellItemArray::GetItemAt"
        )?;
        let shell_item = unsafe { &*shell_item };

        // Fetch the SFGAO_FILESYSTEM attribute for the file
        let mut attribs: SFGAOF = 0;
        // IShellItem::GetAttributes
        com!(
            shell_item.GetAttributes(SFGAO_FILESYSTEM, &mut attribs),
            "IShellItem::GetAttributes"
        )?;

        // Ignore shell items that do not have the SFGAO_FILESYSTEM attribute
        // which indicates that they represent a valid path to a file or folder
        if attribs & SFGAO_FILESYSTEM == 0 {
            continue;
        }

        let file_name = get_shell_item_display_name(shell_item)?;
        file_paths.push(PathBuf::from(file_name));

        // Free non-owned allocation
        unsafe { shell_item.Release() };
    }

    // IFileDialog::GetFileTypeIndex
    let selected_filter_index = get_file_type_index(file_open_dialog)?;

    // Un-initialize COM
    unsafe {
        CoUninitialize();
    }

    file_paths
        .first()
        .cloned()
        .map(|x| OpenDialogResult {
            selected_file_path: x,
            selected_file_paths: file_paths,
            selected_file_type_index: selected_filter_index,
        })
        .ok_or(DialogError::UnsupportedFilepath)
}

/// Displays a Save Dialog using the provided parameters.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), wfd::DialogError> {
/// // An entirely default Save File dialog box with no customization
/// let result = wfd::save_dialog(Default::default())?;
/// #    Ok(())
/// # }
/// ```
/// ```
/// # fn main() -> Result<(), wfd::DialogError> {
/// // A Save File dialog box with a custom dialog title and file types///
/// let params = wfd::DialogParams {
///    title: "My custom save file dialog",
///    file_types: vec![("JPG Files", "*.jpg;*.jpeg"), ("PDF Files", "*.pdf")],
///    ..Default::default()
/// };
/// let result = wfd::save_dialog(params)?;
/// #    Ok(())
/// # }
/// ```
///
/// # Errors
/// If a user cancels the dialog, the [`UserCancelled`] error is returned. The only other kinds of
/// errors that can be retured are COM [`HRESULT`] failure codes - usually as the result of invalid
/// combinations of options. These are returned in a [`HResultFailed`] error
///
/// [`UserCancelled`]: enum.FileDialogError.html#variant.UserCancelled
/// [`HResultFailed`]: enum.FileDialogError.html#variant.HResultFailed
/// [`HRESULT`]: https://en.wikipedia.org/wiki/HRESULT
pub fn save_dialog(params: DialogParams) -> Result<SaveDialogResult, DialogError> {
    // Initialize COM
    com!(
        CoInitializeEx(
            null_mut(),
            COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE,
        ),
        "CoInitializeEx"
    )?;

    // Create IFileSaveDialog instance
    let mut file_save_dialog: *mut IFileSaveDialog;
    file_save_dialog = null_mut();
    com!(
        CoCreateInstance(
            &CLSID_FileSaveDialog,
            null_mut(),
            CLSCTX_ALL,
            &IFileSaveDialog::uuidof(),
            &mut file_save_dialog as *mut *mut IFileSaveDialog as *mut *mut c_void,
        ),
        "CoCreateInstance - FileSaveDialog"
    )?;
    let file_save_dialog = unsafe { &*file_save_dialog };

    // IFileDialog::SetSaveAsItem
    if !params.save_as_item.is_empty() {
        let mut item: *mut IShellItem = null_mut();
        let path = params.save_as_item.as_null_term_utf16();
        com!(
            SHCreateItemFromParsingName(
                path.as_ptr(),
                null_mut(),
                &IShellItem::uuidof(),
                &mut item as *mut *mut IShellItem as *mut *mut c_void
            ),
            "SHCreateItemFromParsingName"
        )?;
        com!(
            file_save_dialog.SetSaveAsItem(item),
            "IFileDialog::SetSaveAsItem"
        )?;
        unsafe {
            let item = &*item;
            item.Release();
        }
    }

    // Perform non save-specific dialog configuration
    configure_file_dialog(file_save_dialog, &params)?;

    show_dialog(file_save_dialog, params.owner)?;

    // IFileDialog::GetResult
    let mut shell_item: *mut IShellItem = null_mut();
    com!(
        file_save_dialog.GetResult(&mut shell_item),
        "IFileDialog::GetResult"
    )?;
    let shell_item = unsafe { &*shell_item };
    let file_name = get_shell_item_display_name(shell_item)?;
    unsafe { shell_item.Release() };

    // IFileDialog::GetFileTypeIndex
    let selected_filter_index = get_file_type_index(file_save_dialog)?;

    // Un-initialize COM
    unsafe {
        CoUninitialize();
    }

    let result = SaveDialogResult {
        selected_filter_index,
        selected_file_path: PathBuf::from(file_name),
    };

    Ok(result)
}

#[allow(overflowing_literals)]
#[allow(unused_comparisons)]
fn show_dialog(file_dialog: &IFileDialog, owner: Option<HWND>) -> Result<(), DialogError> {
    let owner_hwnd = owner.unwrap_or(null_mut());

    // IModalWindow::Show
    let result = com!(file_dialog.Show(owner_hwnd), "IModalWindow::Show");

    match result {
        Ok(_) => Ok(()),
        Err(e) => match e {
            DialogError::HResultFailed { hresult, .. } => {
                if hresult == 0x8007_04C7 {
                    Err(DialogError::UserCancelled)
                } else {
                    Err(e)
                }
            }
            _ => Err(e),
        },
    }
}

fn configure_file_dialog(
    file_dialog: &IFileDialog,
    params: &DialogParams,
) -> Result<(), DialogError> {
    // IFileDialog::SetDefaultExtension
    if !params.default_extension.is_empty() {
        let default_extension = params.default_extension.as_null_term_utf16();
        com!(
            file_dialog.SetDefaultExtension(default_extension.as_ptr()),
            "IFileDialog::SetDefaultExtension"
        )?;
    }

    // IFileDialog::SetDefaultFolder
    if !params.default_folder.is_empty() {
        let mut default_folder: *mut IShellItem = null_mut();
        let path = params.default_folder.as_null_term_utf16();
        com!(
            SHCreateItemFromParsingName(
                path.as_ptr(),
                null_mut(),
                &IShellItem::uuidof(),
                &mut default_folder as *mut *mut IShellItem as *mut *mut c_void
            ),
            "SHCreateItemFromParsingName"
        )?;
        com!(
            file_dialog.SetDefaultFolder(default_folder),
            "IFileDialog::SetDefaultFolder"
        )?;
        unsafe {
            let default_folder = &*default_folder;
            default_folder.Release();
        }
    }

    // IFileDialog::SetFolder
    if !params.folder.is_empty() {
        let mut folder: *mut IShellItem = null_mut();
        let path = params.folder.as_null_term_utf16();
        com!(
            SHCreateItemFromParsingName(
                path.as_ptr(),
                null_mut(),
                &IShellItem::uuidof(),
                &mut folder as *mut *mut IShellItem as *mut *mut c_void
            ),
            "SHCreateItemFromParsingName"
        )?;
        com!(file_dialog.SetFolder(folder), "IFileDialog::SetFolder")?;
        unsafe {
            let folder = &*folder;
            folder.Release();
        }
    }

    // IFileDialog::SetFileName
    if !params.file_name.is_empty() {
        let initial_file_name = params.file_name.as_null_term_utf16();
        com!(
            file_dialog.SetFileName(initial_file_name.as_ptr()),
            "IFileDialog::SetFileName"
        )?;
    }

    // IFileDialog::SetFileNameLabel
    if !params.file_name_label.is_empty() {
        let file_name_label = params.file_name_label.as_null_term_utf16();
        com!(
            file_dialog.SetFileNameLabel(file_name_label.as_ptr()),
            "IFileDialog::SetFileNameLabel"
        )?;
    }

    if !params.file_types.is_empty() {
        add_filters(file_dialog, &params.file_types)?;
    }

    // IFileDialog::SetFileTypeIndex
    if !params.file_types.is_empty() && params.file_type_index > 0 {
        com!(
            file_dialog.SetFileTypeIndex(params.file_type_index),
            "IFileDialog::SetFileTypeIndex"
        )?;
    }

    // IFileDialog::SetOkButtonLabel
    if !params.ok_button_label.is_empty() {
        let ok_button_label = params.ok_button_label.as_null_term_utf16();
        com!(
            file_dialog.SetOkButtonLabel(ok_button_label.as_ptr()),
            "IFileDialog::SetOkButtonLabel"
        )?;
    }

    if params.options > 0 || params.un_options > 0 {
        // IFileDialog::GetOptions
        let mut existing_options: u32 = 0;
        com!(
            file_dialog.GetOptions(&mut existing_options),
            "IFileDialog::GetOptions"
        )?;

        existing_options &= !params.un_options;
        existing_options |= params.options;

        // IFileDialog::SetOptions
        com!(
            file_dialog.SetOptions(existing_options),
            "IFileDialog::SetOptions"
        )?;
    }

    // IFileDialog::SetTitle
    if !params.title.is_empty() {
        let title = params.title.as_null_term_utf16();
        com!(
            file_dialog.SetTitle(title.as_ptr()),
            "IFileDialog::SetTitle"
        )?;
    }

    Ok(())
}

fn add_filters(
    dialog: &IFileDialog,
    filters: &[FileExtensionFilterPair],
) -> Result<(), DialogError> {
    // Create a vec holding the UTF-16 string pairs for the filter - we need
    // to have these in a vec since we need to be able to pass a pointer to them
    // in the COMDLG_FILTERSPEC structs passed to SetFileTypes.
    let temp_filters = filters
        .iter()
        .map(|filter| {
            let name = filter.0.as_null_term_utf16();
            let pattern = filter.1.as_null_term_utf16();
            (name, pattern)
        })
        .collect::<Vec<(Vec<u16>, Vec<u16>)>>();

    let filter_specs = temp_filters
        .iter()
        .map(|x| COMDLG_FILTERSPEC {
            pszName: x.0.as_ptr(),
            pszSpec: x.1.as_ptr(),
        })
        .collect::<Vec<COMDLG_FILTERSPEC>>();

    // IFileDialog::SetFileTypes
    com!(
        dialog.SetFileTypes(filter_specs.len() as u32, filter_specs.as_ptr()),
        "IFileDialog::SetFileTypes"
    )?;

    Ok(())
}

fn get_file_type_index(file_dialog: &IFileDialog) -> Result<u32, DialogError> {
    // IFileDialog::GetFileTypeIndex
    let mut selected_filter_index: u32 = 0;
    com!(
        file_dialog.GetFileTypeIndex(&mut selected_filter_index),
        "IFileDialog::GetFileTypeIndex"
    )?;
    Ok(selected_filter_index)
}

fn get_shell_item_display_name(shell_item: &IShellItem) -> Result<OsString, DialogError> {
    let mut display_name: LPWSTR = null_mut();
    // IShellItem::GetDisplayName
    com!(
        shell_item.GetDisplayName(SIGDN_FILESYSPATH, &mut display_name),
        "IShellItem::GetDisplayName"
    )?;
    let slice = unsafe { slice::from_raw_parts(display_name, wcslen(display_name)) };
    let result = OsString::from_wide(slice);

    // Free non-owned allocation
    unsafe { CoTaskMemFree(display_name as LPVOID) };

    Ok(result)
}

// This wrapper method makes working with COM methods much simpler by
// returning Err if the HRESULT for a call does not return success.
fn com<F>(mut f: F, method: &str) -> Result<(), DialogError>
where
    F: FnMut() -> HRESULT,
{
    let hresult = f();
    if !SUCCEEDED(hresult) {
        Err(DialogError::HResultFailed {
            hresult,
            error_method: method.to_string(),
        })
    } else {
        Ok(())
    }
}
