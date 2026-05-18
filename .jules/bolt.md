## 2023-10-27 - Rust Iterators vs Single Pass Allocations
**Learning:** In Rust codebases heavy on string manipulation (like boilerplate extractors), chaining `.split_whitespace().collect::<Vec<_>>().join(" ")` creates multiple expensive intermediate allocations and prevents short-circuiting.
**Action:** When extracting short, bounded text summaries (like fingerprinting), prefer single-pass state-machine approaches over chained iterator collections to skip unnecessary work.
