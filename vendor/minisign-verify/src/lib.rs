//! A small crate to verify [Minisign](https://jedisct1.github.io/minisign/) signatures.
//!
//! This library provides zero-dependency verification of Minisign signatures. Minisign is a
//! dead simple tool to sign files and verify signatures, developed by Frank Denis
//! (author of libsodium).
//!
//! ## Features
//!
//! * Verify signatures for both standard and pre-hashed modes
//! * Streaming verification for large files
//! * No external dependencies
//! * Simple, auditable code
//!
//! ## Basic Usage
//!
//! ```rust
//! use minisign_verify::{PublicKey, Signature};
//!
//! // Create a public key from a base64 string
//! let public_key =
//!     PublicKey::from_base64("RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3")
//!         .expect("Unable to decode the public key");
//!
//! // Create a signature from a string
//! let signature = Signature::decode(
//!     "untrusted comment: signature from minisign secret key
//! RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/\
//!      z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=
//! trusted comment: timestamp:1633700835\tfile:test\tprehashed
//! wLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==",
//! )
//! .expect("Unable to decode the signature");
//!
//! // Verify the signature
//! let bin = b"test";
//! public_key
//!     .verify(&bin[..], &signature, false)
//!     .expect("Signature didn't verify");
//! ```
//!
//! ## Loading from Files
//!
//! ```rust,no_run
//! use minisign_verify::{PublicKey, Signature};
//! use std::path::Path;
//!
//! // Load a public key from a file
//! let public_key = PublicKey::from_file(Path::new("minisign.pub"))
//!     .expect("Unable to load the public key");
//!
//! // Load a signature from a file
//! let signature = Signature::from_file(Path::new("file.sig"))
//!     .expect("Unable to load the signature");
//!
//! // Load the file content to verify
//! let content = std::fs::read("file").expect("Unable to read the file");
//!
//! // Verify the signature
//! public_key
//!     .verify(&content, &signature, false)
//!     .expect("Signature didn't verify");
//! ```
//!
//! ## Streaming Verification
//!
//! For large files, you can use streaming verification to avoid loading
//! the entire file into memory at once:
//!
//! ```rust,no_run
//! use minisign_verify::{PublicKey, Signature};
//! use std::fs::File;
//! use std::io::{self, Read};
//! use std::path::Path;
//!
//! // Load a public key and signature
//! let public_key = PublicKey::from_file(Path::new("minisign.pub"))
//!     .expect("Unable to load the public key");
//!
//! let signature = Signature::from_file(Path::new("large_file.sig"))
//!     .expect("Unable to load the signature");
//!
//! // Create a stream verifier
//! let mut verifier = public_key.verify_stream(&signature)
//!     .expect("Unable to create stream verifier");
//!
//! // Process the file in chunks
//! let mut file = File::open("large_file").expect("Unable to open file");
//! let mut buffer = [0u8; 8192]; // 8KB buffer
//!
//! loop {
//!     let bytes_read = file.read(&mut buffer).expect("Error reading file");
//!     if bytes_read == 0 {
//!         break; // End of file
//!     }
//!
//!     verifier.update(&buffer[..bytes_read]);
//! }
//!
//! // Verify the signature
//! verifier.finalize().expect("Signature verification failed");
//! ```
//!
//! Note that the streaming verification mode only works with pre-hashed signatures
//! (the default in newer versions of Minisign).

mod base64;
mod crypto;

use std::path::Path;
use std::{fmt, fs, io};

use base64::{Base64, Decoder};

