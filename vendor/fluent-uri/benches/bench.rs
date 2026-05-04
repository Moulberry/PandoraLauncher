use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fluent_uri::{
    enc::{table::*, *},
    *,
};
use iri_string::types::UriReferenceStr;
use uriparse::URIReference;
use url::Url;

criterion_group!(
    benches,
    bench_enc,
    bench_dec,
    bench_dec_unchecked,
    bench_dec_in_place,
    bench_validate,
    bench_parse,
    bench_parse_url,
    bench_parse_uriparse,
    bench_parse_iri_string,
);
criterion_main!(benches);

const ENC_CASE: &str = "te😃a 测1`~!@试#$%st^&+=";

fn bench_enc(c: &mut Criterion) {
    c.bench_function("enc", |b| {
        b.iter(|| encode(black_box(ENC_CASE), QUERY_FRAGMENT))
    });
}

const DEC_CASE: &str = "te%F0%9F%98%83a%20%E6%B5%8B1%60~!@%E8%AF%95%23$%25st%5E&+=";

fn bench_dec(c: &mut Criterion) {
    c.bench_function("dec", |b| b.iter(|| decode(black_box(DEC_CASE))));
}

fn bench_dec_unchecked(c: &mut Criterion) {
    c.bench_function("dec_unchecked", |b| {
        b.iter(|| unsafe {
            decode_unchecked(black_box(DEC_CASE.as_bytes()));
        })
    });
}

fn bench_dec_in_place(c: &mut Criterion) {
    let mut vec = DEC_CASE.as_bytes().to_vec();
    c.bench_function("dec_in_place", |b| {
        b.iter(|| unsafe {
            decode_in_place_unchecked(&mut vec);
            vec.copy_from_slice(DEC_CASE.as_bytes());
        })
    });
}

fn bench_validate(c: &mut Criterion) {
    c.bench_function("validate", |b| {
        b.iter(|| validate(black_box(DEC_CASE), QUERY_FRAGMENT))
    });
}

const PARSE_CASE: &str = "https://user@example.com/search?q=%E6%B5%8B%E8%AF%95#fragment";

fn bench_parse(c: &mut Criterion) {
    c.bench_function("parse", |b| b.iter(|| Uri::parse(black_box(PARSE_CASE))));
}

fn bench_parse_url(c: &mut Criterion) {
    c.bench_function("parse_url", |b| {
        b.iter(|| Url::parse(black_box(PARSE_CASE)))
    });
}

fn bench_parse_uriparse(c: &mut Criterion) {
    c.bench_function("parse_uriparse", |b| {
        b.iter(|| URIReference::try_from(black_box(PARSE_CASE)))
    });
}

fn bench_parse_iri_string(c: &mut Criterion) {
    c.bench_function("parse_iri_string", |b| {
        b.iter(|| <&UriReferenceStr>::try_from(black_box(PARSE_CASE)))
    });
}
