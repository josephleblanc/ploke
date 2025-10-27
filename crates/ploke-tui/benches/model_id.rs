use arrayvec::ArrayString;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use lasso::{Spur, ThreadedRodeo};
use once_cell::sync::Lazy;
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use std::fmt::Write as _;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::hint::black_box;

static GLOBAL_RODEO: Lazy<ThreadedRodeo> = Lazy::new(ThreadedRodeo::default);

fn preseed_global_rodeo(inputs: &[String]) {
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
fn gen_unique_inputs(n: usize) -> Vec<String> {
    // n distinct IDs
    (0..n)
        .map(|i| format!("author{}/model{}:free", i % 10_003, i))
        .collect()
}

fn gen_repeated_inputs(unique: usize, factor: usize) -> Vec<String> {
    // `unique` distinct, each repeated `factor` times (total = unique * factor)
    let base: Vec<String> = (0..unique)
        .map(|i| format!("author{}/model{}:free", i % 1_003, i))
        .collect();
    base.into_iter()
        .flat_map(|s| std::iter::repeat(s).take(factor))
        .collect()
}

// encode `:` only (OpenRouter models path needs %3A)
const PATH_ENCODE: &AsciiSet = &CONTROLS.add(b':');
const URL_PREFIX: &str = "https://openrouter.ai/api/v1/models/";
const URL_SUFFIX: &str = "/endpoints";

// ---------- Strategy A: Plain String ----------

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
struct AString {
    author: String,
    slug: String,
    variant: Option<String>,
}

impl AString {
    fn parse(s: &str) -> Option<Self> {
        let (author, rest) = s.split_once('/')?;
        let (slug, variant) = match rest.split_once(':') {
            Some((slug, v)) => (slug, Some(v)),
            None => (rest, None),
        };
        if slug.is_empty() {
            return None;
        }
        Some(Self {
            author: author.to_owned(),
            slug: slug.to_owned(),
            variant: variant.map(str::to_owned),
        })
    }

    fn endpoints_url(&self) -> String {
        // https://openrouter.ai/api/v1/models/{id_encoded}/endpoints
        let mut id = String::with_capacity(
            self.author.len()
                + 1
                + self.slug.len()
                + self.variant.as_ref().map(|v| 1 + v.len()).unwrap_or(0),
        );
        id.push_str(&self.author);
        id.push('/');
        id.push_str(&self.slug);
        if let Some(v) = &self.variant {
            id.push(':');
            id.push_str(v);
        }
        format!("{}{}{}", URL_PREFIX, utf8_percent_encode(&id, PATH_ENCODE), URL_SUFFIX)
    }

    // Alternative 1: concat with known encoding (encode only ':')
    fn endpoints_url_concat_known(&self) -> String {
        let var = self.variant.as_deref();
        let extra = if var.is_some() { 3 } else { 0 };
        let mut url = String::with_capacity(
            URL_PREFIX.len()
                + self.author.len()
                + 1
                + self.slug.len()
                + extra
                + var.map(|v| v.len()).unwrap_or(0)
                + URL_SUFFIX.len(),
        );
        url.push_str(URL_PREFIX);
        url.push_str(&self.author);
        url.push('/');
        url.push_str(&self.slug);
        if let Some(v) = var {
            url.push_str("%3A");
            url.push_str(v);
        }
        url.push_str(URL_SUFFIX);
        url
    }

    // Alternative 2: prealloc + write! percent-encoder (avoid format!)
    fn endpoints_url_prealloc_write(&self) -> String {
        let id_len = self.author.len() + 1 + self.slug.len() + self.variant.as_ref().map(|v| 1 + v.len()).unwrap_or(0);
        let encoded_growth = if self.variant.is_some() { 2 } else { 0 };
        let mut out = String::with_capacity(URL_PREFIX.len() + id_len + encoded_growth + URL_SUFFIX.len());
        out.push_str(URL_PREFIX);
        let mut tmp = String::with_capacity(id_len);
        tmp.push_str(&self.author);
        tmp.push('/');
        tmp.push_str(&self.slug);
        if let Some(v) = &self.variant {
            tmp.push(':');
            tmp.push_str(v);
        }
        write!(&mut out, "{}", utf8_percent_encode(&tmp, PATH_ENCODE)).unwrap();
        out.push_str(URL_SUFFIX);
        out
    }

    // Alternative 3: simple replace for ':'
    fn endpoints_url_replace(&self) -> String {
        let mut id = String::with_capacity(
            self.author.len()
                + 1
                + self.slug.len()
                + self.variant.as_ref().map(|v| 1 + v.len()).unwrap_or(0),
        );
        id.push_str(&self.author);
        id.push('/');
        id.push_str(&self.slug);
        if let Some(v) = &self.variant {
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

// ---------- Strategy B: SmolStr (inline small strings) ----------

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
struct BSmol {
    author: SmolStr,
    slug: SmolStr,
    variant: Option<SmolStr>,
}

impl BSmol {
    fn parse(s: &str) -> Option<Self> {
        let (author, rest) = s.split_once('/')?;
        let (slug, variant) = match rest.split_once(':') {
            Some((slug, v)) => (slug, Some(v)),
            None => (rest, None),
        };
        if slug.is_empty() {
            return None;
        }
        Some(Self {
            author: SmolStr::new(author),
            slug: SmolStr::new(slug),
            variant: variant.map(SmolStr::new),
        })
    }

    fn endpoints_url(&self) -> String {
        let mut id = String::with_capacity(
            self.author.len()
                + 1
                + self.slug.len()
                + self.variant.as_ref().map(|v| 1 + v.len()).unwrap_or(0),
        );
        id.push_str(self.author.as_str());
        id.push('/');
        id.push_str(self.slug.as_str());
        if let Some(v) = &self.variant {
            id.push(':');
            id.push_str(v.as_str());
        }
        format!("{}{}{}", URL_PREFIX, utf8_percent_encode(&id, PATH_ENCODE), URL_SUFFIX)
    }

    fn endpoints_url_concat_known(&self) -> String {
        let var = self.variant.as_deref();
        let extra = if var.is_some() { 3 } else { 0 };
        let mut url = String::with_capacity(
            URL_PREFIX.len()
                + self.author.len()
                + 1
                + self.slug.len()
                + extra
                + var.map(|v| v.len()).unwrap_or(0)
                + URL_SUFFIX.len(),
        );
        url.push_str(URL_PREFIX);
        url.push_str(self.author.as_str());
        url.push('/');
        url.push_str(self.slug.as_str());
        if let Some(v) = var {
            url.push_str("%3A");
            url.push_str(v);
        }
        url.push_str(URL_SUFFIX);
        url
    }

    fn endpoints_url_prealloc_write(&self) -> String {
        let id_len = self.author.len() + 1 + self.slug.len() + self.variant.as_ref().map(|v| 1 + v.len()).unwrap_or(0);
        let encoded_growth = if self.variant.is_some() { 2 } else { 0 };
        let mut out = String::with_capacity(URL_PREFIX.len() + id_len + encoded_growth + URL_SUFFIX.len());
        out.push_str(URL_PREFIX);
        let mut tmp = String::with_capacity(id_len);
        tmp.push_str(self.author.as_str());
        tmp.push('/');
        tmp.push_str(self.slug.as_str());
        if let Some(v) = &self.variant {
            tmp.push(':');
            tmp.push_str(v.as_str());
        }
        write!(&mut out, "{}", utf8_percent_encode(&tmp, PATH_ENCODE)).unwrap();
        out.push_str(URL_SUFFIX);
        out
    }

    fn endpoints_url_replace(&self) -> String {
        let mut id = String::with_capacity(
            self.author.len()
                + 1
                + self.slug.len()
                + self.variant.as_ref().map(|v| 1 + v.len()).unwrap_or(0),
        );
        id.push_str(self.author.as_str());
        id.push('/');
        id.push_str(self.slug.as_str());
        if let Some(v) = &self.variant {
            id.push(':');
            id.push_str(v.as_str());
        }
        let mut out = String::with_capacity(URL_PREFIX.len() + id.len() + 2 + URL_SUFFIX.len());
        out.push_str(URL_PREFIX);
        out.push_str(&id.replace(":", "%3A"));
        out.push_str(URL_SUFFIX);
        out
    }
}

// ---------- Strategy C: ArrayString<N> (no heap if within bound) ----------

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
struct CArray {
    author: ArrayString<32>,
    slug: ArrayString<64>,
    variant: Option<ArrayString<32>>,
}

impl CArray {
    fn parse(s: &str) -> Option<Self> {
        let (author, rest) = s.split_once('/')?;
        let (slug, variant) = match rest.split_once(':') {
            Some((slug, v)) => (slug, Some(v)),
            None => (rest, None),
        };
        if slug.is_empty() {
            return None;
        }
        let mut a = ArrayString::<32>::new();
        a.push_str(author);
        let mut g = ArrayString::<64>::new();
        g.push_str(slug);
        let var = if let Some(v) = variant {
            let mut vv = ArrayString::<32>::new();
            vv.push_str(v);
            Some(vv)
        } else {
            None
        };
        Some(Self {
            author: a,
            slug: g,
            variant: var,
        })
    }

    fn endpoints_url(&self) -> String {
        // Build into a stack buffer, then percent encode (allocs for result only)
        let mut id = ArrayString::<128>::new();
        id.push_str(self.author.as_str());
        id.push('/');
        id.push_str(self.slug.as_str());
        if let Some(v) = &self.variant {
            id.push(':');
            id.push_str(v.as_str());
        }
        format!("{}{}{}", URL_PREFIX, utf8_percent_encode(id.as_str(), PATH_ENCODE), URL_SUFFIX)
    }

    fn endpoints_url_concat_known(&self) -> String {
        let var = self.variant.as_ref().map(|v| v.as_str());
        let extra = if var.is_some() { 3 } else { 0 };
        let mut url = String::with_capacity(
            URL_PREFIX.len()
                + self.author.len()
                + 1
                + self.slug.len()
                + extra
                + var.map(|v| v.len()).unwrap_or(0)
                + URL_SUFFIX.len(),
        );
        url.push_str(URL_PREFIX);
        url.push_str(self.author.as_str());
        url.push('/');
        url.push_str(self.slug.as_str());
        if let Some(v) = var {
            url.push_str("%3A");
            url.push_str(v);
        }
        url.push_str(URL_SUFFIX);
        url
    }

    fn endpoints_url_prealloc_write(&self) -> String {
        let id_len = self.author.len() + 1 + self.slug.len() + self.variant.as_ref().map(|v| 1 + v.len()).unwrap_or(0);
        let encoded_growth = if self.variant.is_some() { 2 } else { 0 };
        let mut out = String::with_capacity(URL_PREFIX.len() + id_len + encoded_growth + URL_SUFFIX.len());
        out.push_str(URL_PREFIX);
        let mut tmp = String::with_capacity(id_len);
        tmp.push_str(self.author.as_str());
        tmp.push('/');
        tmp.push_str(self.slug.as_str());
        if let Some(v) = &self.variant {
            tmp.push(':');
            tmp.push_str(v.as_str());
        }
        write!(&mut out, "{}", utf8_percent_encode(&tmp, PATH_ENCODE)).unwrap();
        out.push_str(URL_SUFFIX);
        out
    }

    fn endpoints_url_replace(&self) -> String {
        let mut id = ArrayString::<128>::new();
        id.push_str(self.author.as_str());
        id.push('/');
        id.push_str(self.slug.as_str());
        if let Some(v) = &self.variant {
            id.push(':');
            id.push_str(v.as_str());
        }
        let id = id.as_str();
        let mut out = String::with_capacity(URL_PREFIX.len() + id.len() + 2 + URL_SUFFIX.len());
        out.push_str(URL_PREFIX);
        out.push_str(&id.replace(":", "%3A"));
        out.push_str(URL_SUFFIX);
        out
    }
}

// ---------- Strategy D: Interning (lasso) ----------

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct Symbol(Spur);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct DIntern {
    author: Symbol,
    slug: Symbol,
    variant: Option<Symbol>,
}

impl DIntern {
    fn parse(rodeo: &ThreadedRodeo, s: &str) -> Option<Self> {
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

    fn endpoints_url(&self, rodeo: &ThreadedRodeo) -> String {
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
        format!("{}{}{}", URL_PREFIX, utf8_percent_encode(&id, PATH_ENCODE), URL_SUFFIX)
    }

    fn endpoints_url_concat_known(&self, rodeo: &ThreadedRodeo) -> String {
        let a = rodeo.resolve(&self.author.0);
        let g = rodeo.resolve(&self.slug.0);
        let var = self.variant.map(|v| rodeo.resolve(&v.0));
        let extra = if var.is_some() { 3 } else { 0 };
        let mut url = String::with_capacity(
            URL_PREFIX.len()
                + a.len()
                + 1
                + g.len()
                + extra
                + var.as_ref().map(|v| v.len()).unwrap_or(0)
                + URL_SUFFIX.len(),
        );
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

    fn endpoints_url_prealloc_write(&self, rodeo: &ThreadedRodeo) -> String {
        let a = rodeo.resolve(&self.author.0);
        let g = rodeo.resolve(&self.slug.0);
        let var = self.variant.map(|v| rodeo.resolve(&v.0));
        let id_len = a.len() + 1 + g.len() + var.as_ref().map(|v| 1 + v.len()).unwrap_or(0);
        let encoded_growth = if var.is_some() { 2 } else { 0 };
        let mut out = String::with_capacity(URL_PREFIX.len() + id_len + encoded_growth + URL_SUFFIX.len());
        out.push_str(URL_PREFIX);
        let mut tmp = String::with_capacity(id_len);
        tmp.push_str(a);
        tmp.push('/');
        tmp.push_str(g);
        if let Some(v) = var {
            tmp.push(':');
            tmp.push_str(v);
        }
        write!(&mut out, "{}", utf8_percent_encode(&tmp, PATH_ENCODE)).unwrap();
        out.push_str(URL_SUFFIX);
        out
    }

    fn endpoints_url_replace(&self, rodeo: &ThreadedRodeo) -> String {
        let a = rodeo.resolve(&self.author.0);
        let g = rodeo.resolve(&self.slug.0);
        let var = self.variant.map(|v| rodeo.resolve(&v.0));
        let mut id = String::with_capacity(a.len() + 1 + g.len() + var.as_ref().map(|v| 1 + v.len()).unwrap_or(0));
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

// ---------- Bench kernels ----------

fn bench_parse_only(c: &mut Criterion, label: &str, inputs: &[String]) {
    let mut group = c.benchmark_group(format!("parse_only/{}", label));
    group.throughput(Throughput::Elements(inputs.len() as u64));

    group.bench_function(BenchmarkId::new("String", inputs.len()), |b| {
        b.iter(|| {
            let out: Vec<_> = inputs
                .iter()
                .filter_map(|s| AString::parse(black_box(s)))
                .collect();
            black_box(out)
        })
    });

    group.bench_function(BenchmarkId::new("SmolStr", inputs.len()), |b| {
        b.iter(|| {
            let out: Vec<_> = inputs
                .iter()
                .filter_map(|s| BSmol::parse(black_box(s)))
                .collect();
            black_box(out)
        })
    });

    group.bench_function(BenchmarkId::new("ArrayString", inputs.len()), |b| {
        b.iter(|| {
            let out: Vec<_> = inputs
                .iter()
                .filter_map(|s| CArray::parse(black_box(s)))
                .collect();
            black_box(out)
        })
    });

    group.bench_function(BenchmarkId::new("Intern(lasso)", inputs.len()), |b| {
        b.iter(|| {
            let rodeo = ThreadedRodeo::default();
            let out: Vec<_> = inputs
                .iter()
                .filter_map(|s| DIntern::parse(&rodeo, black_box(s)))
                .collect();
            black_box((out, rodeo)) // keep rodeo alive to avoid dropping early
        })
    });

    group.bench_function(BenchmarkId::new("Intern(lasso,warm)", inputs.len()), |b| {
        preseed_global_rodeo(inputs);
        b.iter(|| {
            // no new interner; just lookups
            let out: Vec<_> = inputs
                .iter()
                .filter_map(|s| DIntern::parse(&GLOBAL_RODEO, black_box(s)))
                .collect();
            black_box(out)
        })
    });

    group.finish();
}

fn bench_url_build(c: &mut Criterion, label: &str, inputs: &[String]) {
    let mut group = c.benchmark_group(format!("url_build/{}", label));
    group.throughput(Throughput::Elements(inputs.len() as u64));

    // Pre-parse once outside measurement to isolate URL formatting
    let parsed_a: Vec<_> = inputs.iter().filter_map(|s| AString::parse(s)).collect();
    let parsed_b: Vec<_> = inputs.iter().filter_map(|s| BSmol::parse(s)).collect();
    let parsed_c: Vec<_> = inputs.iter().filter_map(|s| CArray::parse(s)).collect();
    let rodeo = ThreadedRodeo::default();
    let parsed_d: Vec<_> = inputs
        .iter()
        .filter_map(|s| DIntern::parse(&rodeo, s))
        .collect();

    // Baseline: format! + percent-encode
    group.bench_function(BenchmarkId::new("String", parsed_a.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_a.iter().map(|id| id.endpoints_url()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("SmolStr", parsed_b.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_b.iter().map(|id| id.endpoints_url()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("ArrayString", parsed_c.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_c.iter().map(|id| id.endpoints_url()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("Intern(lasso)", parsed_d.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_d.iter().map(|id| id.endpoints_url(&rodeo)).collect();
            black_box(urls)
        })
    });

    // Alt 1: concat with known encoding
    group.bench_function(BenchmarkId::new("String+concat_known", parsed_a.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_a.iter().map(|id| id.endpoints_url_concat_known()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("SmolStr+concat_known", parsed_b.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_b.iter().map(|id| id.endpoints_url_concat_known()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("ArrayString+concat_known", parsed_c.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_c.iter().map(|id| id.endpoints_url_concat_known()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("Intern(lasso)+concat_known", parsed_d.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_d.iter().map(|id| id.endpoints_url_concat_known(&rodeo)).collect();
            black_box(urls)
        })
    });

    // Alt 2: prealloc + write! encoder
    group.bench_function(BenchmarkId::new("String+prealloc_write", parsed_a.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_a.iter().map(|id| id.endpoints_url_prealloc_write()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("SmolStr+prealloc_write", parsed_b.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_b.iter().map(|id| id.endpoints_url_prealloc_write()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("ArrayString+prealloc_write", parsed_c.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_c.iter().map(|id| id.endpoints_url_prealloc_write()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("Intern(lasso)+prealloc_write", parsed_d.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_d.iter().map(|id| id.endpoints_url_prealloc_write(&rodeo)).collect();
            black_box(urls)
        })
    });

    // Alt 3: replace(':', "%3A")
    group.bench_function(BenchmarkId::new("String+replace", parsed_a.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_a.iter().map(|id| id.endpoints_url_replace()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("SmolStr+replace", parsed_b.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_b.iter().map(|id| id.endpoints_url_replace()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("ArrayString+replace", parsed_c.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_c.iter().map(|id| id.endpoints_url_replace()).collect();
            black_box(urls)
        })
    });

    group.bench_function(BenchmarkId::new("Intern(lasso)+replace", parsed_d.len()), |b| {
        b.iter(|| {
            let urls: Vec<_> = parsed_d.iter().map(|id| id.endpoints_url_replace(&rodeo)).collect();
            black_box(urls)
        })
    });

    group.finish();
}

fn bench_hashmap_insert(c: &mut Criterion, label: &str, inputs: &[String]) {
    let mut group = c.benchmark_group(format!("hashmap_insert/{}", label));
    group.throughput(Throughput::Elements(inputs.len() as u64));

    group.bench_function(BenchmarkId::new("String", inputs.len()), |b| {
        b.iter(|| {
            let mut map: HashMap<AString, u32, fxhash::FxBuildHasher> =
                HashMap::with_capacity_and_hasher(inputs.len(), fxhash::FxBuildHasher::default());
            for s in inputs {
                if let Some(id) = AString::parse(black_box(s)) {
                    *map.entry(id).or_insert(0) += 1;
                }
            }
            black_box(map)
        })
    });

    group.bench_function(BenchmarkId::new("SmolStr", inputs.len()), |b| {
        b.iter(|| {
            let mut map: HashMap<BSmol, u32, fxhash::FxBuildHasher> =
                HashMap::with_capacity_and_hasher(inputs.len(), fxhash::FxBuildHasher::default());
            for s in inputs {
                if let Some(id) = BSmol::parse(black_box(s)) {
                    *map.entry(id).or_insert(0) += 1;
                }
            }
            black_box(map)
        })
    });

    group.bench_function(BenchmarkId::new("ArrayString", inputs.len()), |b| {
        b.iter(|| {
            let mut map: HashMap<CArray, u32, fxhash::FxBuildHasher> =
                HashMap::with_capacity_and_hasher(inputs.len(), fxhash::FxBuildHasher::default());
            for s in inputs {
                if let Some(id) = CArray::parse(black_box(s)) {
                    *map.entry(id).or_insert(0) += 1;
                }
            }
            black_box(map)
        })
    });

    group.bench_function(BenchmarkId::new("Intern(lasso)", inputs.len()), |b| {
        b.iter(|| {
            let rodeo = ThreadedRodeo::default();
            let mut map: HashMap<DIntern, u32, fxhash::FxBuildHasher> =
                HashMap::with_capacity_and_hasher(inputs.len(), fxhash::FxBuildHasher::default());
            for s in inputs {
                if let Some(id) = DIntern::parse(&rodeo, black_box(s)) {
                    *map.entry(id).or_insert(0) += 1;
                }
            }
            black_box((map, rodeo))
        })
    });

    group.bench_function(
        BenchmarkId::new("Intern(lasso,warm)_symbol_map", inputs.len()),
        |b| {
            preseed_global_rodeo(inputs);
            b.iter(|| {
                use fxhash::FxBuildHasher;
                let mut map: HashMap<DIntern, u32, FxBuildHasher> =
                    HashMap::with_capacity_and_hasher(inputs.len(), FxBuildHasher::default());
                for s in inputs {
                    if let Some(id) = DIntern::parse(&GLOBAL_RODEO, black_box(s)) {
                        *map.entry(id).or_insert(0) += 1; // tiny key (3× u32 + Option)
                    }
                }
                black_box(map)
            })
        },
    );

    group.finish();
}

fn bench_end_to_end(c: &mut Criterion, label: &str, inputs: &[String]) {
    let mut group = c.benchmark_group(format!("end_to_end/{}", label));
    group.throughput(Throughput::Elements(inputs.len() as u64));

    group.bench_function(BenchmarkId::new("String", inputs.len()), |b| {
        b.iter(|| {
            let mut map: HashMap<String, u32, fxhash::FxBuildHasher> =
                HashMap::with_capacity_and_hasher(inputs.len(), fxhash::FxBuildHasher::default());
            for s in inputs {
                if let Some(id) = AString::parse(black_box(s)) {
                    let url = id.endpoints_url();
                    *map.entry(url).or_insert(0) += 1;
                }
            }
            black_box(map)
        })
    });

    group.bench_function(BenchmarkId::new("SmolStr", inputs.len()), |b| {
        b.iter(|| {
            let mut map: HashMap<String, u32, fxhash::FxBuildHasher> =
                HashMap::with_capacity_and_hasher(inputs.len(), fxhash::FxBuildHasher::default());
            for s in inputs {
                if let Some(id) = BSmol::parse(black_box(s)) {
                    let url = id.endpoints_url();
                    *map.entry(url).or_insert(0) += 1;
                }
            }
            black_box(map)
        })
    });

    group.bench_function(BenchmarkId::new("ArrayString", inputs.len()), |b| {
        b.iter(|| {
            let mut map: HashMap<String, u32, fxhash::FxBuildHasher> =
                HashMap::with_capacity_and_hasher(inputs.len(), fxhash::FxBuildHasher::default());
            for s in inputs {
                if let Some(id) = CArray::parse(black_box(s)) {
                    let url = id.endpoints_url();
                    *map.entry(url).or_insert(0) += 1;
                }
            }
            black_box(map)
        })
    });

    group.bench_function(BenchmarkId::new("Intern(lasso)", inputs.len()), |b| {
        b.iter(|| {
            let rodeo = ThreadedRodeo::default();
            let mut map: HashMap<String, u32, fxhash::FxBuildHasher> =
                HashMap::with_capacity_and_hasher(inputs.len(), fxhash::FxBuildHasher::default());
            for s in inputs {
                if let Some(id) = DIntern::parse(&rodeo, black_box(s)) {
                    let url = id.endpoints_url(&rodeo);
                    *map.entry(url).or_insert(0) += 1;
                }
            }
            black_box((map, rodeo))
        })
    });

    group.bench_function(
        BenchmarkId::new("Intern(lasso,warm)_end2end_compact_key", inputs.len()),
        |b| {
            preseed_global_rodeo(inputs);
            b.iter(|| {
                use fxhash::FxBuildHasher;
                let mut map: HashMap<DIntern, (u32, String), FxBuildHasher> =
                    HashMap::with_capacity_and_hasher(inputs.len(), FxBuildHasher::default());
                for s in inputs {
                    if let Some(id) = DIntern::parse(&GLOBAL_RODEO, black_box(s)) {
                        // still build the URL string (network path needs it),
                        // but the heavy *key* is tiny and cheap to hash/compare
                        let url = id.endpoints_url(&GLOBAL_RODEO);
                        let entry = map.entry(id).or_insert((0, url));
                        entry.0 += 1;
                    }
                }
                black_box(map)
            })
        },
    );

    group.finish();
}

// ---------- Top-level harness ----------

pub fn benches(c: &mut Criterion) {
    // Small & big sizes; adjust as you like
    let sizes = [10_000usize, 100_000usize];

    for &n in &sizes {
        // Case 1: all unique → interning has less to win on
        let unique = gen_unique_inputs(n);
        bench_parse_only(c, &format!("unique/{n}"), &unique);
        bench_url_build(c, &format!("unique/{n}"), &unique);
        bench_hashmap_insert(c, &format!("unique/{n}"), &unique);
        bench_end_to_end(c, &format!("unique/{n}"), &unique);

        // Case 2: repetition → interning shines
        let repeated = gen_repeated_inputs(n / 100, 100); // e.g., 1k unique × 100 = n
        bench_parse_only(c, &format!("repeated/{n}"), &repeated);
        bench_url_build(c, &format!("repeated/{n}"), &repeated);
        bench_hashmap_insert(c, &format!("repeated/{n}"), &repeated);
        bench_end_to_end(c, &format!("repeated/{n}"), &repeated);
    }
}

criterion_group!(model_benches, benches);
criterion_main!(model_benches);
