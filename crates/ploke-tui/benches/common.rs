#![allow(
    dead_code,
    unused_variables,
    reason = "evolving api surface, may be useful, written 2025-12-15"
)]

use lasso::{Spur, ThreadedRodeo};
use once_cell::sync::Lazy;
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use std::fmt::Write as _;

// encode `:` only (OpenRouter models path needs %3A)
pub const PATH_ENCODE: &AsciiSet = &CONTROLS.add(b':');
pub const URL_PREFIX: &str = "https://openrouter.ai/api/v1/models/";
pub const URL_SUFFIX: &str = "/endpoints";

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Symbol(Spur);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct DIntern {
    author: Symbol,
    slug: Symbol,
    variant: Option<Symbol>,
}

impl DIntern {
    pub fn parse(rodeo: &ThreadedRodeo, s: &str) -> Option<Self> {
        let (author, rest) = s.split_once('/')?;
        let (slug, variant) = match rest.split_once(':') {
            Some((slug, v)) => (slug, Some(v)),
            None => (rest, None),
        };
        if slug.is_empty() {
            return None;
        }
        let a = Symbol(rodeo.get_or_intern(author));
        let g = Symbol(rodeo.get_or_intern(slug));
        let v = variant.map(|v| Symbol(rodeo.get_or_intern(v)));
        Some(Self {
            author: a,
            slug: g,
            variant: v,
        })
    }

    pub fn endpoints_url(&self, rodeo: &ThreadedRodeo) -> String {
        let a = rodeo.resolve(&self.author.0);
        let g = rodeo.resolve(&self.slug.0);
        let mut id = String::with_capacity(a.len() + 1 + g.len() + 16);
        id.push_str(a);
        id.push('/');
        id.push_str(g);
        if let Some(v) = self.variant {
            id.push(':');
            id.push_str(rodeo.resolve(&v.0));
        }
        // Baseline approach using percent-encoding + format!
        format!(
            "{}{}{}",
            URL_PREFIX,
            utf8_percent_encode(&id, PATH_ENCODE),
            URL_SUFFIX
        )
    }

    // Alternative 1: direct concat with known encoding (encode only ':')
    // Avoids building an intermediate ID string and avoids format! macro.
    pub fn endpoints_url_concat_known(&self, rodeo: &ThreadedRodeo) -> String {
        let a = rodeo.resolve(&self.author.0);
        let g = rodeo.resolve(&self.slug.0);
        // allocate exactly: prefix + a + '/' + g + ("%3A"+v | nothing) + suffix
        let var = self.variant.map(|v| rodeo.resolve(&v.0));
        let extra = if var.is_some() {
            3 /*"%3A"*/
        } else {
            0
        };
        let cap = URL_PREFIX.len()
            + a.len()
            + 1
            + g.len()
            + extra
            + var.as_ref().map(|v| v.len()).unwrap_or(0)
            + URL_SUFFIX.len();
        let mut url = String::with_capacity(cap);
        url.push_str(URL_PREFIX);
        url.push_str(a);
        url.push('/');
        url.push_str(g);
        if let Some(v) = var {
            url.push_str("%3A");
            url.push_str(v);
        }
        url.push_str(URL_SUFFIX);
        url
    }

    // Alternative 2: preallocated + write! percent-encoder into buffer (no format!)
    pub fn endpoints_url_prealloc_write(&self, rodeo: &ThreadedRodeo) -> String {
        let a = rodeo.resolve(&self.author.0);
        let g = rodeo.resolve(&self.slug.0);
        let var = self.variant.map(|v| rodeo.resolve(&v.0));
        let id_len = a.len() + 1 + g.len() + var.as_ref().map(|v| 1 + v.len()).unwrap_or(0);
        let encoded_growth = if var.is_some() { 2 } else { 0 }; // ':' -> "%3A"
        let mut out =
            String::with_capacity(URL_PREFIX.len() + id_len + encoded_growth + URL_SUFFIX.len());
        out.push_str(URL_PREFIX);
        let mut tmp = String::with_capacity(id_len);
        tmp.push_str(a);
        tmp.push('/');
        tmp.push_str(g);
        if let Some(v) = var {
            tmp.push(':');
            tmp.push_str(v);
        }
        // write percent-encoded id directly into `out`
        write!(&mut out, "{}", utf8_percent_encode(&tmp, PATH_ENCODE)).unwrap();
        out.push_str(URL_SUFFIX);
        out
    }

    // Alternative 3: simple replace for ':' on a temporary id buffer
    pub fn endpoints_url_replace(&self, rodeo: &ThreadedRodeo) -> String {
        let a = rodeo.resolve(&self.author.0);
        let g = rodeo.resolve(&self.slug.0);
        let var = self.variant.map(|v| rodeo.resolve(&v.0));
        let mut id = String::with_capacity(
            a.len() + 1 + g.len() + var.as_ref().map(|v| 1 + v.len()).unwrap_or(0),
        );
        id.push_str(a);
        id.push('/');
        id.push_str(g);
        if let Some(v) = var {
            id.push(':');
            id.push_str(v);
        }
        let mut out = String::with_capacity(URL_PREFIX.len() + id.len() + 2 + URL_SUFFIX.len());
        out.push_str(URL_PREFIX);
        out.push_str(&id.replace(":", "%3A"));
        out.push_str(URL_SUFFIX);
        out
    }
}

pub static GLOBAL_RODEO: Lazy<ThreadedRodeo> = Lazy::new(ThreadedRodeo::default);

pub fn preseed_global_rodeo(inputs: &[String]) {
    // Collect unique tokens first to avoid repeated locking.
    let mut toks = std::collections::BTreeSet::new();
    for s in inputs {
        if let Some((a, rest)) = s.split_once('/') {
            toks.insert(a);
            if let Some((g, v)) = rest.split_once(':') {
                toks.insert(g);
                toks.insert(v);
            } else {
                toks.insert(rest);
            }
        }
    }
    for t in toks {
        let _ = GLOBAL_RODEO.get_or_intern(t);
    }
}

// ---------- dataset helpers ----------
pub fn gen_unique_inputs(n: usize) -> Vec<String> {
    // n distinct IDs
    (0..n)
        .map(|i| format!("author{}/model{}:free", i % 10_003, i))
        .collect()
}

pub fn gen_repeated_inputs(unique: usize, factor: usize) -> Vec<String> {
    // `unique` distinct, each repeated `factor` times (total = unique * factor)
    let base: Vec<String> = (0..unique)
        .map(|i| format!("author{}/model{}:free", i % 1_003, i))
        .collect();
    base.into_iter()
        .flat_map(|s| std::iter::repeat_n(s, factor))
        .collect()
}
