use std::{
    cell::{Cell, RefCell},
    fs::File,
    io::{Error, ErrorKind, Read, Result, Seek, SeekFrom},
    str,
    sync::Arc,
};

#[cfg(feature = "byteorder")]
extern crate byteorder;
extern crate positioned_io;
extern crate tempfile;
#[cfg(feature = "byteorder")]
use positioned_io::ByteIo;
use positioned_io::{Cursor, RandomAccessFile, ReadAt, Size, SizeCursor, Slice, WriteAt};

#[cfg(feature = "byteorder")]
use self::byteorder::LittleEndian;

#[test]
fn test_read_at() {
    let file = File::open("tests/pi.txt").unwrap();
    let mut buf = [0; 4];
    file.read_exact_at(10, buf.as_mut()).unwrap();
    assert_eq!(&buf, b"3589");
}

#[test]
fn test_mixed_read() {
    let mut file = File::open("tests/pi.txt").unwrap();
    let mut buf = [0; 4];
    file.read_exact(&mut buf[..]).unwrap();
    assert_eq!(&buf, b"3.14");
    file.read_exact_at(764, &mut buf[..]).unwrap();
    assert_eq!(&buf, b"9999");
    file.read_exact(&mut buf[..]).unwrap();
    assert_eq!(&buf, b"1592");
}

// A ReadAt that has weird behavior.
struct ReadCustom<I: ReadAt, F: Fn() -> Result<usize>> {
    i: I,
    fail: Cell<bool>,
    onfail: F,
}
impl<I: ReadAt, F: Fn() -> Result<usize>> ReadCustom<I, F> {
    fn new(i: I, f: F) -> Self {
        ReadCustom {
            i: i,
            fail: Cell::new(true),
            onfail: f,
        }
    }
}
impl<I: ReadAt, F: Fn() -> Result<usize>> ReadAt for ReadCustom<I, F> {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> Result<usize> {
        let fail = self.fail.get();
        self.fail.set(!fail);
        if fail {
            (self.onfail)()
        } else {
            self.i.read_at(pos, buf)
        }
    }
}

#[test]
fn test_read_fails() {
    // Test interrupts.
    let file = File::open("tests/pi.txt").unwrap();
    let mut buf = [0; 4];
    {
        let interrupt = ReadCustom::new(&file, || {
            Err(Error::new(ErrorKind::Interrupted, "interrupt!"))
        });
        interrupt.read_exact_at(10, buf.as_mut()).unwrap();
        assert_eq!(&buf, b"3589");
    }

    // Test errors.
    {
        let fail = ReadCustom::new(&file, || Err(Error::new(ErrorKind::Other, "random fail")));
        assert!(fail.read_exact_at(10, buf.as_mut()).is_err());
    }

    // Test EOF.
    assert!(file.read_exact_at(1000000000, buf.as_mut()).is_err());
}

#[test]
fn test_size() {
    let file = File::open("tests/pi.txt").unwrap();
    let size = file.size().unwrap().unwrap();
    assert_eq!(size, 1000002);
}

#[test]
fn test_cursor() {
    let file = File::open("tests/pi.txt").unwrap();
    let mut curs = Cursor::new_pos(file, 10);
    let mut buf = [0; 4];
    {
        curs.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"3589");
    }
    curs.seek(SeekFrom::Current(4)).unwrap();
    curs.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"3846");
}

#[test]
fn test_size_cursor() {
    let file = File::open("Cargo.toml").unwrap();
    let mut curs = SizeCursor::new_pos(file, 10);
    let mut buf = [0; 4];
    curs.seek(SeekFrom::End(-2)).unwrap();
    assert_eq!(2, curs.read(&mut buf).unwrap());
    let s = str::from_utf8(buf.as_ref()).unwrap();
    assert!(s.contains("\n"));
}

#[test]
fn test_clone_size_cursor() {
    let file = Arc::new(RandomAccessFile::open("tests/pi.txt").unwrap());
    let mut curs = SizeCursor::new(file);
    let mut curs_copy = curs.clone();

    let mut buf = [0; 4];
    assert_eq!(4, curs.read(&mut buf).unwrap());
    let s = str::from_utf8(buf.as_ref()).unwrap();
    assert_eq!(s, "3.14");

    assert_eq!(4, curs_copy.read(&mut buf).unwrap());
    let s = str::from_utf8(buf.as_ref()).unwrap();
    assert_eq!(s, "3.14");
}

#[test]
#[cfg(feature = "byteorder")]
fn test_byteio() {
    let file = File::open("Cargo.toml").unwrap();
    let io: ByteIo<_, LittleEndian> = ByteIo::new(file);
    let r = io.read_i32_at(3).unwrap();
    assert_eq!(0x67616b63, r);
}

#[test]
fn test_vector() {
    // Write past the end.
    let mut v = vec![0, 1, 2, 3];
    let buf = [4, 5, 6, 7];
    (&mut v).write_all_at(2, &buf).unwrap();
    assert_eq!(vec![0, 1, 4, 5, 6, 7], v);
}

