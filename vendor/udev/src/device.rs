use std::str;

use std::ffi::{CStr, CString, OsStr};
use std::io::Result;
use std::marker::PhantomData;
use std::path::Path;
use std::ptr;
use std::str::FromStr;

use libc::{c_char, dev_t};

use list::{Entry, EntryList};
use Udev;
use {ffi, util};

use {AsRaw, FromRaw};

/// A structure that provides access to sysfs/kernel devices.
pub struct Device {
    udev: Udev,
    device: *mut ffi::udev_device,
}

/// Permissible types of UNIX file I/O API device special file.
///
/// See also [`from_devnum`][crate::Device::from_devnum].
#[repr(u8)]
pub enum DeviceType {
    /// UNIX character-style file IO semantics.
    Character = b'c',
    /// UNIX block-style file IO semantics.
    Block = b'b',
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device")
            .field("initialized", &self.is_initialized())
            .field("device_major_minor_number", &self.devnum())
            .field("system_path", &self.syspath())
            .field("device_path", &self.devpath())
            .field("device_node", &self.devnode())
            .field("subsystem_name", &self.subsystem())
            .field("system_name", &self.sysname())
            .field("instance_number", &self.sysnum())
            .field("device_type", &self.devtype())
            .field("driver", &self.driver())
            .field("action", &self.action())
            .field("parent", &self.parent())
            .finish()
    }
}

impl Clone for Device {
    fn clone(&self) -> Self {
        Self {
            udev: self.udev.clone(),
            device: unsafe { ffi::udev_device_ref(self.device) },
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            ffi::udev_device_unref(self.device);
        }
    }
}

#[cfg(feature = "send")]
unsafe impl Send for Device {}
#[cfg(feature = "sync")]
unsafe impl Sync for Device {}

as_ffi_with_context!(Device, device, ffi::udev_device, ffi::udev_device_ref);

/// A convenience alias for a list of properties, bound to a device.
pub type Properties<'a> = EntryList<'a, Device>;

/// A convenience alias for a list of attributes, bound to a device.
pub struct Attributes<'a> {
    entries: EntryList<'a, Device>,
    device: &'a Device,
}

impl Device {
    /// Creates a device for a given syspath.
    ///
    /// The `syspath` parameter should be a path to the device file within the `sysfs` file system,
    /// e.g., `/sys/devices/virtual/tty/tty0`.
    pub fn from_syspath(syspath: &Path) -> Result<Self> {
        // Create a new Udev context for this device
        // It would be more efficient to allow callers to create just one context and use multiple
        // devices, however that would be an API-breaking change.
        //
        // When devices are enumerated using an `Enumerator`, it will use
        // `from_syspath_with_context` which can reuse the existing `Udev` context to avoid this
        // extra overhead.
        let udev = Udev::new()?;

        Self::from_syspath_with_context(udev, syspath)
    }

    /// Creates a device for a given syspath, using an existing `Udev` instance rather than
    /// creating one automatically.
    ///
    /// The `syspath` parameter should be a path to the device file within the `sysfs` file system,
    /// e.g., `/sys/devices/virtual/tty/tty0`.
    pub fn from_syspath_with_context(udev: Udev, syspath: &Path) -> Result<Self> {
        let syspath = util::os_str_to_cstring(syspath)?;

        let ptr = try_alloc!(unsafe {
            ffi::udev_device_new_from_syspath(udev.as_raw(), syspath.as_ptr())
        });

        Ok(Self::from_raw(udev, ptr))
    }

    /// Create new udev device, and fill in information from the sys device
    /// and the udev database entry.
    ///
    /// The device is looked up by the `subsystem` and `sysname` string of the device, like "mem" / "zero", or "block" / "sda".
    pub fn from_subsystem_sysname(subsystem: String, sysname: String) -> Result<Self> {
        let subsystem = CString::new(subsystem.as_bytes())
            .ok()
            .ok_or(std::io::Error::from_raw_os_error(libc::EINVAL))?;

        let sysname = CString::new(sysname.as_bytes())
            .ok()
            .ok_or(std::io::Error::from_raw_os_error(libc::EINVAL))?;

        let udev = Udev::new()?;

        let ptr = try_alloc!(unsafe {
            ffi::udev_device_new_from_subsystem_sysname(
                udev.as_raw(),
                subsystem.as_ptr(),
                sysname.as_ptr(),
            )
        });

        Ok(Self::from_raw(udev, ptr))
    }

