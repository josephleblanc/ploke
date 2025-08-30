use ploke_test_utils::workspace_root;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::llm::models::REL_MODEL_ALL_DATA_STATS;

pub(super) trait ValueVisitor {
    fn visit_null(&mut self, path: &str);
    fn visit_bool(&mut self, path: &str, b: bool);
    fn visit_number(&mut self, path: &str, n: &serde_json::Number);
    fn visit_string(&mut self, path: &str, s: &str);
    fn visit_array(&mut self, path: &str, arr: &[Value]);
    fn visit_object(&mut self, path: &str, obj: &serde_json::Map<String, Value>);
}

pub(super) fn walk_value<V: ValueVisitor>(visitor: &mut V, path: &str, value: &Value) {
    match value {
        Value::Null => visitor.visit_null(path),
        Value::Bool(b) => visitor.visit_bool(path, *b),
        Value::Number(n) => visitor.visit_number(path, n),
        Value::String(s) => visitor.visit_string(path, s),
        Value::Array(arr) => {
            visitor.visit_array(path, arr);
            for (i, v) in arr.iter().enumerate() {
                let child_path = format!("{path}[{i}]");
                walk_value(visitor, &child_path, v);
            }
        }
        Value::Object(obj) => {
            visitor.visit_object(path, obj);
            for (k, v) in obj {
                let child_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                walk_value(visitor, &child_path, v);
            }
        }
    }
}

pub(super) struct ProfilingVisitor {
    pub(super) field_counts: HashMap<String, usize>,
    pub(super) field_nulls: HashMap<String, usize>,
    pub(super) field_values: HashMap<String, HashSet<String>>,
    pub(super) top_level_keys: Vec<HashSet<String>>,
}

impl ProfilingVisitor {
    pub(super) fn new() -> Self {
        Self {
            field_counts: HashMap::new(),
            field_nulls: HashMap::new(),
            field_values: HashMap::new(),
            top_level_keys: Vec::new(),
        }
    }
}

impl ValueVisitor for ProfilingVisitor {
    fn visit_null(&mut self, path: &str) {
        *self.field_counts.entry(path.to_string()).or_insert(0) += 1;
        *self.field_nulls.entry(path.to_string()).or_insert(0) += 1;
    }

    fn visit_bool(&mut self, path: &str, b: bool) {
        *self.field_counts.entry(path.to_string()).or_insert(0) += 1;
        self.field_values
            .entry(path.to_string())
            .or_default()
            .insert(b.to_string());
    }

    fn visit_number(&mut self, path: &str, n: &serde_json::Number) {
        *self.field_counts.entry(path.to_string()).or_insert(0) += 1;
        let entry = self.field_values.entry(path.to_string()).or_default();
        if let Some(i) = n.as_i64() {
            entry.insert(i.to_string());
        } else if let Some(f) = n.as_f64() {
            entry.insert(format!("{:.6}", f));
        }
    }

    fn visit_string(&mut self, path: &str, s: &str) {
        *self.field_counts.entry(path.to_string()).or_insert(0) += 1;
        self.field_values
            .entry(path.to_string())
            .or_default()
            .insert(s.to_string());
    }

    fn visit_array(&mut self, path: &str, arr: &[Value]) {
        *self.field_counts.entry(path.to_string()).or_insert(0) += 1;
        // Track array lengths separately
        self.field_values
            .entry(format!("{path}._len"))
            .or_default()
            .insert(arr.len().to_string());

        // Special-case: supported_parameters
        if path.ends_with("supported_parameters") {
            for v in arr {
                if let Some(s) = v.as_str() {
                    self.field_values
                        .entry("supported_parameters.ALL".to_string())
                        .or_default()
                        .insert(s.to_string());
                }
            }
        }
    }

    fn visit_object(&mut self, path: &str, obj: &serde_json::Map<String, Value>) {
        *self.field_counts.entry(path.to_string()).or_insert(0) += 1;

        // Record top-level keys for schema consistency check
        if path.is_empty() {
            self.top_level_keys.push(obj.keys().cloned().collect());
        }
    }
}

