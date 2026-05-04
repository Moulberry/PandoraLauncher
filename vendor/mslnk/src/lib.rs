//! # Shell Link writer for Rust.
//! Works on Windows,theoretically support Windows 7,8,10.
//!
//! ## Example
//! A simple example appears as follows:
//! ```rust
//! use mslnk::ShellLink;
//! // ...
//! let target = r"C:\Users\Admin\Desktop\qq aa\qiuqiu.exe";
//! let lnk = r"C:\Users\Admin\Desktop\qq.lnk";
//! let sl = ShellLink::new(target).unwrap();
//! sl.create_lnk(lnk).unwrap();
//! ``
use log::debug;
use std::fs::File;
use std::io::{prelude::*, BufWriter};
use std::path::Path;

mod header;
pub use header::{
    FileAttributeFlags, HotkeyFlags, HotkeyKey, HotkeyModifiers, LinkFlags, ShellLinkHeader,
    ShowCommand,
};

mod linktarget;
pub use linktarget::LinkTargetIdList;

mod linkinfo;
pub use linkinfo::LinkInfo;

mod stringdata;

mod extradata;
pub use extradata::ExtraData;

// export error trait
mod error;
pub use error::MSLinkError;
/// A shell link
#[derive(Clone, Debug)]
pub struct ShellLink {
    shell_link_header: header::ShellLinkHeader,
    linktarget_id_list: Option<linktarget::LinkTargetIdList>,
    link_info: Option<linkinfo::LinkInfo>,
    name_string: Option<String>,
    relative_path: Option<String>,
    working_dir: Option<String>,
    command_line_arguments: Option<String>,
    icon_location: Option<String>,
    extra_data: Vec<extradata::ExtraData>,
}

impl Default for ShellLink {
    /// Create a new ShellLink, left blank for manual configuration.
    /// For those who are not familar with the Shell Link specification, I
    /// suggest you look at the [`new_simple`](#method.new_simple) method.
    fn default() -> Self {
        Self {
            shell_link_header: header::ShellLinkHeader::default(),
            linktarget_id_list: Some(linktarget::LinkTargetIdList::default()),
            link_info: None,
            name_string: None,
            relative_path: None,
            working_dir: None,
            command_line_arguments: None,
            icon_location: None,
            extra_data: vec![],
        }
    }
}

impl ShellLink {
    /// Create a new ShellLink pointing to a location, with otherwise default settings.
    pub fn new<P: AsRef<Path>>(target: P) -> Result<Self, MSLinkError> {
        use std::fs;

        let meta = fs::metadata(&target)?;
        let canonical = fs::canonicalize(&target)?.into_boxed_path();
        let working_dir_path = target
            .as_ref()
            .parent()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let mut sl = Self::default();

        let mut flags = LinkFlags::IS_UNICODE;
        sl.header_mut().set_link_flags(flags);
        if meta.is_dir() {
            sl.header_mut()
                .set_file_attributes(FileAttributeFlags::FILE_ATTRIBUTE_DIRECTORY);
        } else {
            flags |= LinkFlags::HAS_WORKING_DIR
                | LinkFlags::HAS_RELATIVE_PATH
                | LinkFlags::HAS_LINK_TARGET_ID_LIST;
            sl.header_mut().set_link_flags(flags);
            sl.set_relative_path(Some(format!(
                "./{}",
                canonical.file_name().unwrap().to_str().unwrap()
            )));
            sl.set_working_dir(Some(working_dir_path));
            sl.header_mut().set_file_size(meta.len() as u32);
            // set link_target_idlist
            sl.linktarget_mut().unwrap().set_linktarget(&target);
        }

        Ok(sl)
    }