    /// Create new udev device, and fill in information from the sys device
    /// and the udev database entry, using an existing `Udev` instance rather than
    /// creating a new one.
    ///
    /// The device is looked up by the `subsystem` and `sysname` string of the device, like "mem" / "zero", or "block" / "sda".
    pub fn from_subsystem_sysname_with_context(
        udev: Udev,
        subsystem: String,
        sysname: String,
    ) -> Result<Self> {
        let subsystem = CString::new(subsystem.as_bytes())
            .ok()
            .ok_or(std::io::Error::from_raw_os_error(libc::EINVAL))?;

        let sysname = CString::new(sysname.as_bytes())
            .ok()
            .ok_or(std::io::Error::from_raw_os_error(libc::EINVAL))?;

        let ptr = try_alloc!(unsafe {
            ffi::udev_device_new_from_subsystem_sysname(
                udev.as_raw(),
                subsystem.as_ptr(),
                sysname.as_ptr(),
            )
        });

        Ok(Self::from_raw(udev, ptr))
    }

    /// Creates a rust udev `Device` for a given UNIX device "special file" type and number.
    ///
    /// The `dev_type` parameter indicates which of the historical UNIX file-like I/O paradigms the
    /// device permits, and is either [`DeviceType::Character`] or [`DeviceType::Block`].
    ///
    /// n.b. This function follows the naming used by the underlying `libudev` function. As with
    /// the underlying function, there is **no** **direct** **correspondence** between this
    /// function's `dev_type` parameter and string values returned by [`devtype`][Self::devtype].
    /// i.e. They represent different underlying concepts within the OS kernel.
    ///
    /// The `devnum` parameter is of type [`libc::dev_t`][libc::dev_t] which encodes the historical
    /// UNIX major and minor device numbers (see below).
    ///
    /// Typically both parameters would be determined at run-time by calling one of the `stat`
    /// family of system calls (or Rust std library functions which utilise them) on a filesystem
    /// "special file" inode (e.g. `/dev/null`) or (more commonly) on a symbolic link to such a
    /// file which was created by the `udevd` system daemon such as those under `/dev/disk/`.
    ///
    /// ```
    /// use std::{env, fs, os::linux::fs::MetadataExt};
    /// use udev::DeviceType;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let args: Vec<String> = env::args().collect();
    ///     # // Examples are automatically run as tests: provide dummy args for cargo test.
    ///     # let args: Vec<String> = vec!("testname".into(), "/dev/null".into());
    ///     let path = args.get(1).expect("No filename given");
    ///     let metadata = fs::metadata(path).unwrap_or_else(|_| panic!("Can't open file: {}", path));
    ///     let devtype = match metadata.st_mode() & libc::S_IFMT {
    ///         libc::S_IFCHR => Some(DeviceType::Character),
    ///         libc::S_IFBLK => Some(DeviceType::Block),
    ///         _ => None,
    ///     }.expect("Not a character or block special file");
    ///     let ud = udev::Device::from_devnum(devtype, metadata.st_rdev())
    ///         .expect("Couldn't construct udev from supplied path");
    ///     println!("syspath of {} is {:?}", path, ud.syspath());
    ///     let dn = ud.devnum();
    ///     println!("devnum: {}", dn.unwrap());
    ///     Ok(())
    /// }
    /// ```
    /// The user should be aware that a given device may change its major and/or minor number
    /// across reboots, when the hardware attached to the device is subject to hot-plug events, or
    /// for a variety of other reasons.
    ///
    /// The `udevd` system daemon (or equivalent) is configured to dynamically create filesystem
    /// symbolic links (examples of which can be seen under e.g. `/dev/disk/by-id/` on most Linux
    /// systems), the purpose of which is to provide a predictable and persistent means of
    /// identifying devices which themselves have a persistent state or identity.
    ///
    /// Code similar to the sample presented above may be used to obtain a [`udev::Device`][Self]
    /// corresponding to the filesystem path of the UNIX file I/O style device node or symbolic
    /// link.
    ///
    /// Historical UNIX systems statically allocated their internal data structures which were
    /// associated with devices that exposed a "file-like" user-space API (e.g. `/dev/null`). A
    /// device could be uniquely and persistently identified by combining its type (either
    /// "character" or "block"), with its major and minor device numbers.
    ///
    /// In the underlying OS kernel, a major number might be allocated to a single device driver
    /// such as a SCSI disk controller, and that device driver would allocate the minor device
    /// number (e.g. `4` might have represented the 4th SCSI device addressable by a particular
    /// SCSI host adapter). The `mknod` system utility would be used to create friendly filesystem
    /// paths in the filesystem, which corresponded with these attributes, and file permissions
    /// would be managed with utilities such as `chown` and `chmod` etc. and the numbers would not
    /// change between system reboots.
    ///
    /// As has been noted, modern UNIX-like operating systems dynamically allocate devices. To
    /// provide backward compatibility with existing user-space APIs, the concept of major/minor
    /// devices being associated with file system "special file" inodes has been retained.
    ///
    /// For udev devices which present a UNIX file I/O style interface (i.e. via `/dev/` paths),
    /// the Linux `udevadm` utility currently reports devices belonging to the `"block"` subsystem
    /// to be of type "block", and all other file I/O style udev devices to be of type "character".
    ///
    /// Those needing to compose or decompose values of type `dev_t` should refer to
    /// [`libc::major`], [`libc::minor`], [`libc::makedev`] and equivalent functionality from
    /// higher-level rust crates.
    pub fn from_devnum(dev_type: self::DeviceType, devnum: dev_t) -> Result<Self> {
        let udev = Udev::new()?;

        Self::from_devnum_with_context(udev, dev_type, devnum)
    }