/// Stats configuration knobs to keep memory bounded and guide recommendations.
#[derive(Clone, Copy)]
struct Config {
    top_k_values: usize,
    enum_max_cardinality: usize,
    enum_ratio_max: f64,
    max_union_values: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            top_k_values: 30,
            enum_max_cardinality: 20,
            enum_ratio_max: 0.1, // 10% of dataset
            max_union_values: 100,
        }
    }
}

#[derive(Default, Clone)]
struct TypeCounts {
    null: usize,
    bool_: usize,
    int: usize,
    float: usize,
    string_non_numeric: usize,
    string_numeric_like: usize,
    string_date_like: usize,
    string_uuid_like: usize,
    object: usize,
    array: usize,
}

#[derive(Default, Clone)]
struct NumericStats {
    min: Option<f64>,
    max: Option<f64>,
    ints: usize,
    floats: usize,
}

#[derive(Default, Clone)]
struct FieldStats {
    presence_models: usize,
    null_models: usize,
    type_counts: TypeCounts,
    // scalar value frequencies (strings/bools/numbers stringified); capped at top_k_values
    value_freq: HashMap<String, usize>,
    value_unique_tracked: HashSet<String>,
    value_overflow: usize,
    // arrays
    array_len_dist: HashMap<usize, usize>,
    elem_type_counts: TypeCounts,
    array_string_union_freq: HashMap<String, usize>,
    array_string_union_overflow: usize,
    // objects
    object_key_union_freq: HashMap<String, usize>,
    numeric: NumericStats,
}

impl FieldStats {
    fn bump_presence(&mut self) {
        self.presence_models += 1;
    }
    fn bump_null(&mut self) {
        self.null_models += 1;
        self.type_counts.null += 1;
    }
}

struct StatsBuilder {
    fields: HashMap<String, FieldStats>,
    config: Config,
}

impl StatsBuilder {
    fn new(config: Config) -> Self {
        Self { fields: HashMap::new(), config }
    }

    fn field_mut(&mut self, path: &str) -> &mut FieldStats {
        self.fields.entry(path.to_string()).or_default()
    }

}

fn record_scalar_value_with_limits(st: &mut FieldStats, cfg: Config, val: String) {
    if st.value_unique_tracked.len() < cfg.top_k_values {
        let _ = st.value_unique_tracked.insert(val.clone());
        *st.value_freq.entry(val).or_insert(0) += 1;
    } else if st.value_freq.contains_key(&val) {
        *st.value_freq.get_mut(&val).unwrap() += 1;
    } else {
        st.value_overflow += 1;
    }
}

fn record_array_string_union_with_limits(st: &mut FieldStats, cfg: Config, s: &str) {
    if st.array_string_union_freq.len() < cfg.max_union_values {
        *st.array_string_union_freq.entry(s.to_string()).or_insert(0) += 1;
    } else if st.array_string_union_freq.contains_key(s) {
        *st.array_string_union_freq.get_mut(s).unwrap() += 1;
    } else {
        st.array_string_union_overflow += 1;
    }
}

fn is_string_numeric_like(s: &str) -> bool {
    // Allow decimal, exponent, leading +/-; reject empty/whitespace
    if s.trim().is_empty() { return false; }
    // fast path: parse
    s.parse::<f64>().is_ok()
}

fn is_uuid_like(s: &str) -> bool {
    // Minimal check for canonical UUID v1-5 format: 8-4-4-4-12 hex with hyphens
    if s.len() != 36 { return false; }
    let bytes = s.as_bytes();
    for &i in &[8usize, 13, 18, 23] {
        if bytes[i] != b'-' { return false; }
    }
    for (i, &b) in bytes.iter().enumerate() {
        if i == 8 || i == 13 || i == 18 || i == 23 { continue; }
        let is_hex = b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || (b'A'..=b'F').contains(&b);
        if !is_hex { return false; }
    }
    true
}

