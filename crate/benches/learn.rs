use boilerstrip::{learn, LearnOptions};
use criterion::{criterion_group, criterion_main, Criterion};
use std::path::Path;

fn read_dir_htmls(dir: &Path) -> Vec<String> {
    let mut pages: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("html"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    pages.sort(); // deterministic order
    pages
}

fn bench_learn(c: &mut Criterion) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("fixtures");

    let site_a = read_dir_htmls(&root.join("learn/site_a"));
    if site_a.len() >= 2 {
        c.bench_function("learn/site_a", |b| {
            b.iter(|| learn(&site_a, &LearnOptions::default()))
        });
    }

    let site_b = read_dir_htmls(&root.join("learn/site_b"));
    if site_b.len() >= 2 {
        c.bench_function("learn/site_b", |b| {
            b.iter(|| learn(&site_b, &LearnOptions::default()))
        });
    }

    // Larger bench fixtures if they exist
    let bench_dir = root.join("bench");
    if bench_dir.exists() {
        let bench_pages = read_dir_htmls(&bench_dir);
        if bench_pages.len() >= 2 {
            c.bench_function("learn/bench_corpus", |b| {
                b.iter(|| learn(&bench_pages, &LearnOptions::default()))
            });
        }
    }
}

criterion_group!(benches, bench_learn);
criterion_main!(benches);