    /// Creates a rust udev `Device` for a given UNIX device "special file" type and number. Uses
    /// an existing [`Udev`] instance rather than creating one automatically.
    ///
    /// See [`from_devnum`][Self::from_devnum] for detailed usage.
    pub fn from_devnum_with_context(
        udev: Udev,
        dev_type: self::DeviceType,
        devnum: dev_t,
    ) -> Result<Self> {
        let ptr = try_alloc!(unsafe {
            ffi::udev_device_new_from_devnum(udev.as_raw(), dev_type as c_char, devnum)
        });

        Ok(Self::from_raw(udev, ptr))
    }

    /// Creates a rust `Device` given an already created libudev `ffi::udev_device*` and a
    /// corresponding `Udev` instance from which the device was created.
    ///
    /// This guarantees that the `Udev` will live longer than the corresponding `Device`
    pub(crate) fn from_raw(udev: Udev, ptr: *mut ffi::udev_device) -> Self {
        Self { udev, device: ptr }
    }

    /// Checks whether the device has already been handled by udev.
    ///
    /// When a new device is connected to the system, udev initializes the device by setting
    /// permissions, renaming network devices, and possibly other initialization routines. This
    /// method returns `true` if udev has performed all of its work to initialize this device.
    ///
    /// This method only applies to devices with device nodes or network interfaces. All other
    /// devices return `true` by default.
    pub fn is_initialized(&self) -> bool {
        unsafe { ffi::udev_device_get_is_initialized(self.device) > 0 }
    }

    /// Gets the device's major/minor number.
    pub fn devnum(&self) -> Option<dev_t> {
        match unsafe { ffi::udev_device_get_devnum(self.device) } {
            0 => None,
            n => Some(n),
        }
    }

    /// Returns the syspath of the device.
    ///
    /// The path is an absolute path and includes the sys mount point. For example, the syspath for
    /// `tty0` could be `/sys/devices/virtual/tty/tty0`, which includes the sys mount point,
    /// `/sys`.
    pub fn syspath(&self) -> &Path {
        Path::new(unsafe {
            util::ptr_to_os_str_unchecked(ffi::udev_device_get_syspath(self.device))
        })
    }