fn is_iso8601_like(s: &str) -> bool {
    // Heuristic: YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS(.sss)?Z
    let bytes = s.as_bytes();
    if bytes.len() >= 10 {
        let ok_date = bytes.get(4) == Some(&b'-') && bytes.get(7) == Some(&b'-')
            && bytes[..4].iter().all(|b: &u8| b.is_ascii_digit())
            && bytes[5..7].iter().all(|b: &u8| b.is_ascii_digit())
            && bytes[8..10].iter().all(|b: &u8| b.is_ascii_digit());
        if ok_date {
            // If time part exists, check basic structure
            if s.len() > 10 {
                let has_t = s.contains('T');
                let has_colon = s[11..].contains(':');
                if has_t && has_colon { return true; }
            } else {
                return true;
            }
        }
    }
    false
}

fn traverse_model(builder: &mut StatsBuilder, v: &Value, path: &str, seen_paths: &mut HashSet<String>) {
    match v {
        Value::Null => {
            let first = seen_paths.insert(path.to_string());
            let st = builder.field_mut(path);
            if first { st.bump_presence(); }
            st.bump_null();
        }
        Value::Bool(b) => {
            let first = seen_paths.insert(path.to_string());
            let cfg = builder.config;
            {
                let st = builder.field_mut(path);
                if first { st.bump_presence(); }
                st.type_counts.bool_ += 1;
                record_scalar_value_with_limits(st, cfg, b.to_string());
            }
        }
        Value::Number(n) => {
            let first = seen_paths.insert(path.to_string());
            let cfg = builder.config;
            {
                let st = builder.field_mut(path);
                if first { st.bump_presence(); }
                if let Some(i) = n.as_i64() {
                    st.type_counts.int += 1;
                    st.numeric.ints += 1;
                    let f = i as f64;
                    st.numeric.min = Some(st.numeric.min.map_or(f, |m| m.min(f)));
                    st.numeric.max = Some(st.numeric.max.map_or(f, |m| m.max(f)));
                    record_scalar_value_with_limits(st, cfg, i.to_string());
                } else if let Some(f) = n.as_f64() {
                    st.type_counts.float += 1;
                    st.numeric.floats += 1;
                    st.numeric.min = Some(st.numeric.min.map_or(f, |m| m.min(f)));
                    st.numeric.max = Some(st.numeric.max.map_or(f, |m| m.max(f)));
                    record_scalar_value_with_limits(st, cfg, format!("{}", n));
                }
            }
        }
        Value::String(s) => {
            let first = seen_paths.insert(path.to_string());
            let cfg = builder.config;
            {
                let st = builder.field_mut(path);
                if first { st.bump_presence(); }
                if is_string_numeric_like(s) {
                    st.type_counts.string_numeric_like += 1;
                } else if is_uuid_like(s) {
                    st.type_counts.string_uuid_like += 1;
                } else if is_iso8601_like(s) {
                    st.type_counts.string_date_like += 1;
                } else {
                    st.type_counts.string_non_numeric += 1;
                }
                record_scalar_value_with_limits(st, cfg, s.to_string());
            }
        }
        Value::Array(arr) => {
            let first = seen_paths.insert(path.to_string());
            let cfg = builder.config;
            {
                let st = builder.field_mut(path);
                if first { st.bump_presence(); }
                st.type_counts.array += 1;
                *st.array_len_dist.entry(arr.len()).or_insert(0) += 1;
                for elem in arr {
                    match elem {
                        Value::Null => st.elem_type_counts.null += 1,
                        Value::Bool(_) => st.elem_type_counts.bool_ += 1,
                        Value::Number(n) => {
                            if n.as_i64().is_some() { st.elem_type_counts.int += 1; }
                            else { st.elem_type_counts.float += 1; }
                        }
                        Value::String(s) => {
                            st.elem_type_counts.string_non_numeric += 1; // element-level numeric-like not critical now
                            record_array_string_union_with_limits(st, cfg, s);
                        }
                        Value::Array(_) => st.elem_type_counts.array += 1,
                        Value::Object(_) => st.elem_type_counts.object += 1,
                    }
                }
            }
            // recurse without indexing arrays in path; ensure prior borrows dropped
            for elem in arr {
                traverse_model(builder, elem, path, seen_paths);
            }
        }
        Value::Object(map) => {
            let first = if path.is_empty() { false } else { seen_paths.insert(path.to_string()) };
            if first {
                let st = builder.field_mut(path);
                st.bump_presence();
                st.type_counts.object += 1;
            }
            // record keys for this object under its path
            if !path.is_empty() {
                let st = builder.field_mut(path);
                for k in map.keys() {
                    *st.object_key_union_freq.entry(k.clone()).or_insert(0) += 1;
                }
            }
            for (k, v2) in map {
                let child_path = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                traverse_model(builder, v2, &child_path, seen_paths);
            }
        }
    }
}

