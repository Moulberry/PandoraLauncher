use std::path::PathBuf;

use super::{Dialog, FileFiltersBag};
use crate::ffi::UnsafeWindowHandle;

#[derive(Debug)]
pub struct OpenSingleFile {
    pub filename: Option<String>,
    pub location: Option<PathBuf>,
    pub title: String,
    pub filters: FileFiltersBag,
    pub owner: UnsafeWindowHandle,
}

impl Dialog for OpenSingleFile {
    type Output = Option<PathBuf>;
}

impl OpenSingleFile {
    super::dialog_delegate!();
}

#[derive(Debug)]
pub struct OpenMultipleFile {
    pub filename: Option<String>,
    pub location: Option<PathBuf>,
    pub title: String,
    pub filters: FileFiltersBag,
    pub owner: UnsafeWindowHandle,
}

impl Dialog for OpenMultipleFile {
    type Output = Vec<PathBuf>;
}

impl OpenMultipleFile {
    super::dialog_delegate!();
}

#[derive(Debug)]
pub struct OpenSingleDir {
    pub filename: Option<String>,
    pub location: Option<PathBuf>,
    pub title: String,
    pub owner: UnsafeWindowHandle,
}

impl Dialog for OpenSingleDir {
    type Output = Option<PathBuf>;
}

impl OpenSingleDir {
    super::dialog_delegate!();
}

#[derive(Debug)]
pub struct SaveSingleFile {
    pub filename: Option<String>,
    pub location: Option<PathBuf>,
    pub title: String,
    pub filters: FileFiltersBag,
    pub owner: UnsafeWindowHandle,
}

impl Dialog for SaveSingleFile {
    type Output = Option<PathBuf>;
}

impl SaveSingleFile {
    super::dialog_delegate!();
}