#[test]
fn test_slice() {
    let mut v = vec![0, 1, 2, 3, 4, 5];
    {
        // Read too much, limited by size.
        let mut buf = [9; 4];
        let slice = Slice::new(&mut v, 2, Some(3));
        let bytes = slice.read_at(2, &mut buf).unwrap();
        assert_eq!(bytes, 1);
        assert_eq!(buf, [4, 9, 9, 9]);

        // Read too much, limited by buf
        let mut buf = [9; 2];
        let bytes = slice.read_at(1, &mut buf).unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(buf, [3, 4]);
    }

    {
        // Read too much, limited by original vec.
        let slice = Slice::new(&mut v, 2, Some(5));
        let mut buf = [9; 6];
        let bytes = slice.read_at(2, &mut buf).unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(buf, [4, 5, 9, 9, 9, 9]);
    }

    {
        // Read too much of unsized slice.
        let slice = Slice::new(&mut v, 2, None);
        let mut buf = [9; 6];
        let bytes = slice.read_at(2, &mut buf).unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(buf, [4, 5, 9, 9, 9, 9]);
    }

    // Interleaved read/write to same slice.
    let mut v = vec![0, 1, 2, 3, 4, 5];
    {
        let mut slice = Slice::new(&mut v, 2, Some(3));
        let mut buf = [9; 3];
        {
            let bytes = slice.write_at(2, &buf).unwrap();
            assert_eq!(bytes, 1);
        }
        {
            let bytes = slice.read_at(1, &mut buf).unwrap();
            assert_eq!(bytes, 2);
        }
        assert_eq!(buf, [3, 9, 9]);
        {
            let bytes = slice.write_at(0, &buf[..1]).unwrap();
            assert_eq!(bytes, 1);
        }
    }
    assert_eq!(v, vec![0, 1, 3, 3, 9, 5]);

    // Write past end.
    let mut v = vec![0, 1, 2, 3];
    let buf = [9; 6];
    // Limited by size.
    {
        let mut slice = Slice::new(&mut v, 2, Some(5));
        let bytes = slice.write_at(1, &buf).unwrap();
        assert_eq!(bytes, 4);
    }
    assert_eq!(v, vec![0, 1, 2, 9, 9, 9, 9]);
    // No size.
    {
        let mut slice = Slice::new(&mut v, 2, None);
        let bytes = slice.write_at(1, &buf).unwrap();
        assert_eq!(bytes, 6);
    }
    assert_eq!(v, vec![0, 1, 2, 9, 9, 9, 9, 9, 9]);

    // Automatic-size constructor.
    let mut v = vec![0, 1, 2, 3];
    {
        let mut slice = Slice::new_to_end(&mut v, 1).unwrap();
        assert_eq!(slice.size().unwrap().unwrap(), 3);
        let bytes = slice.write_at(0, &buf).unwrap();
        assert_eq!(bytes, 3);
    }
    assert_eq!(v, vec![0, 9, 9, 9]);

    // Slice of slice.
    let mut v = vec![0, 1, 2, 3, 4, 5];
    {
        let mut slice1 = Slice::new(&mut v, 1, Some(4));
        let mut slice2 = Slice::new(&mut slice1, 1, Some(2));
        let bytes = slice2.write_at(0, &buf).unwrap();
        assert_eq!(bytes, 2);
    }
    assert_eq!(v, vec![0, 1, 9, 9, 4, 5]);
}

#[test]
fn test_refcell() {
    let mut buf = [0; 3];

    // Make sure we can instantiate a bunch of combinations.
    let mut a = [0, 1, 2, 3];
    a.as_ref().read_at(0, &mut buf).unwrap();
    a.as_mut().write_at(0, &buf).unwrap();

    let mut v = vec![0, 1, 2, 3];
    v.read_at(0, &mut buf).unwrap();
    v.write_at(0, &buf).unwrap();

    let mut ra = &RefCell::new(a.as_mut());
    ra.read_at(0, &mut buf).unwrap();
    ra.write_at(0, &buf).unwrap();

    let mut rv = &RefCell::new(&mut v);
    rv.read_at(0, &mut buf).unwrap();
    rv.write_at(0, &buf).unwrap();

    // Test writing but still being able to use, thanks to RefCell.
    let mut v = vec![0, 1, 2, 3];
    let mut rv = &RefCell::new(&mut v);
    let buf = [9; 4];
    rv.write_at(0, &buf).unwrap();
    assert_eq!(*rv.borrow(), &vec![9; 4]);
}

#[test]
fn shared_refs() {
    let file = tempfile::tempfile().unwrap();
    // no mut
    let file = RandomAccessFile::try_new(file).unwrap();
    (&file).write_at(1, &[1, 2, 3, 4]).unwrap();
    let mut buf = [0; 3];
    (&file).read_exact_at(0, &mut buf[..]).unwrap();
    assert_eq!(buf, [0, 1, 2])
}
