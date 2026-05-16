#![deny(clippy::all)]
#![forbid(unsafe_code)]

use boilerstrip::{ConvertOptions as RustConvertOptions, LearnOptions as RustLearnOptions};
use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi::{Env, Task};
use napi_derive::napi;
use rayon::prelude::*;
use serde_json::Value;

#[napi(object)]
pub struct Removals {
    pub css_selectors_to_remove: Vec<String>,
    pub html_to_remove: Vec<String>,
}

impl From<boilerstrip::Removals> for Removals {
    fn from(r: boilerstrip::Removals) -> Self {
        Self {
            css_selectors_to_remove: r.css_selectors_to_remove,
            html_to_remove: r.html_to_remove,
        }
    }
}

impl From<Removals> for boilerstrip::Removals {
    fn from(r: Removals) -> Self {
        Self {
            css_selectors_to_remove: r.css_selectors_to_remove,
            html_to_remove: r.html_to_remove,
        }
    }
}

#[napi(object)]
pub struct LearnOptions {
    /// Text patterns (case-insensitive) that suggest boilerplate content.
    /// Pass an empty array to disable pattern matching.
    /// Omit (null/undefined) to use the built-in defaults.
    pub boilerplate_patterns: Option<Vec<String>>,
    /// Maximum times a selector can match per page before it is considered too broad.
    /// Defaults to `20`.
    pub max_selector_matches_per_page: Option<u32>,
    /// Minimum average stable-match ratio across all pages. Defaults to `0.6`.
    pub min_selector_average_stable_ratio: Option<f64>,
    /// Minimum per-page stable-match ratio. Defaults to `0.35`.
    pub min_selector_per_page_stable_ratio: Option<f64>,
    /// Minimum text length for a snippet to qualify as boilerplate. Defaults to `40`.
    pub min_snippet_text_length: Option<u32>,
    /// Maximum text length for a snippet to qualify as boilerplate. Defaults to `240`.
    pub max_snippet_text_length: Option<u32>,
}

impl From<LearnOptions> for RustLearnOptions {
    fn from(o: LearnOptions) -> Self {
        Self {
            boilerplate_patterns: o.boilerplate_patterns,
            max_selector_matches_per_page: o.max_selector_matches_per_page.map(|v| v as usize),
            min_selector_average_stable_ratio: o.min_selector_average_stable_ratio,
            min_selector_per_page_stable_ratio: o.min_selector_per_page_stable_ratio,
            min_snippet_text_length: o.min_snippet_text_length.map(|v| v as usize),
            max_snippet_text_length: o.max_snippet_text_length.map(|v| v as usize),
        }
    }
}

#[napi(object)]
pub struct ConvertOptions {
    /// Boilerplate removals learned from a set of pages; applied before conversion.
    pub removals: Option<Removals>,
    /// CSS selectors whose matching elements are removed before conversion.
    pub css_selectors_to_remove: Option<Vec<String>>,
    /// CSS selectors that identify the main content root (first match wins).
    pub content_selectors: Option<Vec<String>>,
    /// Link visible-text patterns whose matching `<a>`/`<button>` elements are removed.
    pub link_text_content_to_remove: Option<Vec<String>>,
    /// Link href prefixes whose matching elements are removed (e.g. `"javascript:"`).
    pub link_hrefs_to_remove: Option<Vec<String>>,
    /// `<link rel="...">` tokens to exclude from the extracted `link` map.
    pub link_rel_tokens_to_remove: Option<Vec<String>>,
    /// When `true`, use text-density scoring to locate the main content element.
    pub use_text_density_filter: Option<bool>,
}

impl From<ConvertOptions> for RustConvertOptions {
    fn from(o: ConvertOptions) -> Self {
        Self {
            removals: o.removals.map(boilerstrip::Removals::from),
            css_selectors_to_remove: o.css_selectors_to_remove.unwrap_or_default(),
            content_selectors: o.content_selectors.unwrap_or_default(),
            link_text_content_to_remove: o.link_text_content_to_remove.unwrap_or_default(),
            link_hrefs_to_remove: o.link_hrefs_to_remove.unwrap_or_default(),
            link_rel_tokens_to_remove: o.link_rel_tokens_to_remove.unwrap_or_default(),
            use_text_density_filter: o.use_text_density_filter.unwrap_or(false),
        }
    }
}

