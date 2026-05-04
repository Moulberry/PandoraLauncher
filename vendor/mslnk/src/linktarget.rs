use byteorder::{ByteOrder, LE};
use std::{
    ffi::OsString, mem::size_of, os::windows::ffi::OsStrExt, path::Path, ptr::null_mut, rc::Rc,
};
/// computer item id (this pc on win10)
const ROOT_FOLDER_SHELL: [u8; 20] = [
    0x14, 0x00, 0x1F, 0x50, 0xE0, 0x4F, 0xD0, 0x20, 0xEA, 0x3A, 0x69, 0x10, 0xA2, 0xD8, 0x08, 0x00,
    0x2B, 0x30, 0x30, 0x9D,
];

/// The LinkTargetIDList structure specifies the target of the link. The presence of this optional
/// structure is specified by the HasLinkTargetIDList bit (LinkFlagssection 2.1.1) in the
/// ShellLinkHeader(section2.1).
fn get_driveitemid(driveletter: char) -> [u8; 25] {
    let mut drive = [
        0x19, 0x00, 0x2f, b'C', b':', b'\\', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    // fix drive letter
    drive[3] = driveletter.to_string().as_bytes()[0];
    drive
}
#[derive(Clone, Debug)]
pub struct LinkTargetIdList {
    /// The size, in bytes, of the IDList field.
    pub size: u16,
    /// A stored IDList structure (section 2.2.1), which contains the item ID list. An IDList
    /// structure conforms to the following ABNF [RFC5234]:
    ///   `IDLIST = *ITEMID TERMINALID`
    id_list: Vec<ItemID>,
}

impl Default for LinkTargetIdList {
    fn default() -> Self {
        Self {
            size: 0,
            id_list: Vec::new(),
        }
    }
}
fn return_fattrflag_clstpind(
    long_name: &str,
    counter: usize,
    total_long_parts: usize,
) -> (u16, u8) {
    if counter + 1 < total_long_parts {
        //   0x01 is_dir
        if long_name.is_ascii() {
            (0x0010u16, 0x31)
        } else {
            // 0x01 is_dir and 0x04 has unicode string
            (0x0010u16, 0x35)
        }
    } else {
        if long_name.is_ascii() {
            // is_file
            (0x0020, 0x32u8)
        } else {
            // is_file and has unicode
            (0x0020, 0x36u8)
        }
    }
}
impl LinkTargetIdList {
    pub fn set_linktarget<P: AsRef<Path>>(&mut self, target: P) {
        let target = target.as_ref().to_owned().to_str().unwrap().to_owned();
        // root folder shell item this pc
        let itemid_computer = ItemID::from(ROOT_FOLDER_SHELL.to_vec().as_slice());
        self.id_list.push(itemid_computer);

        // dirve_item
        let drive_letter = target.chars().nth(0).unwrap();
        let drive = get_driveitemid(drive_letter);
        let itemid_drive = ItemID::from(drive.to_vec().as_slice());
        self.id_list.push(itemid_drive);
        // File entry shell item
        let mut long_pathparts = Path::new(&target)
            .iter()
            .map(|e| e.to_str().unwrap())
            .collect::<Vec<_>>();
        // remove \
        long_pathparts.remove(1);
        let num_filesystemobjects = long_pathparts.len();
        let mut c = 1;
        while c < num_filesystemobjects {
            let long_item = long_pathparts.get(c).unwrap().to_owned();
            let (fattr, class_type_indicator) =
                return_fattrflag_clstpind(long_item, c, num_filesystemobjects);
            // extension block
            let mut fileitem = FileEntryItem::default();
            fileitem.extension_block.version = 0x0008;
            fileitem.extension_block.signature = 0xbeef0004;
            fileitem.extension_block.ctime = 0;
            fileitem.extension_block.atime = 0;
            // win7+
            fileitem.extension_block.unknown_ver_id = 0x002a;
            fileitem.extension_block.unknown_emp = 0;
            fileitem.extension_block.first_offset = 0x0014;
            // construct itemid
            // folder or file
            fileitem.class_type_indicator = class_type_indicator;
            // file attr
            fileitem.file_attrbute_flags = fattr;
            // path parts : test
            fileitem.entry_name = long_item.to_owned();
            fileitem
                .extension_block
                .set_size(fileitem.return_entry_name_wide_vec().len() as u16);
            fileitem.set_size();
            let item_vec: Vec<u8> = fileitem.into();
            let itemid = ItemID::from(item_vec.as_slice());
            self.id_list.push(itemid);
            c += 1;
        }
        // sum up itemid.size
        let mut idlist_size = 2u16;
        for itemid in &self.id_list {
            idlist_size += itemid.size
        }
        self.size = idlist_size;
    }
}

impl Into<Vec<u8>> for LinkTargetIdList {
    fn into(self) -> Vec<u8> {
        let mut data = vec![0, 0];
        LE::write_u16(&mut data[0..2], self.size);
        for id in self.id_list {
            let mut other_data = id.into();
            data.append(&mut other_data);
        }
        // add terminal id 0000
        data.append(&mut 0u16.to_le_bytes().to_vec());
        data
    }
}
/// file entry item
#[derive(Debug, Default)]
struct FileEntryItem {
    size: u16,
    class_type_indicator: u8,
    unknown_empty: u8,
    filesize: u32,
    mtime: u32,
    file_attrbute_flags: u16,
    // usually u8 str,u16 if not ascii,0-terminated
    entry_name: String,
    extension_block: ExtensionBlock,
}
impl FileEntryItem {
    fn set_size(&mut self) {
        // size
        let size_field = 2;
        let unknown_emp = 1;
        let class_tp_indicator = 1;
        let filesize = 4;
        let mt = 4;
        let fatr = 2;
        let entry_name_size = self.return_entry_name_vec().len();
        let extension_block_size = self.extension_block.size;
        self.size = (size_field
            + unknown_emp
            + class_tp_indicator
            + filesize
            + mt
            + fatr
            + entry_name_size
            + extension_block_size as usize) as u16;
    }
    /// wide-string to vec
    fn return_entry_name_wide_vec(&self) -> Vec<u8> {
        let long_name = &self.entry_name;
        let data = OsString::from(long_name).encode_wide().collect::<Vec<_>>();
        let mut data_vec = vec![];
        for i in data {
            let mut v = i.to_le_bytes().to_vec();
            data_vec.append(&mut v);
        }
        data_vec.append(&mut 0u16.to_le_bytes().to_vec());
        data_vec
    }
    /// u8 string if ascii,u16 string if not ascii
    fn return_entry_name_vec(&self) -> Vec<u8> {
        let long_name = &self.entry_name;
        let file_name_vec = if !long_name.is_ascii() {
            let mut m = vec![];
            let a = OsString::from(long_name).encode_wide().collect::<Vec<_>>();
            for i in a {
                m.append(&mut i.to_le_bytes().to_vec());
            }

            m.append(&mut 0u16.to_le_bytes().to_vec());
            m
        } else {
            // if is ascii
            let mut m = long_name.as_bytes().to_vec();
            m.push(0);
            m
        };
        file_name_vec
    }
}
impl Into<Vec<u8>> for FileEntryItem {
    /// into vec add field first_offset
    fn into(self) -> Vec<u8> {
        let len = self.size;
        let mut data = vec![0u8; 14];
        // size
        LE::write_u16(&mut data[0..2], self.size);
        // class_type_indicator 1
        *(data.get_mut(2).unwrap()) = self.class_type_indicator;
        // Unknown (Empty value) 1
        *(data.get_mut(3).unwrap()) = self.unknown_empty;
        // filesize
        LE::write_u32(&mut data[4..8], self.filesize);
        // mtime
        LE::write_u32(&mut data[8..12], self.mtime);
        // file attr
        LE::write_u16(&mut data[12..14], self.file_attrbute_flags);
        // entry name vec
        data.append(&mut self.return_entry_name_vec());
        // extension block vec except fisrst offsert
        let mut exblock = self.extension_block.return_vec_except_1stoffset();
        data.append(&mut exblock);
        // wide string
        data.append(&mut self.return_entry_name_wide_vec());
        // 1st offset
        data.append(&mut self.extension_block.first_offset.to_le_bytes().to_vec());
        assert_eq!(len, data.len() as u16);
        data
    }
}
/// only support version 8,10
#[derive(Debug, Default)]
struct ExtensionBlock {
    size: u16,
    version: u16,
    signature: u32,
    ctime: u32,
    atime: u32,
    unknown_ver_id: u16,
    unknown_emp: u32,
    first_offset: u16,
}
impl ExtensionBlock {
    fn set_size(&mut self, wide_vec_len: u16) {
        self.size = size_of::<ExtensionBlock>() as u16 + wide_vec_len;
    }
    fn return_vec_except_1stoffset(&self) -> Vec<u8> {
        let mut data = vec![0u8; 22];
        // need offset
        LE::write_u16(&mut data[0..2], self.size);
        LE::write_u16(&mut data[2..4], self.version);
        LE::write_u32(&mut data[4..8], self.signature);
        LE::write_u32(&mut data[8..12], self.ctime);
        LE::write_u32(&mut data[12..16], self.atime);
        LE::write_u16(&mut data[16..18], self.unknown_ver_id);
        LE::write_u32(&mut data[18..22], self.unknown_emp);
        data
    }
}

/// The stored IDList structure specifies the format of a persisted item ID list.
#[derive(Clone, Debug)]
pub struct ItemID {
    /// A 16-bit, unsigned integer that specifies the size, in bytes, of the ItemID structure,
    /// including the ItemIDSize field.
    size: u16,
    /// The shell data source-defined data that specifies an item.
    data: Vec<u8>,
}

impl Default for ItemID {
    fn default() -> Self {
        Self {
            size: 0,
            data: Vec::new(),
        }
    }
}

impl From<&[u8]> for ItemID {
    fn from(data: &[u8]) -> Self {
        let mut id = Self::default();
        // include field size u16
        id.size = data.len() as u16;

        id.data = data[2..(id.size as usize)].to_vec();

        id
    }
}

impl Into<Vec<u8>> for ItemID {
    fn into(self) -> Vec<u8> {
        let mut data = vec![0, 0];
        assert_eq!(self.data.len() as u16 + 2, self.size);

        LE::write_u16(&mut data, self.size);
        let mut other_data = self.data;
        data.append(&mut other_data);

        data
    }
}

#[test]
fn tests() {
    let s = r"D:\文档_~1\anki\ANKI__~1\ANKI_S~1.36\ANKI_S~1.EXE";
    let z = Path::new(s);
    let a = OsString::from("文档_~1").encode_wide().collect::<Vec<_>>();
    for i in a {
        println!("{:02x}{:02x}", i.to_le_bytes()[0], i.to_le_bytes()[1]);
    }
    let xx = z.iter().collect::<Vec<_>>();
    println!("{:?}", xx);
}

#[test]
fn test_writele() {
    let mut v = vec![0u8; 3];
    // fail
    LE::write_u16(&mut v, 10u16);
}
