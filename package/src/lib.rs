#![deny(clippy::all)]

use boilerstrip::{ConvertOptions, LearnOptions};
use napi::bindgen_prelude::{AsyncTask, Buffer, Either};
use napi::{Env, Result, Task};
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

fn to_bytes(input: Either<Buffer, String>) -> Vec<u8> {
    match input {
        Either::A(buf) => buf.to_vec(),
        Either::B(s) => s.into_bytes(),
    }
}

fn bytes_to_str(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes).map_err(|e| napi::Error::from_reason(e.to_string()))
}

fn do_learn(pages: &[Vec<u8>]) -> Result<boilerstrip::Removals> {
    let strings: Result<Vec<String>> = pages
        .iter()
        .map(|b| bytes_to_str(b).map(str::to_owned))
        .collect();
    boilerstrip::learn(&strings?, &LearnOptions::default())
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

fn do_convert(
    htmls: &[Vec<u8>],
    removals: Option<&boilerstrip::Removals>,
) -> Result<Vec<Vec<u8>>> {
    htmls
        .iter()
        .map(|bytes| {
            let html = bytes_to_str(bytes)?;
            let options = ConvertOptions {
                removals: removals.cloned(),
                ..Default::default()
            };
            boilerstrip::convert(html, &options)
                .map(|r| r.content.into_bytes())
                .map_err(|e| napi::Error::from_reason(e.to_string()))
        })
        .collect()
}

// ── Learn ─────────────────────────────────────────────────────────────────────

pub struct LearnTask(Vec<Vec<u8>>);

impl Task for LearnTask {
    type Output = boilerstrip::Removals;
    type JsValue = Removals;

    fn compute(&mut self) -> Result<Self::Output> {
        do_learn(&self.0)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(Removals::from(output))
    }
}

#[napi(ts_return_type = "Promise<Removals>")]
pub fn learn(pages: Vec<Either<Buffer, String>>) -> AsyncTask<LearnTask> {
    AsyncTask::new(LearnTask(pages.into_iter().map(to_bytes).collect()))
}

#[napi]
pub fn learn_sync(pages: Vec<Either<Buffer, String>>) -> Result<Removals> {
    do_learn(&pages.into_iter().map(to_bytes).collect::<Vec<_>>()).map(Removals::from)
}

// ── Convert ───────────────────────────────────────────────────────────────────

pub struct ConvertTask {
    htmls: Vec<Vec<u8>>,
    removals: Option<boilerstrip::Removals>,
}

impl Task for ConvertTask {
    type Output = Vec<Vec<u8>>;
    type JsValue = Vec<Buffer>;

    fn compute(&mut self) -> Result<Self::Output> {
        do_convert(&self.htmls, self.removals.as_ref())
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into_iter().map(Buffer::from).collect())
    }
}

#[napi(ts_return_type = "Promise<Buffer[]>")]
pub fn convert(
    htmls: Vec<Either<Buffer, String>>,
    removals: Option<Removals>,
) -> AsyncTask<ConvertTask> {
    AsyncTask::new(ConvertTask {
        htmls: htmls.into_iter().map(to_bytes).collect(),
        removals: removals.map(boilerstrip::Removals::from),
    })
}

#[napi]
pub fn convert_sync(
    htmls: Vec<Either<Buffer, String>>,
    removals: Option<Removals>,
) -> Result<Vec<Buffer>> {
    do_convert(
        &htmls.into_iter().map(to_bytes).collect::<Vec<_>>(),
        removals.map(boilerstrip::Removals::from).as_ref(),
    )
    .map(|vecs| vecs.into_iter().map(Buffer::from).collect())
}
