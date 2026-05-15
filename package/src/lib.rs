#![deny(clippy::all)]

use boilerstrip::{ConvertOptions, LearnOptions};
use napi_derive::napi;

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

#[napi]
pub fn learn(pages: Vec<String>) -> napi::Result<Removals> {
    boilerstrip::learn(&pages, &LearnOptions::default())
        .map(Removals::from)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn convert(html: String, removals: Option<Removals>) -> napi::Result<String> {
    let options = ConvertOptions {
        removals: removals.map(boilerstrip::Removals::from),
        ..Default::default()
    };
    boilerstrip::convert(&html, &options)
        .map(|result| result.content)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}