    /// Returns the kernel devpath value of the device.
    ///
    /// The path does not contain the sys mount point, but does start with a `/`. For example, the
    /// devpath for `tty0` could be `/devices/virtual/tty/tty0`.
    pub fn devpath(&self) -> &OsStr {
        unsafe { util::ptr_to_os_str_unchecked(ffi::udev_device_get_devpath(self.device)) }
    }

    /// Returns the path to the device node belonging to the device.
    ///
    /// The path is an absolute path and starts with the device directory. For example, the device
    /// node for `tty0` could be `/dev/tty0`.
    pub fn devnode(&self) -> Option<&Path> {
        unsafe { util::ptr_to_os_str(ffi::udev_device_get_devnode(self.device)) }
            .map(|path| Path::new(path))
    }

    /// Returns the parent of the device.
    pub fn parent(&self) -> Option<Self> {
        let ptr = unsafe { ffi::udev_device_get_parent(self.device) };

        if ptr.is_null() {
            return None;
        }

        Some(Self::from_raw(self.udev.clone(), unsafe {
            ffi::udev_device_ref(ptr)
        }))
    }

    /// Returns the parent of the device with the matching subsystem and devtype if any.
    pub fn parent_with_subsystem<T: AsRef<OsStr>>(&self, subsystem: T) -> Result<Option<Self>> {
        let subsystem = util::os_str_to_cstring(subsystem)?;
        let ptr = unsafe {
            ffi::udev_device_get_parent_with_subsystem_devtype(
                self.device,
                subsystem.as_ptr(),
                ptr::null(),
            )
        };

        if ptr.is_null() {
            return Ok(None);
        }

        Ok(Some(Self::from_raw(self.udev.clone(), unsafe {
            ffi::udev_device_ref(ptr)
        })))
    }

    /// Returns the parent of the device with the matching subsystem and devtype if any.
    pub fn parent_with_subsystem_devtype<T: AsRef<OsStr>, U: AsRef<OsStr>>(
        &self,
        subsystem: T,
        devtype: U,
    ) -> Result<Option<Self>> {
        let subsystem = util::os_str_to_cstring(subsystem)?;
        let devtype = util::os_str_to_cstring(devtype)?;
        let ptr = unsafe {
            ffi::udev_device_get_parent_with_subsystem_devtype(
                self.device,
                subsystem.as_ptr(),
                devtype.as_ptr(),
            )
        };

        if ptr.is_null() {
            return Ok(None);
        }

        Ok(Some(Self::from_raw(self.udev.clone(), unsafe {
            ffi::udev_device_ref(ptr)
        })))
    }

    /// Returns the subsystem name of the device.
    ///
    /// The subsystem name is a string that indicates which kernel subsystem the device belongs to.
    /// Examples of subsystem names are `tty`, `vtconsole`, `block`, `scsi`, and `net`.
    pub fn subsystem(&self) -> Option<&OsStr> {
        unsafe { util::ptr_to_os_str(ffi::udev_device_get_subsystem(self.device)) }
    }

    /// Returns the kernel device name for the device.
    ///
    /// The sysname is a string that differentiates the device from others in the same subsystem.
    /// For example, `tty0` is the sysname for a TTY device that differentiates it from others,
    /// such as `tty1`.
    pub fn sysname(&self) -> &OsStr {
        unsafe { util::ptr_to_os_str_unchecked(ffi::udev_device_get_sysname(self.device)) }
    }

    /// Returns the instance number of the device.
    ///
    /// The instance number is used to differentiate many devices of the same type. For example,
    /// `/dev/tty0` and `/dev/tty1` are both TTY devices but have instance numbers of 0 and 1,
    /// respectively.
    ///
    /// Some devices don't have instance numbers, such as `/dev/console`, in which case the method
    /// returns `None`.
    pub fn sysnum(&self) -> Option<usize> {
        let ptr = unsafe { ffi::udev_device_get_sysnum(self.device) };

        if ptr.is_null() {
            return None;
        }

        match str::from_utf8(unsafe { CStr::from_ptr(ptr) }.to_bytes()) {
            Err(_) => None,
            Ok(s) => FromStr::from_str(s).ok(),
        }
    }

