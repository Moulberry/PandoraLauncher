use bitflags::bitflags;

/// The LinkInfo structure specifies information necessary to resolve a
/// linktarget if it is not found in its original location. This includes
/// information about the volume that the target was stored on, the mapped
/// drive letter, and a Universal Naming Convention (UNC)form of the path
/// if one existed when the linkwas created. For more details about UNC
/// paths, see [MS-DFSNM] section 2.2.1.4
#[derive(Clone, Debug)]
pub struct LinkInfo {
    /// The parsed struct size
    pub size: u32,
    /// Flags that specify whether the VolumeID, LocalBasePath,
    /// LocalBasePathUnicode, and CommonNetworkRelativeLinkfields are present
    /// in this structure.
    link_info_flags: LinkInfoFlags,
    /// An optional, NULL–terminated string, defined by the system default code
    /// page, which is used to construct the full path to the link item or link
    /// target by appending the string in the CommonPathSuffix field. This
    /// field is present if the VolumeIDAndLocalBasePath flag is set.
    local_base_path: Option<String>,
    /// An optional CommonNetworkRelativeLink structure (section 2.3.2) that
    /// specifies information about the network location where the link target
    /// is stored.
    common_network_relative_link: Option<CommonNetworkRelativeLink>,
    /// A NULL–terminated string, defined by the system default code page,
    /// which is used to construct the full path to the link item or link
    /// target by being appended to the string in the LocalBasePath field.
    common_path_suffix: String,
    /// An optional, NULL–terminated, Unicode string that is used to construct
    /// the full path to the link item or link target by appending the string
    /// in the CommonPathSuffixUnicode field. This field can be present only
    /// if the VolumeIDAndLocalBasePath flag is set and the value of the
    /// LinkInfoHeaderSize field is greater than or equal to 0x00000024.
    local_base_path_unicode: Option<String>,
    /// An optional, NULL–terminated, Unicode string that is used to construct
    /// the full path to the link item or link target by being appended to the
    /// string in the LocalBasePathUnicode field. This field can be present
    /// only if the value of the LinkInfoHeaderSize field is greater than or
    /// equal to 0x00000024.
    common_path_suffix_unicode: Option<String>,
}

impl Default for LinkInfo {
    fn default() -> Self {
        Self {
            size: 0,
            link_info_flags: LinkInfoFlags::empty(),

            local_base_path: None,
            common_network_relative_link: None,
            common_path_suffix: String::new(),
            local_base_path_unicode: None,
            common_path_suffix_unicode: None,
        }
    }
}

impl Into<Vec<u8>> for LinkInfo {
    fn into(self) -> Vec<u8> {
        unimplemented!()
    }
}

bitflags! {
    pub struct LinkInfoFlags: u32 {
        /// If set, the VolumeIDand LocalBasePath fields are present, and their
        /// locations are specified by the values of the VolumeIDOffset and
        /// LocalBasePathOffset fields, respectively. If the value of the
        /// LinkInfoHeaderSize field is greater than or equal to 0x00000024, the
        /// LocalBasePathUnicode field is present, and its location is specified
        /// by the value of the LocalBasePathOffsetUnicode field. If not set,
        /// the VolumeID, LocalBasePath, and LocalBasePathUnicode fields are
        /// not present, and the values of the VolumeIDOffset and
        /// LocalBasePathOffset fields are zero. If the value of the
        /// LinkInfoHeaderSize field is greater than or equal to 0x00000024,
        /// the value of the LocalBasePathOffsetUnicode field is zero.
        const VOLUME_ID_AND_LOCAL_BASE_PATH = 0b0000_0000_0000_0000_0000_0000_0000_0001;

        /// If set, the CommonNetworkRelativeLink field is present, and its
        /// location is specified by the value of the
        /// CommonNetworkRelativeLinkOffset field.If not set, the
        /// CommonNetworkRelativeLink field is not present, and the value of
        /// the CommonNetworkRelativeLinkOffset field is zero
        const COMMON_NETWORK_RELATIVE_LINK_AND_PATH_SUFFIX = 0b0000_0000_0000_0000_0000_0000_0000_0010;
    }
}

