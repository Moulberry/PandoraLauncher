use byteorder::{ByteOrder, LE};
#[allow(unused)]

/// ExtraData refers to a set of structures that convey additional information
/// about a link target. These optional structures can be present in an extra
/// data section that is appended to the basic Shell Link Binary File Format.
///
/// At the moment, ExtraData can only be read, not written to shortcuts.
#[allow(missing_docs)]
#[derive(Clone, Debug)]
pub enum ExtraData {
    ConsoleProps(Vec<u8>),
    ConsoleFeProps(Vec<u8>),
    DarwinProps(Vec<u8>),
    EnvironmentProps(Vec<u8>),
    IconEnvironmentProps(Vec<u8>),
    KnownFolderProps(Vec<u8>),
    PropertyStoreProps(Vec<u8>),
    ShimProps(Vec<u8>),
    SpecialFolderProps(Vec<u8>),
    TrackerProps(Vec<u8>),
    VistaAndAboveIdListProps(Vec<u8>),
}

impl From<&[u8]> for ExtraData {
    fn from(data: &[u8]) -> Self {
        let _size = LE::read_u32(data) as usize;
        let sig = LE::read_u32(&data[4..]);
        let data = &data[8..];

        match sig {
            0xa0000002 => Self::ConsoleProps(data.to_vec()),
            0xa0000004 => Self::ConsoleFeProps(data.to_vec()),
            0xa0000006 => Self::DarwinProps(data.to_vec()),
            0xa0000001 => Self::EnvironmentProps(data.to_vec()),
            0xa0000007 => Self::IconEnvironmentProps(data.to_vec()),
            0xa000000b => Self::KnownFolderProps(data.to_vec()),
            0xa0000009 => Self::PropertyStoreProps(data.to_vec()),
            0xa0000008 => Self::ShimProps(data.to_vec()),
            0xa0000005 => Self::SpecialFolderProps(data.to_vec()),
            0xa0000003 => Self::TrackerProps(data.to_vec()),
            0xa000000a => Self::VistaAndAboveIdListProps(data.to_vec()),
            _ => panic!("Invalid extra data type!"),
        }
    }
}
