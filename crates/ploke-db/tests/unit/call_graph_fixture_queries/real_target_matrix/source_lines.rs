use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use cozo::DataValue;
use ploke_db::multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE};
use ploke_test_utils::{
    FixtureAutomation, FixtureCreationStrategy, FixtureDb, backup_db_snapshot_fixture_dir,
};

use super::super::*;
use super::common::assert_no_traversal_candidates_for_sites;

#[derive(Clone, Copy)]
pub(super) struct SourceLineFanout {
    pub(super) file_suffix: &'static str,
    pub(super) lines: &'static [u32],
}

pub(super) fn assert_targetless_path_line_fanout(
    db: &Database,
    fixture: &FixtureDb,
    path_parts: &[&str],
    status: CallStatusKind,
    expected: &[SourceLineFanout],
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("path".to_string(), path_value(path_parts));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));

    let script = [
        ANCESTOR_RULES_NOW,
        METHOD_NODE_ANCESTOR_RULE,
        r#"
module_has_file[mid] := *file_mod{ owner_id: mid @ 'NOW' }
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[file_path, site_id, owner_id, span, resolution_kind] :=
    *call_site {
        id: site_id,
        owner_id,
        call_kind: "Path",
        path: $path,
        span @ 'NOW'
    },
    *call_resolution_status {
        source_id: site_id,
        source_kind: "Path",
        status_kind: $status,
        resolution_kind @ 'NOW'
    },
    ancestor[owner_id, module_id],
    *module { id: module_id @ 'NOW' },
    file_owner_for_module[module_id, file_id],
    *file_mod { owner_id: file_id, file_path @ 'NOW' }
:sort file_path, span, site_id
"#,
    ]
    .join("\n");

    let rows = db.raw_query_params(&script, params)?;
    let needle = path_parts.join("::");
    assert_targetless_line_rows(
        db,
        fixture,
        &rows.rows,
        expected,
        &needle,
        &format!("{status:?} source-line fanout rows for {path_parts:?}"),
    )
}

pub(super) fn assert_targetless_path_owner_kind_line_fanout(
    db: &Database,
    fixture: &FixtureDb,
    path_parts: &[&str],
    status: CallStatusKind,
    owner_kind: &str,
    expected: &[SourceLineFanout],
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("path".to_string(), path_value(path_parts));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));
    params.insert("owner_kind".to_string(), DataValue::from(owner_kind));

    let script = [
        ANCESTOR_RULES_NOW,
        METHOD_NODE_ANCESTOR_RULE,
        r#"
module_has_file[mid] := *file_mod{ owner_id: mid @ 'NOW' }
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[file_path, site_id, owner_id, span, resolution_kind] :=
    *call_site {
        id: site_id,
        owner_id,
        call_kind: "Path",
        path: $path,
        span @ 'NOW'
    },
    *call_resolution_status {
        source_id: site_id,
        source_kind: "Path",
        status_kind: $status,
        resolution_kind @ 'NOW'
    },
    *call_body_owner {
        id: owner_id,
        owner_kind: $owner_kind,
        parent_id @ 'NOW'
    },
    ancestor[parent_id, module_id],
    *module { id: module_id @ 'NOW' },
    file_owner_for_module[module_id, file_id],
    *file_mod { owner_id: file_id, file_path @ 'NOW' }
:sort file_path, span, site_id
"#,
    ]
    .join("\n");

    let rows = db.raw_query_params(&script, params)?;
    let needle = path_parts.join("::");
    assert_targetless_line_rows(
        db,
        fixture,
        &rows.rows,
        expected,
        &needle,
        &format!("{status:?} {owner_kind} source-line fanout rows for {path_parts:?}"),
    )
}

pub(super) fn assert_targetless_method_line_fanout(
    db: &Database,
    fixture: &FixtureDb,
    method: &str,
    receiver_kind: &str,
    receiver_path: Option<&[&str]>,
    status: CallStatusKind,
    expected: &[SourceLineFanout],
) -> Result<(), DbError> {
    assert_targetless_method_line_fanout_with_needle(
        db,
        fixture,
        method,
        receiver_kind,
        receiver_path,
        status,
        expected,
        method,
    )
}