use crate::crypto::blake2b::{Blake2b, BLAKE2B_OUTBYTES};
use crate::crypto::ed25519;
#[derive(Debug)]
pub enum Error {
    /// The provided string couldn't be decoded properly
    InvalidEncoding,
    /// The signature verification failed
    InvalidSignature,
    /// An I/O error occurred
    IoError(io::Error),
    /// The algorithm doesn't match what was expected
    UnexpectedAlgorithm,
    /// The key ID from the signature doesn't match the public key
    UnexpectedKeyId,
    /// The specified algorithm is not supported by this implementation
    UnsupportedAlgorithm,
    /// Legacy mode is not supported in streaming verification
    UnsupportedLegacyMode,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidEncoding => write!(f, "Invalid encoding in minisign data"),
            Error::InvalidSignature => write!(f, "The signature verification failed"),
            Error::IoError(e) => write!(f, "I/O error: {}", e),
            Error::UnexpectedAlgorithm => write!(f, "Unexpected signature algorithm"),
            Error::UnexpectedKeyId => write!(
                f,
                "The signature was created with a different key than the one provided"
            ),
            Error::UnsupportedAlgorithm => write!(
                f,
                "This signature algorithm is not supported by this implementation"
            ),
            Error::UnsupportedLegacyMode => {
                write!(f, "StreamVerifier only supports non-legacy mode signatures")
            }
        }
    }
}

impl std::error::Error for Error {
    // Note: description() is deprecated in favor of Display implementation

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<base64::Error> for Error {
    fn from(_e: base64::Error) -> Error {
        // We could consider adding a dedicated Base64Error variant that includes the original error
        // in the future, but for now we'll just use InvalidEncoding
        Error::InvalidEncoding
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IoError(e)
    }
}

/// A Minisign public key
///
/// This struct represents a Minisign public key, which can be used to verify
/// signatures. A public key can be created from a base64 string, read from a file,
/// or parsed from a string in minisign.pub format.
///
/// The public key contains an Ed25519 key, a key ID for signature matching,
/// and optionally an untrusted comment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicKey {
    untrusted_comment: Option<String>,
    signature_algorithm: [u8; 2],
    key_id: [u8; 8],
    key: [u8; 32],
}

/// A StreamVerifier to verify a signature against a data stream
///
/// This mode of operation allows for verification of large files by processing them
/// in chunks, without having to load the entire file into memory.
///
/// Note that this mode only works with pre-hashed signatures (not legacy mode).
#[derive(Clone)]
pub struct StreamVerifier<'a> {
    public_key: &'a PublicKey,
    signature: &'a Signature,
    hasher: Blake2b,
}

/// A Minisign signature
///
/// This struct represents a Minisign signature, which contains:
/// - An untrusted comment (usually identifies the key that created the signature)
/// - The signature itself (Ed25519 signature of the message or its hash)
/// - A trusted comment (usually contains timestamp and filename)
/// - A global signature (Ed25519 signature of the signature and trusted comment)
/// - A flag indicating if the signature was created in pre-hashed mode
///
/// Pre-hashed mode is the default in newer versions of Minisign.
#[derive(Clone)]
pub struct Signature {
    untrusted_comment: String,
    key_id: [u8; 8],
    signature: [u8; 64],
    trusted_comment: String,
    global_signature: [u8; 64],
    is_prehashed: bool,
}

impl Signature {
    /// Create a Minisign signature from a string
    pub fn decode(lines_str: &str) -> Result<Self, Error> {
        let mut lines = lines_str.lines();
        let untrusted_comment = lines.next().ok_or(Error::InvalidEncoding)?.to_string();
        let bin1 = Base64::decode_to_vec(lines.next().ok_or(Error::InvalidEncoding)?)?;
        if bin1.len() != 74 {
            return Err(Error::InvalidEncoding);
        }
        let trusted_comment = lines.next().ok_or(Error::InvalidEncoding)?.to_string();
        let bin2 = Base64::decode_to_vec(lines.next().ok_or(Error::InvalidEncoding)?)?;
        if bin2.len() != 64 {
            return Err(Error::InvalidEncoding);
        }
        if !trusted_comment.starts_with("trusted comment: ") {
            return Err(Error::InvalidEncoding);
        }
        let mut signature_algorithm = [0u8; 2];
        signature_algorithm.copy_from_slice(&bin1[0..2]);
        let mut key_id = [0u8; 8];
        key_id.copy_from_slice(&bin1[2..10]);
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&bin1[10..74]);
        let mut global_signature = [0u8; 64];
        global_signature.copy_from_slice(&bin2);
        let is_prehashed = match (signature_algorithm[0], signature_algorithm[1]) {
            (0x45, 0x64) => false,
            (0x45, 0x44) => true,
            _ => return Err(Error::UnsupportedAlgorithm),
        };
        Ok(Signature {
            untrusted_comment,
            key_id,
            signature,
            trusted_comment,
            global_signature,
            is_prehashed,
        })
    }

    /// Load a Minisign signature from a `.sig` file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let bin = fs::read_to_string(path)?;
        Signature::decode(&bin)
    }

    /// Return the trusted comment of the signature
    pub fn trusted_comment(&self) -> &str {
        &self.trusted_comment[17..]
    }

    /// Return the untrusted comment of the signature
    pub fn untrusted_comment(&self) -> &str {
        &self.untrusted_comment
    }
}