pub fn explore_file_visit<P: AsRef<Path>>(path: P) {
    explore_file_visit_to_dir(path, None::<&Path>);
}

/// Profile JSON data and write stats next to configurable output directory.
/// If out_dir is None, falls back to default REL_MODEL_ALL_DATA_STATS location.
pub fn explore_file_visit_to_dir<P: AsRef<Path>, Q: AsRef<Path>>(path: P, out_dir: Option<Q>) {
    let data = fs::read_to_string(&path).expect("Failed to read file");
    let root: Value = serde_json::from_str(&data).expect("Invalid JSON");
    let models: Vec<Value> = root
        .get("data")
        .and_then(|v| v.as_array())
        .expect("an object containing an array")
        .to_vec();

    let cfg = Config::default();
    let mut builder = StatsBuilder::new(cfg);

    for model in &models {
        let mut seen_paths: HashSet<String> = HashSet::new();
        if let Value::Object(map) = model {
            for (k, v) in map {
                traverse_model(&mut builder, v, k, &mut seen_paths);
            }
        } else {
            traverse_model(&mut builder, model, "", &mut seen_paths);
        }
    }

    // === Write output ===
    let mut out = String::new();
    out.push_str("=== JSON Profiling Report ===\n");
    out.push_str(&format!("Models analyzed: {}\n\n", models.len()));

    out.push_str("Field presence (%, present/null/missing):\n");
    let mut fields_sorted: Vec<_> = builder.fields.iter().collect();
    fields_sorted.sort_by(|a, b| a.0.cmp(b.0));
    for (field, st) in &fields_sorted {
        let present = st.presence_models as f64;
        let total = models.len() as f64;
        let missing = total - present;
        out.push_str(&format!(
            "({:>5.1}%) {}: present={} null={} missing={}\n",
            if total > 0.0 { present * 100.0 / total } else { 0.0 },
            field,
            st.presence_models,
            st.null_models,
            missing as usize
        ));
    }
    out.push('\n');

    out.push_str("Type distributions per field:\n");
    for (field, st) in &fields_sorted {
        let tc = &st.type_counts;
        if tc.null + tc.bool_ + tc.int + tc.float + tc.string_non_numeric + tc.string_numeric_like + tc.string_date_like + tc.string_uuid_like + tc.object + tc.array == 0 {
            continue;
        }
        out.push_str(&format!(
            "{}: null={} bool={} int={} float={} str={} str(num)={} str(date)={} str(uuid)={} obj={} arr={}\n",
            field, tc.null, tc.bool_, tc.int, tc.float, tc.string_non_numeric, tc.string_numeric_like, tc.string_date_like, tc.string_uuid_like, tc.object, tc.array
        ));
        if st.numeric.ints + st.numeric.floats > 0 {
            let min = st.numeric.min.map(|v| format!("{:.6}", v)).unwrap_or_else(|| "-".into());
            let max = st.numeric.max.map(|v| format!("{:.6}", v)).unwrap_or_else(|| "-".into());
            out.push_str(&format!(
                "  numeric: ints={} floats={} min={} max={}\n",
                st.numeric.ints, st.numeric.floats, min, max
            ));
        }
    }
    out.push('\n');

    out.push_str("Array stats (lengths and element types):\n");
    for (field, st) in &fields_sorted {
        if !st.array_len_dist.is_empty() {
            out.push_str(&format!("{}:\n", field));
            let mut d: Vec<_> = st.array_len_dist.iter().collect();
            d.sort_by_key(|(len, _)| **len);
            for (len, freq) in d {
                out.push_str(&format!("  len {}: {} models\n", len, freq));
            }
            let etc = &st.elem_type_counts;
            out.push_str(&format!(
                "  elem types: null={} bool={} int={} float={} str={} obj={} arr={}\n",
                etc.null, etc.bool_, etc.int, etc.float, etc.string_non_numeric, etc.object, etc.array
            ));
        }
    }
    out.push('\n');

    out.push_str("Candidate enum-like fields (top values):\n");
    for (field, st) in &fields_sorted {
        let total_models = models.len();
        let has_scalar_types = st.type_counts.bool_ + st.type_counts.int + st.type_counts.float + st.type_counts.string_non_numeric + st.type_counts.string_numeric_like > 0;
        if !has_scalar_types { continue; }
        let numeric_only = st.type_counts.bool_ == 0 && st.type_counts.string_non_numeric == 0 && st.type_counts.string_numeric_like == 0 && (st.type_counts.int + st.type_counts.float > 0);
        if numeric_only { continue; }
        let cardinality = st.value_unique_tracked.len() + if st.value_overflow > 0 { st.value_overflow } else { 0 };
        let ratio = if total_models > 0 { (cardinality as f64) / (total_models as f64) } else { 1.0 };
        if cardinality > 1 && (cardinality <= builder.config.enum_max_cardinality || ratio <= builder.config.enum_ratio_max) {
            out.push_str(&format!(
                "{} (cardinality~{}, ratio~{:.3}):\n",
                field, cardinality, ratio
            ));
            let mut pairs: Vec<_> = st.value_freq.iter().collect();
            pairs.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            for (val, cnt) in pairs.iter().take(builder.config.top_k_values) {
                out.push_str(&format!("  - {} ({} models)\n", val, cnt));
            }
            if st.value_overflow > 0 {
                out.push_str(&format!("  ... +{} other values (not fully tracked)\n", st.value_overflow));
            }
        }
    }
    out.push('\n');

    out.push_str("Array-of-strings unions (top values):\n");
    for (field, st) in &fields_sorted {
        if !st.array_string_union_freq.is_empty() {
            out.push_str(&format!("{}:\n", field));
            let mut pairs: Vec<_> = st.array_string_union_freq.iter().collect();
            pairs.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            for (val, cnt) in pairs.into_iter().take(builder.config.max_union_values) {
                out.push_str(&format!("  - {} ({})\n", val, cnt));
            }
            if st.array_string_union_overflow > 0 {
                out.push_str(&format!("  ... +{} other values (not fully tracked)\n", st.array_string_union_overflow));
            }
        }
    }
    out.push('\n');

    out.push_str("Object key unions (top keys):\n");
    for (field, st) in &fields_sorted {
        if !st.object_key_union_freq.is_empty() {
            out.push_str(&format!("{}:\n", field));
            let mut pairs: Vec<_> = st.object_key_union_freq.iter().collect();
            pairs.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            for (key, cnt) in pairs.into_iter().take(50) {
                out.push_str(&format!("  - {} ({})\n", key, cnt));
            }
        }
    }
    out.push('\n');

    // Choose output directory
    let (txt_path, json_path) = if let Some(dir) = out_dir.as_ref().map(|d| d.as_ref().to_path_buf()) {
        let base = std::path::Path::new(path.as_ref()).file_stem().and_then(|s| s.to_str()).unwrap_or("stats");
        let mut txt = dir.clone();
        txt.push(format!("{}_stats.txt", base));
        let mut js = dir;
        js.push(format!("{}_stats.json", base));
        (txt, js)
    } else {
        let mut out_path = workspace_root();
        out_path.push(REL_MODEL_ALL_DATA_STATS);
        let json_out = out_path.with_extension("json");
        (out_path, json_out)
    };

    fs::write(&txt_path, &out).expect("Failed to write stats file");

    // Also emit JSON artifact
    let mut json_fields = serde_json::Map::new();
    for (field, st) in &builder.fields {
        let mut values_top: Vec<_> = st.value_freq.iter().collect();
        values_top.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let values_top = values_top
            .into_iter()
            .take(builder.config.top_k_values)
            .map(|(v, c)| json!({"value": v, "count": c}))
            .collect::<Vec<_>>();

        let mut lengths: Vec<_> = st.array_len_dist.iter().collect();
        lengths.sort_by_key(|(k, _)| **k);
        let lengths = lengths
            .into_iter()
            .map(|(k, v)| json!({"len": k, "count": v}))
            .collect::<Vec<_>>();

        let mut arr_union: Vec<_> = st.array_string_union_freq.iter().collect();
        arr_union.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let arr_union = arr_union
            .into_iter()
            .take(builder.config.max_union_values)
            .map(|(v, c)| json!({"value": v, "count": c}))
            .collect::<Vec<_>>();

        let mut obj_keys: Vec<_> = st.object_key_union_freq.iter().collect();
        obj_keys.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let obj_keys = obj_keys
            .into_iter()
            .take(100)
            .map(|(k, c)| json!({"key": k, "count": c}))
            .collect::<Vec<_>>();

        json_fields.insert(
            field.clone(),
            json!({
                "presence": {
                    "present": st.presence_models,
                    "null": st.null_models,
                    "missing": models.len().saturating_sub(st.presence_models),
                    "present_pct": if !models.is_empty() { (st.presence_models as f64) * 100.0 / (models.len() as f64) } else { 0.0 },
                },
                "types": {
                    "null": st.type_counts.null,
                    "bool": st.type_counts.bool_,
                    "int": st.type_counts.int,
                    "float": st.type_counts.float,
                    "string": st.type_counts.string_non_numeric,
                    "string_numeric_like": st.type_counts.string_numeric_like,
                    "string_date_like": st.type_counts.string_date_like,
                    "string_uuid_like": st.type_counts.string_uuid_like,
                    "object": st.type_counts.object,
                    "array": st.type_counts.array,
                },
                "numeric": {
                    "ints": st.numeric.ints,
                    "floats": st.numeric.floats,
                    "min": st.numeric.min,
                    "max": st.numeric.max,
                },
                "values": {
                    "top": values_top,
                    "overflow": st.value_overflow,
                },
                "array": {
                    "lengths": lengths,
                    "elem_types": {
                        "null": st.elem_type_counts.null,
                        "bool": st.elem_type_counts.bool_,
                        "int": st.elem_type_counts.int,
                        "float": st.elem_type_counts.float,
                        "string": st.elem_type_counts.string_non_numeric,
                        "object": st.elem_type_counts.object,
                        "array": st.elem_type_counts.array,
                    },
                    "string_union_top": arr_union,
                    "string_union_overflow": st.array_string_union_overflow,
                },
                "object": { "key_union_top": obj_keys }
            }),
        );
    }

    let json_summary = json!({ "models": models.len(), "fields": json_fields });
    fs::write(&json_path, serde_json::to_string_pretty(&json_summary).unwrap()).expect("Failed to write JSON stats file");
}

