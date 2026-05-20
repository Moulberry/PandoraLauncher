#[macro_use]
mod utils;

test_cases!(gzip);

#[allow(unused)]
use utils::{algos::gzip::sync, InputStream};

#[cfg(feature = "futures-io")]
use utils::algos::gzip::futures::bufread;

#[allow(unused)]
fn compress_with_header(data: &[u8]) -> Vec<u8> {
    use flate2::{Compression, GzBuilder};
    use std::io::Write;

    let mut bytes = Vec::new();
    {
        let mut gz = GzBuilder::new()
            .filename("hello_world.txt")
            .comment("test file, please delete")
            .extra(vec![1, 2, 3, 4])
            .write(&mut bytes, Compression::fast());

        gz.write_all(data).unwrap();
    }

    bytes
}

#[test]
#[ntest::timeout(1000)]
#[cfg(feature = "futures-io")]
fn gzip_bufread_decompress_with_extra_header() {
    let bytes = compress_with_header(&[1, 2, 3, 4, 5, 6]);

    let input = InputStream::from(vec![bytes]);
    let output = bufread::decompress(bufread::from(&input));

    assert_eq!(output, &[1, 2, 3, 4, 5, 6][..]);
}

#[test]
#[ntest::timeout(1000)]
#[cfg(feature = "futures-io")]
fn gzip_bufread_chunks_decompress_with_extra_header() {
    let bytes = compress_with_header(&[1, 2, 3, 4, 5, 6]);

    let input = InputStream::from(bytes.chunks(2));
    let output = bufread::decompress(bufread::from(&input));

    assert_eq!(output, &[1, 2, 3, 4, 5, 6][..]);
}

#[test]
#[ntest::timeout(1000)]
#[cfg(feature = "futures-io")]
fn gzip_bufread_chunks_compress_flushes_when_reader_pending() {
    use crate::utils::block_on;
    use async_compression::futures::bufread::GzipEncoder;
    use futures::AsyncRead;
    use futures::AsyncReadExt;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::task::{Context, Poll};

    struct Input {
        remaining: usize,
        chunks_sent: Arc<AtomicUsize>,
        chunks_received: Arc<AtomicUsize>,
    }

    impl AsyncRead for Input {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<std::io::Result<usize>> {
            if self.chunks_received.load(Ordering::Relaxed)
                < self.chunks_sent.load(Ordering::Relaxed)
            {
                Poll::Pending
            } else {
                let bytes = b"X".repeat(buf.len().min(64).min(self.remaining));
                buf[..bytes.len()].copy_from_slice(&bytes);
                let this = self.get_mut();
                this.remaining -= bytes.len();
                this.chunks_sent.fetch_add(1, Ordering::Relaxed);
                Poll::Ready(Ok(bytes.len()))
            }
        }
    }

    let chunks_sent = Arc::new(AtomicUsize::new(0));
    let chunks_received = Arc::new(AtomicUsize::new(0));

    let input = futures::io::BufReader::new(Input {
        remaining: 4 * 1024,
        chunks_sent: Arc::clone(&chunks_sent),
        chunks_received: Arc::clone(&chunks_received),
    });

    let mut encoder = GzipEncoder::new(input);

    let mut encoded_buffer: [u8; 64] = [0; 64];

    block_on(async {
        while let Ok(read) = encoder.read(&mut encoded_buffer).await {
            if read == 0 {
                break;
            }
            chunks_received.fetch_add(1, Ordering::Relaxed);
        }
    });

    assert_eq!(
        chunks_sent.load(Ordering::Relaxed),
        chunks_received.load(Ordering::Relaxed)
    );
}

#[test]
#[ntest::timeout(1000)]
#[cfg(feature = "futures-io")]
fn gzip_bufread_chunks_decompress_without_footer_emits_all_payload() {
    use flate2::bufread::GzDecoder;
    use std::io::Read;

    let mut bytes = compress_with_header(&[1, 2, 3, 4, 5, 6]);

    // Remove the footer.
    bytes.truncate(bytes.len() - 8);

    let mut decoder = GzDecoder::new(bytes.as_slice());

    let mut output = vec![];
    let result = decoder.read_to_end(&mut output);

    assert!(result.is_err());
    assert_eq!(output, &[1, 2, 3, 4, 5, 6][..]);
}
