use rust_embed::{Embed, EmbeddedCompressedFile, EmbeddedFile};

#[derive(Embed)]
#[folder = "examples/public/"]
struct Asset;

#[derive(Embed)]
#[folder = "examples/public/"]
#[metadata_only = true]
struct MetadataAsset;

#[test]
fn get_returns_decompressed_content() {
  let file: EmbeddedFile = Asset::get("index.html").expect("index.html exists");
  assert!(!file.data.is_empty());
}

#[test]
fn get_missing_returns_none() {
  assert!(Asset::get("missing.html").is_none());
}

#[test]
fn metadata_only_compressed_always_returns_none() {
  let file: Option<EmbeddedCompressedFile> = MetadataAsset::compressed("index.html");
  assert!(file.is_none(), "metadata_only structs never have compressed data");
}

#[test]
fn metadata_only_get_returns_empty_data() {
  let file: EmbeddedFile = MetadataAsset::get("index.html").expect("index.html exists");
  assert_eq!(file.data.len(), 0);
}

#[cfg(any(not(debug_assertions), feature = "debug-embed"))]
mod release {
  use super::*;

  #[test]
  fn compressed_exists() {
    let file: Option<EmbeddedCompressedFile> = Asset::compressed("index.html");
    assert!(file.is_some(), "index.html should have a compressed version");
  }

  #[test]
  fn compressed_missing_returns_none() {
    let file: Option<EmbeddedCompressedFile> = Asset::compressed("missing.html");
    assert!(file.is_none());
  }

  #[test]
  fn compressed_decompresses_to_original_content() {
    let compressed: EmbeddedCompressedFile = Asset::compressed("index.html").expect("index.html exists");
    let uncompressed: EmbeddedFile = Asset::get("index.html").expect("index.html exists");
    assert_eq!(compressed.data.decoded(), uncompressed.data.as_ref());
  }

  #[test]
  fn compressed_and_get_share_same_hash() {
    let compressed: EmbeddedCompressedFile = Asset::compressed("index.html").expect("index.html exists");
    let uncompressed: EmbeddedFile = Asset::get("index.html").expect("index.html exists");
    assert_eq!(compressed.metadata.sha256_hash(), uncompressed.metadata.sha256_hash());
  }

  #[test]
  fn compressed_bytes_smaller_than_original_for_text() {
    let compressed: EmbeddedCompressedFile = Asset::compressed("index.html").expect("index.html exists");
    let uncompressed: EmbeddedFile = Asset::get("index.html").expect("index.html exists");
    assert!(
      compressed.data.compressed().len() < uncompressed.data.len(),
      "compressed text should be smaller than the original"
    );
  }

  #[test]
  fn content_encoding_is_valid_http_header_value() {
    let compressed: EmbeddedCompressedFile = Asset::compressed("index.html").expect("index.html exists");
    let encoding = compressed.content_encoding();
    assert!(
      encoding == "deflate" || encoding == "zstd",
      "content_encoding should be a known HTTP Content-Encoding value, got: {}",
      encoding
    );
  }

  #[test]
  fn compressed_iter_matches_get_iter() {
    let file_count = Asset::iter().count();
    let compressed_count = Asset::iter().filter_map(|p| Asset::compressed(p.as_ref())).count();
    assert_eq!(file_count, compressed_count, "every file should have a compressed version");
  }
}