/// Endpoints variant: root expects { data: { endpoints: [ ... ] } }
pub fn explore_endpoints_file_visit_to_dir<P: AsRef<Path>, Q: AsRef<Path>>(path: P, out_dir: Q) {
    let data = fs::read_to_string(&path).expect("Failed to read file");
    let root: Value = serde_json::from_str(&data).expect("Invalid JSON");
    let arr = root
        .get("data")
        .and_then(|d| d.get("endpoints"))
        .and_then(|v| v.as_array())
        .expect("object with data.endpoints array")
        .to_vec();

    // Reuse the general logic by wrapping into { "data": [...] }
    let wrapped = json!({ "data": arr });
    let tmp = serde_json::to_string(&wrapped).unwrap();
    // Write to a temp file in memory path? Instead, traverse directly using the same machinery.
    // We'll inline traversal to avoid writing an intermediate file.

    let cfg = Config::default();
    let mut builder = StatsBuilder::new(cfg);

    for v in wrapped.get("data").and_then(|a| a.as_array()).unwrap() {
        let mut seen_paths: HashSet<String> = HashSet::new();
        if let Value::Object(map) = v {
            for (k, val) in map {
                traverse_model(&mut builder, val, k, &mut seen_paths);
            }
        } else {
            traverse_model(&mut builder, v, "", &mut seen_paths);
        }
    }

    // Compose out paths
    let base = std::path::Path::new(path.as_ref()).file_stem().and_then(|s| s.to_str()).unwrap_or("endpoint");
    let mut txt = out_dir.as_ref().to_path_buf();
    txt.push(format!("{}_stats.txt", base));
    let mut js = out_dir.as_ref().to_path_buf();
    js.push(format!("{}_stats.json", base));

    // Emit text same as above
    let mut out = String::new();
    out.push_str("=== JSON Profiling Report (endpoints) ===\n");
    out.push_str(&format!("Endpoints analyzed: {}\n\n", arr.len()));

    let mut fields_sorted: Vec<_> = builder.fields.iter().collect();
    fields_sorted.sort_by(|a, b| a.0.cmp(b.0));
    for (field, st) in &fields_sorted {
        let present = st.presence_models as f64;
        let total = arr.len() as f64;
        let missing = total - present;
        out.push_str(&format!(
            "({:>5.1}%) {}: present={} null={} missing={}\n",
            if total > 0.0 { present * 100.0 / total } else { 0.0 },
            field,
            st.presence_models,
            st.null_models,
            missing as usize
        ));
    }

    fs::write(&txt, &out).expect("Failed to write endpoints stats txt");

    // Emit JSON
    let mut json_fields = serde_json::Map::new();
    for (field, st) in &builder.fields {
        json_fields.insert(
            field.clone(),
            json!({
                "presence": {
                    "present": st.presence_models,
                    "null": st.null_models,
                    "missing": arr.len().saturating_sub(st.presence_models),
                }
            }),
        );
    }
    let summary = json!({ "endpoints": arr.len(), "fields": json_fields });
    fs::write(&js, serde_json::to_string_pretty(&summary).unwrap()).expect("Failed to write endpoints stats json");
}

