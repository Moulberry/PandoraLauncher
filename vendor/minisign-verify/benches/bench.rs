#![feature(test)]

extern crate minisign_verify;
extern crate test;

use minisign_verify::{PublicKey, Signature};
use test::Bencher;

static PUBLIC_KEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
static SIGNATURE: &str = "untrusted comment: signature from minisign secret key
RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/\
z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=
trusted comment: timestamp:1556193335\tfile:test
y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==";

#[bench]
fn bench_public_key_from_base64(b: &mut Bencher) {
    b.iter(|| {
        let _public_key = PublicKey::from_base64(PUBLIC_KEY).unwrap();
    });
}

#[bench]
fn bench_signature_decode(b: &mut Bencher) {
    b.iter(|| {
        let _signature = Signature::decode(SIGNATURE).unwrap();
    });
}

#[bench]
fn bench_verify_small(b: &mut Bencher) {
    let public_key = PublicKey::from_base64(PUBLIC_KEY).unwrap();
    let signature = Signature::decode(SIGNATURE).unwrap();
    let data = b"test";

    b.iter(|| {
        public_key.verify(data, &signature, false).unwrap();
    });
}

#[bench]
fn bench_verify_medium(b: &mut Bencher) {
    let public_key = PublicKey::from_base64(PUBLIC_KEY).unwrap();
    let signature = Signature::decode(SIGNATURE).unwrap();
    let data = vec![0u8; 1024 * 10]; // 10KB

    b.iter(|| {
        public_key.verify(&data, &signature, false).unwrap();
    });
}

#[bench]
fn bench_verify_large_streaming(b: &mut Bencher) {
    let public_key = PublicKey::from_base64(PUBLIC_KEY).unwrap();
    let signature = Signature::decode(SIGNATURE).unwrap();
    let data = vec![0u8; 1024 * 100]; // 100KB

    b.iter(|| {
        let mut verifier = public_key.verify_stream(&signature).unwrap();

        // Process in chunks
        let chunk_size = 8192;
        let mut i = 0;
        while i < data.len() {
            let end = std::cmp::min(i + chunk_size, data.len());
            verifier.update(&data[i..end]);
            i += chunk_size;
        }

        verifier.finalize().unwrap();
    });
}

#[bench]
fn bench_verify_large_direct(b: &mut Bencher) {
    let public_key = PublicKey::from_base64(PUBLIC_KEY).unwrap();
    let signature = Signature::decode(SIGNATURE).unwrap();
    let data = vec![0u8; 1024 * 100]; // 100KB

    b.iter(|| {
        public_key.verify(&data, &signature, false).unwrap();
    });
}