impl PublicKey {
    /// Create a Minisign public key from a base64 string
    pub fn from_base64(public_key_b64: &str) -> Result<Self, Error> {
        let bin = Base64::decode_to_vec(public_key_b64)?;
        if bin.len() != 42 {
            return Err(Error::InvalidEncoding);
        }
        let mut signature_algorithm = [0u8; 2];
        signature_algorithm.copy_from_slice(&bin[0..2]);
        match (signature_algorithm[0], signature_algorithm[1]) {
            (0x45, 0x64) | (0x45, 0x44) => {}
            _ => return Err(Error::UnsupportedAlgorithm),
        };
        let mut key_id = [0u8; 8];
        key_id.copy_from_slice(&bin[2..10]);
        let mut key = [0u8; 32];
        key.copy_from_slice(&bin[10..42]);
        Ok(PublicKey {
            untrusted_comment: None,
            signature_algorithm,
            key_id,
            key,
        })
    }

    /// Create a Minisign public key from a string, as in the `minisign.pub`
    /// file
    pub fn decode(lines_str: &str) -> Result<Self, Error> {
        let mut lines = lines_str.lines();
        let untrusted_comment = lines.next().ok_or(Error::InvalidEncoding)?;
        let public_key_b64 = lines.next().ok_or(Error::InvalidEncoding)?;
        let mut public_key = PublicKey::from_base64(public_key_b64)?;
        public_key.untrusted_comment = Some(untrusted_comment.to_string());
        Ok(public_key)
    }

    /// Load a Minisign key from a file (such as the `minisign.pub` file)
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let bin = fs::read_to_string(path)?;
        PublicKey::decode(&bin)
    }

    /// Return the untrusted comment, if there is one
    pub fn untrusted_comment(&self) -> Option<&str> {
        self.untrusted_comment.as_deref()
    }

    fn verify_ed25519(&self, bin: &[u8], signature: &Signature) -> Result<(), Error> {
        if !ed25519::verify(bin, &self.key, &signature.signature) {
            return Err(Error::InvalidSignature);
        }
        let trusted_comment_bin = signature.trusted_comment().as_bytes();
        let mut global = Vec::with_capacity(signature.signature.len() + trusted_comment_bin.len());
        global.extend_from_slice(&signature.signature[..]);
        global.extend_from_slice(trusted_comment_bin);
        if !ed25519::verify(&global, &self.key, &signature.global_signature) {
            return Err(Error::InvalidSignature);
        }
        Ok(())
    }

    /// Verify that `signature` is a valid signature for `bin` using this public
    /// key `allow_legacy` should only be set to `true` in order to support
    /// signatures made by older versions of Minisign.
    pub fn verify(
        &self,
        bin: &[u8],
        signature: &Signature,
        allow_legacy: bool,
    ) -> Result<(), Error> {
        if self.key_id != signature.key_id {
            return Err(Error::UnexpectedKeyId);
        }
        let mut h;
        let bin = if signature.is_prehashed {
            h = vec![0u8; BLAKE2B_OUTBYTES];
            Blake2b::blake2b(&mut h, bin);
            &h
        } else if !allow_legacy {
            return Err(Error::UnexpectedAlgorithm);
        } else {
            bin
        };
        self.verify_ed25519(bin, signature)
    }

    /// Sets up a stream verifier that can be use iteratively.
    pub fn verify_stream<'a>(
        &'a self,
        signature: &'a Signature,
    ) -> Result<StreamVerifier<'a>, Error> {
        if self.key_id != signature.key_id {
            return Err(Error::UnexpectedKeyId);
        }
        if !signature.is_prehashed {
            return Err(Error::UnsupportedLegacyMode);
        }
        let hasher = Blake2b::new(BLAKE2B_OUTBYTES);
        Ok(StreamVerifier {
            public_key: self,
            signature,
            hasher,
        })
    }
}