/// Analyze cardinality for specific keys at the top-level of models JSON and write to out_dir.
pub fn analyze_cardinality_to_dir<P: AsRef<Path>, Q: AsRef<Path>>(path: P, out_dir: Q, keys: &[&str]) {
    let data = fs::read_to_string(&path).expect("Failed to read file");
    let root: Value = serde_json::from_str(&data).expect("Invalid JSON");
    let models: Vec<Value> = root
        .get("data")
        .and_then(|v| v.as_array())
        .expect("an object containing an array")
        .to_vec();

    let mut sets: HashMap<String, HashSet<String>> = HashMap::new();

    fn collect(path: &str, v: &Value, sets: &mut HashMap<String, HashSet<String>>, targets: &std::collections::HashSet<String>) {
        match v {
            Value::Null | Value::Bool(_) | Value::Number(_) => {
                // Only collect strings scalars to reduce noise, but include numbers stringified if directly targeted
                if targets.contains(path) {
                    sets.entry(path.to_string()).or_default().insert(v.to_string());
                }
            }
            Value::String(s) => {
                if targets.contains(path) {
                    sets.entry(path.to_string()).or_default().insert(s.to_string());
                }
            }
            Value::Array(arr) => {
                for elem in arr {
                    collect(path, elem, sets, targets);
                }
            }
            Value::Object(map) => {
                for (k, v2) in map {
                    let child_path = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                    collect(&child_path, v2, sets, targets);
                }
            }
        }
    }

    let target_set: std::collections::HashSet<String> = keys.iter().map(|s| s.to_string()).collect();
    for model in &models {
        if let Value::Object(map) = model {
            for (k, v) in map {
                let child_path = k.clone();
                collect(&child_path, v, &mut sets, &target_set);
            }
        }
    }

    let mut out_txt = String::new();
    out_txt.push_str("=== Cardinality Report ===\n");
    for key in keys {
        let set = sets.get(*key).map(|s| s.len()).unwrap_or(0);
        out_txt.push_str(&format!("{}: {} unique values\n", key, set));
    }

    let mut out_json = serde_json::Map::new();
    for key in keys {
        let values = sets.get(*key).map(|s| {
            let mut v: Vec<_> = s.iter().cloned().collect();
            v.sort();
            v
        }).unwrap_or_default();
        out_json.insert(key.to_string(), json!({ "unique": values.len(), "samples": values.iter().take(100).collect::<Vec<_>>() }));
    }

    let base = std::path::Path::new(path.as_ref()).file_stem().and_then(|s| s.to_str()).unwrap_or("models");
    let mut txt = out_dir.as_ref().to_path_buf();
    txt.push(format!("{}_cardinality.txt", base));
    let mut js = out_dir.as_ref().to_path_buf();
    js.push(format!("{}_cardinality.json", base));

    fs::write(&txt, out_txt).expect("write cardinality txt");
    fs::write(&js, serde_json::to_string_pretty(&Value::Object(out_json)).unwrap()).expect("write cardinality json");
}