pub(super) fn assert_targetless_method_line_fanout_with_needle(
    db: &Database,
    fixture: &FixtureDb,
    method: &str,
    receiver_kind: &str,
    receiver_path: Option<&[&str]>,
    status: CallStatusKind,
    expected: &[SourceLineFanout],
    needle: &str,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), DataValue::from(method));
    params.insert("receiver_kind".to_string(), DataValue::from(receiver_kind));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));
    params.insert(
        "receiver_path".to_string(),
        receiver_path.map_or(DataValue::Null, path_value),
    );

    let script = format!(
        r#"
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[file_path, site_id, owner_id, span, resolution_kind] :=
    *call_site {{
        id: site_id,
        owner_id,
        call_kind: "Method",
        method_name: $method,
        receiver_kind: $receiver_kind,
        receiver_path: $receiver_path,
        span @ 'NOW'
    }},
    *call_resolution_status {{
        source_id: site_id,
        source_kind: "Method",
        status_kind: $status,
        resolution_kind @ 'NOW'
    }},
    ancestor[owner_id, module_id],
    *module {{ id: module_id @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod {{ owner_id: file_id, file_path @ 'NOW' }}
:sort file_path, span, site_id
"#
    );

    let rows = db.raw_query_params(&script, params)?;
    assert_targetless_line_rows(
        db,
        fixture,
        &rows.rows,
        expected,
        needle,
        &format!("{status:?} source-line fanout rows for method {method:?}"),
    )
}

pub(super) fn assert_targetless_dynamic_line_fanout_by_method(
    db: &Database,
    fixture: &FixtureDb,
    owner_method: &str,
    status: CallStatusKind,
    expected: &[SourceLineFanout],
) -> Result<(), DbError> {
    assert_targetless_dynamic_line_fanout_by_method_filtered(
        db,
        fixture,
        owner_method,
        None,
        status,
        expected,
        "(self.call)",
    )
}

pub(super) fn assert_targetless_dynamic_line_fanout_by_method_arg_count(
    db: &Database,
    fixture: &FixtureDb,
    owner_method: &str,
    arg_count: u32,
    status: CallStatusKind,
    expected: &[SourceLineFanout],
    needle: &str,
) -> Result<(), DbError> {
    assert_targetless_dynamic_line_fanout_by_method_filtered(
        db,
        fixture,
        owner_method,
        Some(arg_count),
        status,
        expected,
        needle,
    )
}

fn assert_targetless_dynamic_line_fanout_by_method_filtered(
    db: &Database,
    fixture: &FixtureDb,
    owner_method: &str,
    arg_count: Option<u32>,
    status: CallStatusKind,
    expected: &[SourceLineFanout],
    needle: &str,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), DataValue::from(owner_method));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));
    let arg_filter = if let Some(arg_count) = arg_count {
        params.insert("arg_count".to_string(), DataValue::from(arg_count as i64));
        "arg_count: $arg_count,"
    } else {
        ""
    };

    let script = format!(
        r#"
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[file_path, site_id, owner_id, span, resolution_kind] :=
    *method {{ id: owner_id, name: $method @ 'NOW' }},
    *call_site {{
        id: site_id,
        owner_id,
        call_kind: "Dynamic",
        {arg_filter}
        span @ 'NOW'
    }},
    *call_resolution_status {{
        source_id: site_id,
        source_kind: "Dynamic",
        status_kind: $status,
        resolution_kind @ 'NOW'
    }},
    ancestor[owner_id, module_id],
    *module {{ id: module_id @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod {{ owner_id: file_id, file_path @ 'NOW' }}
:sort file_path, span, site_id
"#
    );

    let rows = db.raw_query_params(&script, params)?;
    assert_targetless_line_rows(
        db,
        fixture,
        &rows.rows,
        expected,
        needle,
        &format!("{status:?} source-line fanout rows for dynamic owner method {owner_method:?}"),
    )
}

fn assert_targetless_line_rows(
    db: &Database,
    fixture: &FixtureDb,
    rows: &[Vec<DataValue>],
    expected: &[SourceLineFanout],
    needle: &str,
    label: &str,
) -> Result<(), DbError> {
    let suffixes = expected
        .iter()
        .map(|case| case.file_suffix)
        .collect::<BTreeSet<_>>();
    let mut sources = BTreeMap::<String, String>::new();
    let mut actual = BTreeMap::<String, Vec<u32>>::new();
    let mut sites = Vec::new();

    for row in rows {
        let file_path = data_str(&row[0], "file_path");
        let Some(suffix) = suffixes.iter().find(|suffix| file_path.ends_with(**suffix)) else {
            panic!(
                "unexpected file path for {label} row: {file_path}; rows: {:#?}",
                rows
            );
        };
        assert_eq!(row[4], DataValue::Null);

        let site_id = to_uuid(&row[1])?;
        let owner_id = to_uuid(&row[2])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "{label} row in {suffix} should not have call_relation targets"
        );
        sites.push((owner_id, site_id));

        let source = sources.entry((*suffix).to_string()).or_insert_with(|| {
            let source_file = source_file_for_suffix(fixture, file_path, suffix);
            fs::read_to_string(&source_file).unwrap_or_else(|err| {
                panic!(
                    "failed to read pinned source file {}: {err}",
                    source_file.display()
                )
            })
        });
        let line = line_for_byte(source, span_start(&row[3]));
        let lines = source.lines().collect::<Vec<_>>();
        let line_index = line as usize - 1;
        let text = lines
            .get(line_index)
            .unwrap_or_else(|| panic!("{suffix}:{line} should exist in pinned source"));
        let window = lines[line_index..usize::min(line_index + 12, lines.len())].join("\n");
        assert!(
            window.contains(needle),
            "{suffix}:{line} should contain {needle:?} on the span line or nearby continuation; source line was {text:?}"
        );
        actual.entry((*suffix).to_string()).or_default().push(line);
    }

    for lines in actual.values_mut() {
        lines.sort_unstable();
    }
    let expected = expected
        .iter()
        .map(|case| (case.file_suffix.to_string(), case.lines.to_vec()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual, expected,
        "unexpected source-line fanout for {label}"
    );
    assert_no_traversal_candidates_for_sites(db, &sites, label)?;

    Ok(())
}

fn path_value(path_parts: &[&str]) -> DataValue {
    DataValue::List(
        path_parts
            .iter()
            .map(|part| DataValue::from(*part))
            .collect(),
    )
}

fn source_file_for_suffix(fixture: &FixtureDb, file_path: &str, suffix: &str) -> PathBuf {
    if let Some(root) = github_corpus_root(fixture) {
        let source_file = root.join(suffix);
        if source_file.exists() {
            return source_file;
        }
    }

    let db_file = Path::new(file_path);
    if db_file.exists() {
        return db_file.to_path_buf();
    }

    panic!(
        "could not find source file for fixture {} suffix {suffix:?}; db path was {file_path:?}",
        fixture.id
    );
}

fn github_corpus_root(fixture: &FixtureDb) -> Option<PathBuf> {
    match fixture.creation {
        FixtureCreationStrategy::Automated(
            FixtureAutomation::GithubCorpusCrate {
                checkout_slug, rev, ..
            }
            | FixtureAutomation::GithubCorpusCrateOpenRouterEmbeddings {
                checkout_slug, rev, ..
            }
            | FixtureAutomation::GithubCorpusWorkspaceTargets {
                checkout_slug, rev, ..
            }
            | FixtureAutomation::GithubCorpusWorkspaceTargetsOpenRouterEmbeddings {
                checkout_slug,
                rev,
                ..
            },
        ) => Some(
            backup_db_snapshot_fixture_dir()
                .join("_source_cache")
                .join("corpus")
                .join("checkouts")
                .join(checkout_slug)
                .join(rev),
        ),
        _ => None,
    }
}

fn span_start(value: &DataValue) -> u32 {
    let DataValue::List(values) = value else {
        panic!("call_site.span should be a start/end list: {value:?}");
    };
    let Some(DataValue::Num(cozo::Num::Int(start))) = values.first() else {
        panic!("call_site.span should start with an integer byte offset: {value:?}");
    };

    *start as u32
}

fn line_for_byte(source: &str, byte: u32) -> u32 {
    let byte = byte as usize;
    assert!(
        byte <= source.len(),
        "span byte offset {byte} should be inside source file of length {}",
        source.len()
    );

    source.as_bytes()[..byte]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count() as u32
        + 1
}
