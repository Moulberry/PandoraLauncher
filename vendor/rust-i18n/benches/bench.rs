use rust_i18n::t;

rust_i18n::i18n!("./tests/locales");

use criterion::{criterion_group, criterion_main, Criterion};

lazy_static::lazy_static! {
pub static ref DICT: std::collections::HashMap<&'static str, &'static str> =
    [
        ("hello", "Bar - Hello, World!"),
    ].iter().cloned().collect();
}

fn bench_t(c: &mut Criterion) {
    // 102 ns
    c.bench_function("t", |b| b.iter(|| t!("hello")));

    c.bench_function("t_with_locale", |b| b.iter(|| t!("hello", locale = "en")));

    c.bench_function("t_with_threads", |b| {
        let exit_loop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let exit_loop = exit_loop.clone();
            handles.push(std::thread::spawn(move || {
                while !exit_loop.load(std::sync::atomic::Ordering::SeqCst) {
                    criterion::black_box(t!("hello"));
                }
            }));
        }
        b.iter(|| t!("hello"));
        exit_loop.store(true, std::sync::atomic::Ordering::SeqCst);
        for handle in handles {
            handle.join().unwrap();
        }
    });

    c.bench_function("t_lorem_ipsum", |b| b.iter(|| t!("lorem-ipsum")));

    // 73.239 ns
    c.bench_function("_rust_i18n_translate", |b| {
        b.iter(|| crate::_rust_i18n_translate("en", "hello"))
    });

    // 54.221 ns
    c.bench_function("_RUST_I18N_BACKEND.translate", |b| {
        b.iter(|| crate::_RUST_I18N_BACKEND.translate("en", "hello"))
    });

    // 46.721
    c.bench_function("static_hashmap_get_to_string", |b| {
        b.iter(|| DICT.get("hello").unwrap().to_string())
    });

    // 20.023 ns
    c.bench_function("static_hashmap_get_as_static_str", |b| {
        b.iter(|| DICT.get("hello").unwrap())
    });

    c.bench_function("t_with_args", |b| {
        b.iter(|| t!("a.very.nested.message", name = "Jason", msg = "Bla bla"))
    });

    c.bench_function("t_with_args (str)", |b| {
        b.iter(|| t!("a.very.nested.message", "name" = "Jason", "msg" = "Bla bla"))
    });

    c.bench_function("t_with_args (many)", |b| {
        b.iter(|| {
            t!(
                "a.very.nested.response",
                id = 123,
                name = "Marion",
                surname = "Christiansen",
                email = "Marion_Christiansen83@hotmail.com",
                city = "Litteltown",
                zip = 8408,
                website = "https://snoopy-napkin.name"
            )
        })
    });
}

criterion_group!(benches, bench_t);
criterion_main!(benches);