/// Analyze endpoints cardinality: extract data.endpoints and then compute set sizes for selected keys.
pub fn analyze_endpoints_cardinality_to_dir<P: AsRef<Path>, Q: AsRef<Path>>(path: P, out_dir: Q, keys: &[&str]) {
    let data = fs::read_to_string(&path).expect("Failed to read file");
    let root: Value = serde_json::from_str(&data).expect("Invalid JSON");
    let arr = root
        .get("data")
        .and_then(|d| d.get("endpoints"))
        .and_then(|v| v.as_array())
        .expect("object with data.endpoints array")
        .to_vec();

    let mut sets: HashMap<String, HashSet<String>> = HashMap::new();

    for ep in &arr {
        if let Value::Object(map) = ep {
            for (k, v) in map {
                if keys.contains(&k.as_str()) {
                    if let Value::String(s) = v { sets.entry(k.clone()).or_default().insert(s.clone()); }
                    else { sets.entry(k.clone()).or_default().insert(v.to_string()); }
                }
            }
        }
    }

    let mut out_txt = String::new();
    out_txt.push_str("=== Endpoints Cardinality Report ===\n");
    for key in keys {
        let set = sets.get(*key).map(|s| s.len()).unwrap_or(0);
        out_txt.push_str(&format!("{}: {} unique values\n", key, set));
    }

    let mut out_json = serde_json::Map::new();
    for key in keys {
        let values = sets.get(*key).map(|s| {
            let mut v: Vec<_> = s.iter().cloned().collect();
            v.sort();
            v
        }).unwrap_or_default();
        out_json.insert(key.to_string(), json!({ "unique": values.len(), "samples": values.iter().take(100).collect::<Vec<_>>() }));
    }

    let base = std::path::Path::new(path.as_ref()).file_stem().and_then(|s| s.to_str()).unwrap_or("endpoint");
    let mut txt = out_dir.as_ref().to_path_buf();
    txt.push(format!("{}_cardinality.txt", base));
    let mut js = out_dir.as_ref().to_path_buf();
    js.push(format!("{}_cardinality.json", base));

    fs::write(&txt, out_txt).expect("write endpoints cardinality txt");
    fs::write(&js, serde_json::to_string_pretty(&Value::Object(out_json)).unwrap()).expect("write endpoints cardinality json");
}

mod tests {
    use ploke_test_utils::workspace_root;

    use crate::llm::models::REL_MODEL_ALL_DATA_RAW;

    use super::*;
    #[test]
    fn test_explore_file_visit() {
        let mut path = workspace_root();
        path.push(REL_MODEL_ALL_DATA_RAW);

        println!("Writing stats to file: {}", path.display());
        explore_file_visit(path);
    }
}
