#![cfg(windows)]

use reflink_copy::{
    check_reflink_support, reflink, reflink_or_copy, ReflinkBlockBuilder, ReflinkSupport,
};
use std::fs::File;
use std::io::{Read, Write};
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

const FILE_SIZE: usize = 256 * 1024;
const FILENAME: &str = "test_file.dat";
const CLUSTER_SIZE: usize = 4 * 1024;

// paths are defined in build.yml

fn temp_dir() -> PathBuf {
    PathBuf::from(std::env::var("RUNNER_TEMP").expect("RUNNER_TEMP is not set"))
}
fn refs1_dir() -> PathBuf {
    temp_dir().join("refs1")
}
fn refs2_dir() -> PathBuf {
    temp_dir().join("refs2")
}
fn ntfs_dir() -> PathBuf {
    temp_dir().join("ntfs")
}

fn make_subfolder(folder: &Path, line: u32) -> std::io::Result<PathBuf> {
    let subfolder = folder.join(format!("subfolder_{line}"));
    std::fs::create_dir_all(&subfolder)?;
    Ok(subfolder)
}

fn create_test_file(path: &Path) -> std::io::Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }

    let mut file = File::create(path)?;
    file.write_all(&vec![0u8; FILE_SIZE])?;
    Ok(())
}

#[test]
#[ignore]
fn test_correct_deployment() {
    assert!(temp_dir().join("ntfs.vhdx").exists());
}

#[test]
#[ignore]
fn test_reflink_support_refs1_to_refs2() -> std::io::Result<()> {
    let result = check_reflink_support(refs1_dir(), refs2_dir())?;
    assert_eq!(result, ReflinkSupport::NotSupported);

    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs2_dir(), line!())?;
    let result = check_reflink_support(from, to)?;
    assert_eq!(result, ReflinkSupport::NotSupported);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_support_ntfs_to_refs1() -> std::io::Result<()> {
    let result = check_reflink_support(ntfs_dir(), refs1_dir())?;
    assert_eq!(result, ReflinkSupport::NotSupported);

    let from = make_subfolder(&ntfs_dir(), line!())?;
    let to = make_subfolder(&refs1_dir(), line!())?;
    let result = check_reflink_support(from, to)?;
    assert_eq!(result, ReflinkSupport::NotSupported);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_support_refs1_to_ntfs() -> std::io::Result<()> {
    let result = check_reflink_support(refs1_dir(), ntfs_dir())?;
    assert_eq!(result, ReflinkSupport::NotSupported);

    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&ntfs_dir(), line!())?;
    let result = check_reflink_support(from, to)?;
    assert_eq!(result, ReflinkSupport::NotSupported);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_support_refs1() -> std::io::Result<()> {
    let result = check_reflink_support(refs1_dir(), refs1_dir())?;
    assert_eq!(result, ReflinkSupport::Supported);

    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs1_dir(), line!())?;
    let result = check_reflink_support(from, to)?;
    assert_eq!(result, ReflinkSupport::Supported);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_on_supported_config() -> std::io::Result<()> {
    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs1_dir(), line!())?;
    create_test_file(&from.join(FILENAME))?;
    reflink(from.join(FILENAME), to.join(FILENAME))
}

#[test]
#[ignore]
fn test_reflink_on_unsupported_config() -> std::io::Result<()> {
    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs2_dir(), line!())?;
    create_test_file(&from.join(FILENAME))?;
    let _ = reflink(from.join(FILENAME), to.join(FILENAME)).unwrap_err();
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_or_copy_on_supported_config() -> std::io::Result<()> {
    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs1_dir(), line!())?;
    create_test_file(&from.join(FILENAME))?;
    let result = reflink_or_copy(from.join(FILENAME), to.join(FILENAME))?;
    assert_eq!(result, None);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_or_copy_on_unsupported_config() -> std::io::Result<()> {
    let from = make_subfolder(&refs1_dir(), line!())?;
    let to = make_subfolder(&refs2_dir(), line!())?;
    create_test_file(&from.join(FILENAME))?;
    let result = reflink_or_copy(from.join(FILENAME), to.join(FILENAME))?;
    assert_eq!(result, Some(FILE_SIZE as u64));
    Ok(())
}

fn compare_files_eq(file1: &Path, file2: &Path) -> std::io::Result<()> {
    let mut f1 = File::open(file1)?;
    let mut f2 = File::open(file2)?;
    let block_size = f1.metadata()?.len().min(1024 * 1024) as usize;

    let mut buffer1 = vec![0; block_size];
    let mut buffer2 = vec![0; block_size];

    loop {
        let bytes_read1 = f1.read(&mut buffer1)?;
        let bytes_read2 = f2.read(&mut buffer2)?;
        assert_eq!(bytes_read1, bytes_read2);

        if bytes_read1 == 0 {
            break;
        }
        assert_eq!(&buffer1[..bytes_read1], &buffer2[..bytes_read1]);
    }

    Ok(())
}

