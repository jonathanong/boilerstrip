use boilerstrip::{convert, ConvertOptions};
use criterion::{criterion_group, criterion_main, Criterion};
use std::path::Path;

fn fixture(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("fixtures");
    std::fs::read_to_string(root.join(path))
        .unwrap_or_else(|_| "<html><body><p>placeholder</p></body></html>".to_string())
}

fn bench_convert(c: &mut Criterion) {
    let basic = fixture("convert/basic_article.html");
    c.bench_function("convert/basic_article", |b| {
        b.iter(|| convert(&basic, &ConvertOptions::default()))
    });

    let with_meta = fixture("convert/with_meta.html");
    c.bench_function("convert/with_meta", |b| {
        b.iter(|| convert(&with_meta, &ConvertOptions::default()))
    });

    let tables = fixture("convert/tables_and_lists.html");
    c.bench_function("convert/tables_and_lists", |b| {
        b.iter(|| convert(&tables, &ConvertOptions::default()))
    });

    // Bench fixtures from the bench/ directory when they exist
    for entry in std::fs::read_dir(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("fixtures/bench"),
    )
    .into_iter()
    .flatten()
    .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("html") {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let html = std::fs::read_to_string(&path).unwrap();
            c.bench_function(&format!("convert/bench/{name}"), |b| {
                b.iter(|| convert(&html, &ConvertOptions::default()))
            });
        }
    }
}

fn bench_convert_density(c: &mut Criterion) {
    let basic = fixture("convert/basic_article.html");
    let opts = ConvertOptions {
        use_text_density_filter: true,
        ..Default::default()
    };
    c.bench_function("convert/basic_article_density", |b| {
        b.iter(|| convert(&basic, &opts))
    });
}

criterion_group!(benches, bench_convert, bench_convert_density);
criterion_main!(benches);