    /// Returns the devtype name of the device (if any), for example "disk".
    pub fn devtype(&self) -> Option<&OsStr> {
        unsafe { util::ptr_to_os_str(ffi::udev_device_get_devtype(self.device)) }
    }

    /// Returns the name of the kernel driver attached to the device.
    pub fn driver(&self) -> Option<&OsStr> {
        unsafe { util::ptr_to_os_str(ffi::udev_device_get_driver(self.device)) }
    }

    /// Retrieves the value of a device property.
    pub fn property_value<T: AsRef<OsStr>>(&self, property: T) -> Option<&OsStr> {
        let prop = match util::os_str_to_cstring(property) {
            Ok(s) => s,
            Err(_) => return None,
        };

        unsafe {
            util::ptr_to_os_str(ffi::udev_device_get_property_value(
                self.device,
                prop.as_ptr(),
            ))
        }
    }

    /// Retrieves the value of a device attribute.
    pub fn attribute_value<T: AsRef<OsStr>>(&self, attribute: T) -> Option<&OsStr> {
        let attr = match util::os_str_to_cstring(attribute) {
            Ok(s) => s,
            Err(_) => return None,
        };

        unsafe {
            util::ptr_to_os_str(ffi::udev_device_get_sysattr_value(
                self.device,
                attr.as_ptr(),
            ))
        }
    }

    /// Sets the value of a device attribute.
    pub fn set_attribute_value<T: AsRef<OsStr>, U: AsRef<OsStr>>(
        &mut self,
        attribute: T,
        value: U,
    ) -> Result<()> {
        let attribute = util::os_str_to_cstring(attribute)?;
        let value = util::os_str_to_cstring(value)?;

        util::errno_to_result(unsafe {
            ffi::udev_device_set_sysattr_value(
                self.device,
                attribute.as_ptr(),
                value.as_ptr() as *mut c_char,
            )
        })
    }

    /// Returns an iterator over the device's properties.
    ///
    /// ## Example
    ///
    /// This example prints out all of a device's properties:
    ///
    /// ```no_run
    /// # use std::path::Path;
    /// # let device = udev::Device::from_syspath(Path::new("/sys/devices/virtual/tty/tty0")).unwrap();
    /// for property in device.properties() {
    ///     println!("{:?} = {:?}", property.name(), property.value());
    /// }
    /// ```
    pub fn properties(&self) -> Properties {
        Properties {
            entry: unsafe { ffi::udev_device_get_properties_list_entry(self.device) },
            phantom: PhantomData,
        }
    }

    /// Returns an iterator over the device's attributes.
    ///
    /// ## Example
    ///
    /// This example prints out all of a device's attributes:
    ///
    /// ```no_run
    /// # use std::path::Path;
    /// # let device = udev::Device::from_syspath(Path::new("/sys/devices/virtual/tty/tty0")).unwrap();
    /// for attribute in device.attributes() {
    ///     println!("{:?} = {:?}", attribute.name(), attribute.value());
    /// }
    /// ```
    pub fn attributes(&self) -> Attributes {
        Attributes {
            entries: EntryList {
                entry: unsafe { ffi::udev_device_get_sysattr_list_entry(self.device) },
                phantom: PhantomData,
            },
            device: self,
        }
    }

    /// Returns the device action for the device.
    pub fn action(&self) -> Option<&OsStr> {
        unsafe { util::ptr_to_os_str(ffi::udev_device_get_action(self.device)) }
    }
}

impl<'a> Iterator for Attributes<'a> {
    type Item = Entry<'a>;

    // The list of sysattr entries only contains the attribute names, with
    // the values being empty. To get the value, each has to be queried.
    fn next(&mut self) -> Option<Entry<'a>> {
        match self.entries.next() {
            Some(Entry { name, value: _ }) => Some(Entry {
                name,
                value: self.device.attribute_value(name),
            }),
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