impl StreamVerifier<'_> {
    /// Update the verifier with a chunk of data
    ///
    /// This method can be called multiple times with different chunks of the file
    /// to be verified. The chunks will be hashed incrementally.
    pub fn update(&mut self, buf: &[u8]) {
        self.hasher.update(buf);
    }

    /// Finalize the verification process
    ///
    /// This method must be called after all data has been processed with `update()`.
    /// It computes the final hash and verifies the signature.
    ///
    /// Returns `Ok(())` if the signature is valid, or an error otherwise.
    pub fn finalize(&mut self) -> Result<(), Error> {
        let mut bin = vec![0u8; BLAKE2B_OUTBYTES];
        self.hasher.finalize(&mut bin);
        self.public_key.verify_ed25519(&bin, self.signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verify() {
        let public_key =
            PublicKey::from_base64("RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3")
                .expect("Unable to decode the public key");
        assert_eq!(public_key.untrusted_comment(), None);
        let signature = Signature::decode(
            "untrusted comment: signature from minisign secret key
RWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=
trusted comment: timestamp:1555779966\tfile:test
QtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==",
        )
        .expect("Unable to decode the signature");
        assert_eq!(
            signature.untrusted_comment(),
            "untrusted comment: signature from minisign secret key"
        );
        assert_eq!(
            signature.trusted_comment(),
            "timestamp:1555779966\tfile:test"
        );
        let bin = b"test";
        public_key
            .verify(&bin[..], &signature, true)
            .expect("Signature didn't verify");
        let bin = b"Test";
        match public_key.verify(&bin[..], &signature, true) {
            Err(Error::InvalidSignature) => {}
            _ => panic!("Invalid signature verified"),
        };

        let public_key2 = PublicKey::decode(
            "untrusted comment: minisign public key E7620F1842B4E81F
RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3",
        )
        .expect("Unable to decode the public key");
        assert_eq!(
            public_key2.untrusted_comment(),
            Some("untrusted comment: minisign public key E7620F1842B4E81F")
        );
        match public_key2.verify(&bin[..], &signature, true) {
            Err(Error::InvalidSignature) => {}
            _ => panic!("Invalid signature verified"),
        };
    }

    #[test]
    fn verify_prehashed() {
        let public_key =
            PublicKey::from_base64("RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3")
                .expect("Unable to decode the public key");
        assert_eq!(public_key.untrusted_comment(), None);
        let signature = Signature::decode(
            "untrusted comment: signature from minisign secret key
RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/\
             z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=
trusted comment: timestamp:1556193335\tfile:test
y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==",
        )
        .expect("Unable to decode the signature");
        assert_eq!(
            signature.untrusted_comment(),
            "untrusted comment: signature from minisign secret key"
        );
        assert_eq!(
            signature.trusted_comment(),
            "timestamp:1556193335\tfile:test"
        );
        let bin = b"test";
        public_key
            .verify(&bin[..], &signature, false)
            .expect("Signature didn't verify");
    }

    #[test]
    fn verify_stream() {
        let public_key =
            PublicKey::from_base64("RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3")
                .expect("Unable to decode the public key");
        assert_eq!(public_key.untrusted_comment(), None);
        let signature = Signature::decode(
            "untrusted comment: signature from minisign secret key
RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/\
             z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=
trusted comment: timestamp:1556193335\tfile:test
y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==",
        )
        .expect("Unable to decode the signature");
        assert_eq!(
            signature.untrusted_comment(),
            "untrusted comment: signature from minisign secret key"
        );
        assert_eq!(
            signature.trusted_comment(),
            "timestamp:1556193335\tfile:test"
        );
        let mut stream_verifier = public_key
            .verify_stream(&signature)
            .expect("Can't extract StreamerVerifier");

        let bin: &[u8] = b"te";
        stream_verifier.update(bin);

        let bin: &[u8] = b"st";
        stream_verifier.update(bin);

        stream_verifier.finalize().expect("Signature didn't verify");
    }
}