#[test]
#[ignore]
fn test_reflink_block_whole_file() -> std::io::Result<()> {
    let num_clusters = 3;
    let data_size = CLUSTER_SIZE * num_clusters;

    let from = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);
    let to = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);

    let mut source_file = File::create_new(&from)?;

    let data: Vec<u8> = (1..=num_clusters)
        .flat_map(|i| vec![i as u8; CLUSTER_SIZE])
        .collect();
    source_file.write_all(&data)?;
    source_file.flush()?;
    assert_eq!(source_file.metadata()?.len(), data_size as u64);

    let mut dest_file = File::create_new(&to)?;

    dest_file.set_len(data_size as u64)?;
    ReflinkBlockBuilder::new(
        &source_file,
        &dest_file,
        NonZeroU64::new(data_size as u64).unwrap(),
    )
    .reflink_block()?;

    dest_file.flush()?;
    drop(source_file);
    drop(dest_file);

    compare_files_eq(&from, &to)?;
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_block_6gb() -> std::io::Result<()> {
    let data_size = 6u64 * 1024 * 1024 * 1024;
    let from = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);
    let to = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);

    let mut source_file = File::create_new(&from)?;
    source_file.set_len(data_size as u64)?;
    // to make test faster, we don't write anything to the file
    source_file.flush()?;
    assert_eq!(source_file.metadata()?.len(), data_size as u64);

    let mut dest_file = File::create_new(&to)?;

    dest_file.set_len(data_size as u64)?;
    ReflinkBlockBuilder::new(
        &source_file,
        &dest_file,
        NonZeroU64::new(data_size as u64).unwrap(),
    )
    .reflink_block()?;

    dest_file.flush()?;
    drop(source_file);
    drop(dest_file);

    compare_files_eq(&from, &to)?;
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_unaligned_file() -> std::io::Result<()> {
    let num_clusters = 3;
    let data_size = (CLUSTER_SIZE * num_clusters + 1) as u64;
    let aligned_data_size = (CLUSTER_SIZE * num_clusters + CLUSTER_SIZE) as u64;

    let from = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);
    let to = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);

    let mut source_file = File::create_new(&from)?;

    let data: Vec<u8> = (1..=num_clusters)
        .flat_map(|i| vec![i as u8; CLUSTER_SIZE])
        .collect();
    source_file.write_all(&data)?;
    source_file.write("+".as_bytes())?;
    source_file.flush()?;
    assert_eq!(source_file.metadata()?.len(), data_size);

    let mut dest_file = File::create_new(&to)?;
    dest_file.set_len(data_size)?;
    println!(
        "reflink {}:0 -> {}:0, block {data_size}",
        from.display(),
        to.display()
    );

    ReflinkBlockBuilder::new(
        &source_file,
        &dest_file,
        NonZeroU64::new(aligned_data_size as u64).unwrap(),
    )
    .reflink_block()?;

    dest_file.flush()?;
    drop(source_file);
    drop(dest_file);

    compare_files_eq(&from, &to)?;
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_source_file() -> std::io::Result<()> {
    let num_clusters = 3;
    let data_size = (CLUSTER_SIZE * num_clusters) as u64;

    let from = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);
    let mut source_file = File::create_new(&from)?;

    let data: Vec<u8> = (1..=num_clusters)
        .flat_map(|i| vec![i as u8; CLUSTER_SIZE])
        .collect();
    source_file.write_all(&data)?;
    source_file.flush()?;
    assert_eq!(source_file.metadata()?.len(), data_size);

    source_file.set_len(data_size * 2)?;
    println!(
        "reflink {}:0 -> {}:{data_size}, block {data_size}",
        from.display(),
        from.display()
    );
    ReflinkBlockBuilder::new(
        &source_file,
        &source_file,
        NonZeroU64::new(data_size as u64).unwrap(),
    )
    .to_offset(data_size as u64)
    .reflink_block()?;
    source_file.flush()?;
    assert_eq!(source_file.metadata()?.len(), data_size * 2);
    drop(source_file);

    let mut file = File::open(from)?;
    let mut buffer1 = vec![0u8; data_size as usize];
    let mut buffer2 = vec![0u8; data_size as usize];
    file.read_exact(buffer1.as_mut_slice())?;
    file.read_exact(buffer2.as_mut_slice())?;
    assert_eq!(buffer1, buffer2);
    Ok(())
}

#[test]
#[ignore]
fn test_reflink_block_reverse() -> std::io::Result<()> {
    let num_clusters = 3;
    let data_size = CLUSTER_SIZE * num_clusters;

    let from = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);
    let to = make_subfolder(&refs2_dir(), line!())?.join(FILENAME);

    let mut source_file = File::create_new(&from)?;

    let data: Vec<Vec<u8>> = (1..=num_clusters)
        .map(|i| vec![i as u8; CLUSTER_SIZE])
        .collect();
    for cluster in &data {
        source_file.write_all(&cluster)?;
    }
    source_file.flush()?;
    assert_eq!(source_file.metadata()?.len(), data_size as u64);

    let mut dest_file = File::create_new(&to)?;

    dest_file.set_len(data_size as u64)?;

    for i in 0..num_clusters {
        let r = num_clusters - 1 - i;
        let from_offset = i * CLUSTER_SIZE;
        let to_offset = r * CLUSTER_SIZE;
        println!(
            "reflink {}:{from_offset} -> {}:{to_offset}, block {CLUSTER_SIZE}",
            from.display(),
            to.display()
        );
        ReflinkBlockBuilder::new(
            &source_file,
            &dest_file,
            NonZeroU64::new(CLUSTER_SIZE as u64).unwrap(),
        )
        .from_offset(from_offset as u64)
        .to_offset(to_offset as u64)
        .reflink_block()?;
    }
    dest_file.flush()?;
    drop(source_file);
    drop(dest_file);

    let mut dest_file = std::fs::OpenOptions::new().read(true).open(&to)?;

    let mut buf = vec![0; CLUSTER_SIZE];
    for i in num_clusters - 1..=0 {
        dest_file.read(buf.as_mut_slice())?;
        assert_eq!(buf, data[i as usize]);
    }

    Ok(())
}