#[napi(object)]
pub struct ConvertResult {
    /// Page title from `<title>`.
    pub title: Option<String>,
    /// `<meta name/property>` map as a plain object.
    pub meta: Value,
    /// `<link rel>` map as a plain object.
    pub link: Value,
    /// Cleaned Markdown content.
    pub content: String,
    /// Canonical URL from `<link rel="canonical">`.
    pub canonical_url: Option<String>,
    /// Language code from `<html lang="...">`.
    pub lang: Option<String>,
}

impl From<boilerstrip::ConvertResult> for ConvertResult {
    fn from(r: boilerstrip::ConvertResult) -> Self {
        Self {
            title: r.title,
            meta: Value::Object(r.meta),
            link: Value::Object(r.link),
            content: r.content,
            canonical_url: r.canonical_url,
            lang: r.lang,
        }
    }
}

fn panic_message(payload: &Box<dyn std::any::Any + Send + 'static>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

// ── learn ─────────────────────────────────────────────────────────────────────

pub struct LearnTask {
    pages: Vec<Buffer>,
    options: RustLearnOptions,
}

impl Task for LearnTask {
    type Output = boilerstrip::Removals;
    type JsValue = Removals;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let pages = self
                .pages
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    std::str::from_utf8(b)
                        .map(str::to_owned)
                        .map_err(|e| napi::Error::from_reason(format!("learn[{i}]: {e}")))
                })
                .collect::<napi::Result<Vec<_>>>()?;
            boilerstrip::learn(&pages, &self.options)
                .map_err(|e| napi::Error::from_reason(e.to_string()))
        }))
        .map_err(|payload| {
            napi::Error::from_reason(format!("boilerstrip panic: {}", panic_message(&payload)))
        })?
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(Removals::from(output))
    }
}

#[napi(ts_return_type = "Promise<Removals>")]
pub fn learn(pages: Vec<Buffer>, options: Option<LearnOptions>) -> AsyncTask<LearnTask> {
    let rust_options = options.map(RustLearnOptions::from).unwrap_or_default();
    AsyncTask::new(LearnTask {
        pages,
        options: rust_options,
    })
}

// ── convert ───────────────────────────────────────────────────────────────────

pub struct ConvertTask {
    html: Buffer,
    options: RustConvertOptions,
}

impl Task for ConvertTask {
    type Output = ConvertResult;
    type JsValue = ConvertResult;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let html = std::str::from_utf8(&self.html)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
            Ok(ConvertResult::from(boilerstrip::convert(
                html,
                &self.options,
            )))
        }))
        .map_err(|payload| {
            napi::Error::from_reason(format!("boilerstrip panic: {}", panic_message(&payload)))
        })?
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi(ts_return_type = "Promise<ConvertResult>")]
pub fn convert(html: Buffer, options: Option<ConvertOptions>) -> AsyncTask<ConvertTask> {
    let rust_options = options.map(RustConvertOptions::from).unwrap_or_default();
    AsyncTask::new(ConvertTask {
        html,
        options: rust_options,
    })
}

// ── convertMany ───────────────────────────────────────────────────────────────

pub struct ConvertManyTask {
    htmls: Vec<Buffer>,
    options: RustConvertOptions,
}

impl Task for ConvertManyTask {
    type Output = Vec<ConvertResult>;
    type JsValue = Vec<ConvertResult>;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.htmls
                .par_iter()
                .enumerate()
                .map(|(i, buf)| {
                    let html = std::str::from_utf8(buf)
                        .map_err(|e| napi::Error::from_reason(format!("convertMany[{i}]: {e}")))?;
                    Ok(ConvertResult::from(boilerstrip::convert(
                        html,
                        &self.options,
                    )))
                })
                .collect()
        }))
        .map_err(|payload| {
            napi::Error::from_reason(format!("boilerstrip panic: {}", panic_message(&payload)))
        })?
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi(ts_return_type = "Promise<ConvertResult[]>")]
pub fn convert_many(
    htmls: Vec<Buffer>,
    options: Option<ConvertOptions>,
) -> AsyncTask<ConvertManyTask> {
    let rust_options = options.map(RustConvertOptions::from).unwrap_or_default();
    AsyncTask::new(ConvertManyTask {
        htmls,
        options: rust_options,
    })
}
