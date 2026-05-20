use std::path::Path;

use wfd::{
    DialogError, DialogParams, FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FOS_NOREADONLYRETURN,
    FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FOS_STRICTFILETYPES, OpenDialogResult,
    SaveDialogResult,
};

use crate::dialog::{
    DialogImpl, FileFilter, OpenMultipleFile, OpenSingleDir, OpenSingleFile, SaveSingleFile,
};
use crate::ffi::UnsafeWindowHandle;
use crate::utils::resolve_tilde;
use crate::{Error, Result};

impl OpenSingleFile {
    fn create(&self) -> OpenDialogParams<'_> {
        OpenDialogParams {
            title: &self.title,
            filename: self.filename.as_deref(),
            location: self.location.as_deref(),
            filters: &self.filters.items,
            owner: self.owner.clone(),
            multiple: false,
            dir: false,
        }
    }
}

impl DialogImpl for OpenSingleFile {
    fn show(self) -> Result<Self::Output> {
        super::process_init();
        let result = open_dialog(self.create())?;
        Ok(result.map(|x| x.selected_file_path))
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        self.show()
    }
}

impl OpenMultipleFile {
    fn create(&self) -> OpenDialogParams<'_> {
        OpenDialogParams {
            title: &self.title,
            filename: self.filename.as_deref(),
            location: self.location.as_deref(),
            filters: &self.filters.items,
            owner: self.owner.clone(),
            multiple: true,
            dir: false,
        }
    }
}

impl DialogImpl for OpenMultipleFile {
    fn show(self) -> Result<Self::Output> {
        super::process_init();

        let result = open_dialog(self.create())?;
        match result {
            Some(t) => Ok(t.selected_file_paths),
            None => Ok(vec![]),
        }
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        self.show()
    }
}

impl OpenSingleDir {
    fn create(&self) -> OpenDialogParams<'_> {
        OpenDialogParams {
            title: &self.title,
            filename: self.filename.as_deref(),
            location: self.location.as_deref(),
            filters: &[],
            owner: self.owner.clone(),
            multiple: false,
            dir: true,
        }
    }
}

impl DialogImpl for OpenSingleDir {
    fn show(self) -> Result<Self::Output> {
        super::process_init();

        let result = open_dialog(self.create())?;
        Ok(result.map(|x| x.selected_file_path))
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        self.show()
    }
}

impl SaveSingleFile {
    fn create(&self) -> SaveDialogParams<'_> {
        SaveDialogParams {
            title: &self.title,
            filename: self.filename.as_deref(),
            location: self.location.as_deref(),
            filters: &self.filters.items,
            owner: self.owner.clone(),
        }
    }
}

impl DialogImpl for SaveSingleFile {
    fn show(self) -> Result<Self::Output> {
        super::process_init();

        let result = save_dialog(self.create())?;
        Ok(result.map(|x| x.selected_file_path))
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        self.show()
    }
}

pub struct OpenDialogParams<'a> {
    title: &'a str,
    filename: Option<&'a str>,
    location: Option<&'a Path>,
    filters: &'a [FileFilter],
    owner: UnsafeWindowHandle,
    multiple: bool,
    dir: bool,
}

fn open_dialog(params: OpenDialogParams) -> Result<Option<OpenDialogResult>> {
    let folder = params.location.and_then(resolve_tilde);
    let folder = folder.as_deref().and_then(Path::to_str).unwrap_or("");

    let file_types: Vec<_> = get_dialog_file_types(params.filters);
    let file_types = file_types.iter().map(|t| (t.0, &*t.1)).collect();

    let file_name = params.filename.unwrap_or("");

    let mut options = FOS_PATHMUSTEXIST | FOS_FILEMUSTEXIST | FOS_STRICTFILETYPES;
    if params.multiple {
        options |= FOS_ALLOWMULTISELECT;
    }
    if params.dir {
        options |= FOS_PICKFOLDERS;
    }

    let owner = unsafe { params.owner.as_win32() };

    let params = DialogParams {
        folder,
        file_types,
        file_name,
        options,
        owner,
        title: params.title,
        ..Default::default()
    };

    let result = wfd::open_dialog(params);

    convert_result(result)
}

pub struct SaveDialogParams<'a> {
    title: &'a str,
    filename: Option<&'a str>,
    location: Option<&'a Path>,
    filters: &'a [FileFilter],
    owner: UnsafeWindowHandle,
}

fn save_dialog(params: SaveDialogParams) -> Result<Option<SaveDialogResult>> {
    let folder = params.location.and_then(resolve_tilde);
    let folder = folder.as_deref().and_then(Path::to_str).unwrap_or("");

    let file_types: Vec<_> = get_dialog_file_types(params.filters);
    let file_types = file_types.iter().map(|t| (t.0, &*t.1)).collect();

    let file_name = params.filename.unwrap_or("");

    let default_extension = match params.filters.first() {
        Some(first) => &first.extensions[0],
        _ => "",
    };

    let options =
        FOS_OVERWRITEPROMPT | FOS_PATHMUSTEXIST | FOS_NOREADONLYRETURN | FOS_STRICTFILETYPES;

    let owner = unsafe { params.owner.as_win32() };

    let params = DialogParams {
        folder,
        file_types,
        file_name,
        default_extension,
        options,
        owner,
        title: params.title,
        ..Default::default()
    };

    let result = wfd::save_dialog(params);

    convert_result(result)
}

fn get_dialog_file_types(filters: &[FileFilter]) -> Vec<(&str, String)> {
    filters
        .iter()
        .map(|filter| {
            let name = &*filter.name;
            let types = filter.format("{types}", "*{ext}", ";");
            (name, types)
        })
        .collect()
}

fn convert_result<T>(result: std::result::Result<T, DialogError>) -> Result<Option<T>> {
    match result {
        Ok(t) => Ok(Some(t)),
        Err(e) => match e {
            DialogError::UserCancelled => Ok(None),
            DialogError::HResultFailed { error_method, .. } => Err(Error::Other(error_method)),
            DialogError::UnsupportedFilepath => {
                Err(Error::Other("Unsupported filepath".to_string()))
            }
        },
    }
}