#[derive(Clone, Debug)]
pub struct CommonNetworkRelativeLink {
    /// Flags that specify the contents of the DeviceNameOffset and
    /// NetProviderType fields.
    flags: CommonNetworkRelativeLinkFlags,
    /// A 32-bit, unsigned integer that specifies the type of network
    /// provider.
    network_provider_type: Option<NetworkProviderType>,
    /// A NULL–terminated string, as defined by the system default code
    /// page, which specifies a server share path; for example,
    /// "\\server\share".
    net_name: String,
    /// A NULL–terminated string, as defined by the system default code
    /// page, which specifies a device; for example, the drive letter
    /// "D:".
    device_name: String,
    /// An optional, NULL–terminated, Unicode string that is the
    /// Unicode version of the NetName string. This field MUST be
    /// present if the value of the NetNameOffset field is greater
    /// than 0x00000014; otherwise, this field MUST NOT be present.
    net_name_unicode: Option<String>,
    /// An optional, NULL–terminated, Unicode string that is the
    /// Unicode version of the DeviceName string. This field MUST be
    /// present if the value of the NetNameOffset field is greater than
    /// 0x00000014; otherwise, this field MUST NOT be present.
    device_name_unicode: Option<String>,
}

impl Default for CommonNetworkRelativeLink {
    fn default() -> Self {
        Self {
            flags: CommonNetworkRelativeLinkFlags::empty(),
            network_provider_type: None,
            net_name: String::new(),
            device_name: String::new(),
            net_name_unicode: None,
            device_name_unicode: None,
        }
    }
}

impl Into<Vec<u8>> for CommonNetworkRelativeLink {
    fn into(self) -> Vec<u8> {
        unimplemented!()
    }
}

bitflags! {
    /// Flags that specify the contents of the DeviceNameOffset and NetProviderType fields.
    pub struct CommonNetworkRelativeLinkFlags: u32 {
        /// If set, the DeviceNameOffset field contains an offset to the device
        /// name. If not set, the DeviceNameOffset field does not contain an
        /// offset to the device name, and its value MUST be zero.
        const VALID_DEVICE = 0b0000_0000_0000_0000_0000_0000_0000_0001;
        /// If set, the NetProviderType field contains the network provider
        /// type. If not set, the NetProviderType field does not contain the
        /// network provider type, and its value MUST be zero.
        const VALID_NET_TYPE = 0b0000_0000_0000_0000_0000_0000_0000_0010;
    }
}

#[derive(Clone, Debug)]
pub enum NetworkProviderType {
    Avid = 0x1a0000,
    Docuspace = 0x1b0000,
    Mangosoft = 0x1c0000,
    Sernet = 0x1d0000,
    Riverfront1 = 0x1e0000,
    Riverfront2 = 0x1f0000,
    Decorb = 0x200000,
    Protstor = 0x210000,
    FjRedir = 0x220000,
    Distinct = 0x230000,
    Twins = 0x240000,
    Rdr2Sample = 0x250000,
    CSC = 0x260000,
    _3In1 = 0x270000,
    ExtendNet = 0x290000,
    Stac = 0x2a0000,
    Foxbat = 0x2b0000,
    Yahoo = 0x2c0000,
    Exifs = 0x2d0000,
    Dav = 0x2e0000,
    Knoware = 0x2f0000,
    ObjectDire = 0x300000,
    Masfax = 0x310000,
    HobNfs = 0x320000,
    Shiva = 0x330000,
    Ibmal = 0x340000,
    Lock = 0x350000,
    Termsrv = 0x360000,
    Srt = 0x370000,
    Quincy = 0x380000,
    Openafs = 0x390000,
    Avid1 = 0x3a0000,
    Dfs = 0x3b0000,
    Kwnp = 0x3c0000,
    Zenworks = 0x3d0000,
    Driveonweb = 0x3e0000,
    Vmware = 0x3f0000,
    Rsfx = 0x400000,
    Mfiles = 0x410000,
    MsNfs = 0x420000,
    Google = 0x430000,
}