    /// Save a shell link.
    ///
    /// Note that this doesn't save any [`ExtraData`](struct.ExtraData.html) entries.
    pub fn create_lnk<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), MSLinkError> {
        let mut w = BufWriter::new(File::create(path)?);

        debug!("Writing header...");
        let header_data: [u8; 0x4c] = self.shell_link_header.into();
        w.write_all(&header_data)?;

        let link_flags = *self.header().link_flags();

        if link_flags.contains(LinkFlags::HAS_LINK_TARGET_ID_LIST) {
            let mut data: Vec<u8> = self.linktarget_id_list.clone().unwrap().into();
            w.write_all(&mut data)?;
        }

        if link_flags.contains(LinkFlags::HAS_LINK_INFO) {
            debug!("LinkInfo is marked as present. Writing.");
            let mut data: Vec<u8> = self.link_info.clone().unwrap().into();
            w.write_all(&mut data)?;
        }

        if link_flags.contains(LinkFlags::HAS_NAME) {
            debug!("Name is marked as present. Writing.");
            w.write_all(&stringdata::to_data(
                self.name_string.as_ref().unwrap(),
                link_flags,
            ))?;
        }

        if link_flags.contains(LinkFlags::HAS_RELATIVE_PATH) {
            debug!("Relative path is marked as present. Writing.");
            w.write_all(&stringdata::to_data(
                self.relative_path.as_ref().unwrap(),
                link_flags,
            ))?;
        }

        if link_flags.contains(LinkFlags::HAS_WORKING_DIR) {
            debug!("Working dir is marked as present. Writing.");
            w.write_all(&stringdata::to_data(
                self.working_dir.as_ref().unwrap(),
                link_flags,
            ))?;
        }

        if link_flags.contains(LinkFlags::HAS_ARGUMENTS) {
            debug!("Arguments are marked as present. Writing.");
            w.write_all(&stringdata::to_data(
                self.command_line_arguments.as_ref().unwrap(),
                link_flags,
            ))?;
        }

        if link_flags.contains(LinkFlags::HAS_ICON_LOCATION) {
            debug!("Icon Location is marked as present. Writing.");
            w.write_all(&stringdata::to_data(
                self.icon_location.as_ref().unwrap(),
                link_flags,
            ))?;
        }

        Ok(())
    }

    ///  Get a mutable instance of the shell link's target
    pub fn linktarget_mut(&mut self) -> Option<&mut LinkTargetIdList> {
        self.linktarget_id_list.as_mut()
    }
    /// Get the header of the shell link
    pub fn header(&self) -> &ShellLinkHeader {
        &self.shell_link_header
    }

    /// Get a mutable instance of the shell link's header
    pub fn header_mut(&mut self) -> &mut ShellLinkHeader {
        &mut self.shell_link_header
    }

    /// Get the shell link's name, if set
    pub fn name(&self) -> &Option<String> {
        &self.name_string
    }

    /// Set the shell link's name
    pub fn set_name(&mut self, name: Option<String>) {
        self.header_mut()
            .update_link_flags(LinkFlags::HAS_NAME, name.is_some());
        self.name_string = name;
    }

    /// Get the shell link's relative path, if set
    pub fn relative_path(&self) -> &Option<String> {
        &self.relative_path
    }

    /// Set the shell link's relative path
    pub fn set_relative_path(&mut self, relative_path: Option<String>) {
        self.header_mut()
            .update_link_flags(LinkFlags::HAS_RELATIVE_PATH, relative_path.is_some());
        self.relative_path = relative_path;
    }

    /// Get the shell link's working directory, if set
    pub fn working_dir(&self) -> &Option<String> {
        &self.working_dir
    }

    /// Set the shell link's working directory
    pub fn set_working_dir(&mut self, working_dir: Option<String>) {
        self.header_mut()
            .update_link_flags(LinkFlags::HAS_WORKING_DIR, working_dir.is_some());
        self.working_dir = working_dir;
    }

    /// Get the shell link's arguments, if set
    pub fn arguments(&self) -> &Option<String> {
        &self.command_line_arguments
    }

    /// Set the shell link's arguments
    pub fn set_arguments(&mut self, arguments: Option<String>) {
        self.header_mut()
            .update_link_flags(LinkFlags::HAS_ARGUMENTS, arguments.is_some());
        self.command_line_arguments = arguments;
    }

    /// Get the shell link's icon location, if set
    pub fn icon_location(&self) -> &Option<String> {
        &self.icon_location
    }

    /// Set the shell link's icon location
    pub fn set_icon_location(&mut self, icon_location: Option<String>) {
        self.header_mut()
            .update_link_flags(LinkFlags::HAS_ICON_LOCATION, icon_location.is_some());
        self.icon_location = icon_location;
    }
}

#[test]
fn test_create_lnk() {
    let target = r"D:\编程地图书籍、源码\NumPy Essentials.epub";
    let lnk = r"C:\Users\Admin\Desktop\np.lnk";
    let sl = ShellLink::new(target).unwrap();
    sl.create_lnk(lnk).unwrap();
}
