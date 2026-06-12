//! 2512.07921 DeepCode-inspired static report fixtures.
//!
//! These helpers normalize tiny hand-checkable artifacts into report-only
//! diagnostics. They do not select candidates, admit patches, score agents, or
//! mutate Ploke History/Crown/route/model/budget authority.

use std::collections::BTreeSet;

type Path = &'static str;
type Tuple = (&'static str, &'static str, &'static str);

#[derive(Debug, PartialEq)]
pub struct BlueprintReport {
    pub required_coverage: f64,
    pub files: Vec<Path>,
    pub file_precision: f64,
    pub file_recall: f64,
    pub validation_present: bool,
    pub fallback_rejected: bool,
    pub authority_violations: usize,
}

#[derive(Debug, PartialEq)]
pub struct CodeMemReport {
    pub implemented_files: Vec<Path>,
    pub unimplemented_files: Vec<Path>,
    pub summary_coverage: f64,
    pub file_delta: isize,
    pub unimplemented_exact: bool,
    pub interface_errors: usize,
    pub stale_claims: usize,
    pub next_step_persisted: bool,
}

#[derive(Debug, PartialEq)]
pub struct CodeRagReport {
    pub tuple_precision: f64,
    pub tuple_recall: f64,
    pub forbidden_count: usize,
    pub unknown_type_count: usize,
    pub confidence_authority_violation: bool,
}

#[derive(Debug, PartialEq)]
pub struct BoundaryReport {
    pub loop_warning: bool,
    pub unique_completed: usize,
    pub total_planned: usize,
    pub verification_failures: usize,
    pub proposals_without_admission: usize,
    pub admission_violations: usize,
    pub selector_violations: usize,
    pub report_only: bool,
}

struct Summary {
    core_purpose: Option<&'static str>,
    functions: &'static [&'static str],
    constants_or_types: &'static [&'static str],
    internal_dependencies: &'static [&'static str],
    external_dependencies: &'static [&'static str],
    implementation_notes: &'static [&'static str],
}

const REQUIRED_SECTIONS: &[&str] = &[
    "file_structure",
    "implementation_components",
    "validation_approach",
    "environment_setup",
    "implementation_strategy",
];

const FALLBACK_FILES: &[Path] = &[
    "README.md",
    "src/main.py",
    "src/pipeline.py",
    "tests/test_pipeline.py",
];

/// Report deterministic checks for the blueprint mini fixture.
pub fn blueprint_fixture_report() -> BlueprintReport {
    let sections = REQUIRED_SECTIONS;
    let expected_files = ["src/lib.rs", "tests/greet.rs"];
    let candidate_files = ["src/lib.rs", "tests/greet.rs"];
    let commands = ["cargo test -p greeter"];
    let candidate = path_set(&candidate_files);
    let expected = path_set(&expected_files);
    let matched = candidate.intersection(&expected).count();

    BlueprintReport {
        required_coverage: ratio(
            REQUIRED_SECTIONS
                .iter()
                .filter(|section| sections.contains(section))
                .count(),
            REQUIRED_SECTIONS.len(),
        ),
        files: sorted(&candidate),
        file_precision: ratio(matched, candidate.len()),
        file_recall: ratio(matched, expected.len()),
        validation_present: commands.iter().any(|command| local_validation(command)),
        fallback_rejected: FALLBACK_FILES
            .iter()
            .all(|path| !candidate_files.contains(path)),
        authority_violations: 0,
    }
}

/// Report deterministic checks for the compact CodeMem ledger fixture.
pub fn codemem_fixture_report() -> CodeMemReport {
    let all_files = ["src/lib.rs", "src/parser.rs", "tests/parser.rs"];
    let writes = [
        (
            "src/lib.rs",
            Summary {
                core_purpose: Some("crate API re-exporting parse_cfg"),
                functions: &[],
                constants_or_types: &[],
                internal_dependencies: &["src/parser.rs::parse_cfg"],
                external_dependencies: &["tests/parser.rs"],
                implementation_notes: &["exports parser result"],
            },
        ),
        (
            "src/parser.rs",
            Summary {
                core_purpose: Some("parse key=value text into Config"),
                functions: &["parse_cfg(input: &str) -> Result<Config, ParseError>"],
                constants_or_types: &["Config", "ParseError"],
                internal_dependencies: &[],
                external_dependencies: &["src/lib.rs", "tests/parser.rs"],
                implementation_notes: &["empty input is an error"],
            },
        ),
    ];
    let implemented = writes
        .iter()
        .map(|(file, _)| *file)
        .collect::<BTreeSet<_>>();
    let all = path_set(&all_files);
    let unimplemented = all
        .difference(&implemented)
        .copied()
        .collect::<BTreeSet<_>>();
    let oracle_writes = path_set(&["src/lib.rs", "src/parser.rs"]);
    let expected_unimplemented = path_set(&["tests/parser.rs"]);
    let sections_present = writes
        .iter()
        .map(|(_, summary)| summary_sections(summary))
        .sum::<usize>();

    CodeMemReport {
        implemented_files: sorted(&implemented),
        unimplemented_files: sorted(&unimplemented),
        summary_coverage: ratio(sections_present, writes.len() * 5),
        file_delta: implemented.len() as isize - oracle_writes.len() as isize,
        unimplemented_exact: unimplemented == expected_unimplemented,
        interface_errors: interface_errors(&writes),
        stale_claims: stale_claims(&writes, &all),
        next_step_persisted: false,
    }
}

/// Report deterministic checks for hand-labeled CodeRAG relationship tuples.
pub fn coderag_fixture_report() -> CodeRagReport {
    let oracle = tuple_set(&[
        ("reference/src/parser.rs", "src/parser.rs", "direct_match"),
        ("reference/src/graph.rs", "src/model.rs", "partial_match"),
    ]);
    let candidate = [
        ("reference/src/parser.rs", "src/parser.rs", "direct_match"),
        ("reference/src/graph.rs", "src/model.rs", "partial_match"),
    ];
    let candidate = tuple_set(&candidate);
    let correct = candidate.intersection(&oracle).count();

    CodeRagReport {
        tuple_precision: ratio(correct, candidate.len()),
        tuple_recall: ratio(correct, oracle.len()),
        forbidden_count: candidate
            .iter()
            .filter(|tuple| forbidden_tuple(tuple))
            .count(),
        unknown_type_count: candidate
            .iter()
            .filter(|(_, _, relation)| !allowed_relation(relation))
            .count(),
        confidence_authority_violation: false,
    }
}

/// Report deterministic stagnation and verification/admission boundary checks.
pub fn boundary_fixture_report() -> BoundaryReport {
    let planned_files = ["src/lib.rs", "tests/lib.rs"];
    let trace = [
        ("read_code_mem", Some("src/lib.rs"), "success"),
        ("read_code_mem", Some("src/lib.rs"), "success"),
        ("read_code_mem", Some("src/lib.rs"), "success"),
        ("read_code_mem", Some("src/lib.rs"), "success"),
        ("read_code_mem", Some("src/lib.rs"), "success"),
        ("write_file", Some("src/lib.rs"), "success"),
        ("execute_bash", Some("cargo test"), "error"),
    ];
    let verification_exit = [101];
    let proposals = [("p1", ["v1"] as [&str; 1])];
    let admitted: [&str; 0] = [];
    let completed = trace
        .iter()
        .filter_map(|(tool, target, status)| {
            (*tool == "write_file" && *status == "success").then_some(target.as_ref()?)
        })
        .filter(|path| planned_files.contains(path))
        .copied()
        .collect::<BTreeSet<_>>();

    BoundaryReport {
        loop_warning: repeated_tool_warning(&trace, 5),
        unique_completed: completed.len(),
        total_planned: planned_files.len(),
        verification_failures: verification_exit.iter().filter(|code| **code != 0).count(),
        proposals_without_admission: proposals
            .iter()
            .filter(|(proposal, evidence)| !admitted.contains(proposal) && evidence == &["v1"])
            .count(),
        admission_violations: admitted
            .iter()
            .filter(|admitted_id| verification_exit[0] != 0 && **admitted_id == "v1")
            .count(),
        selector_violations: 0,
        report_only: true,
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn path_set(items: &[Path]) -> BTreeSet<Path> {
    items.iter().copied().collect()
}

fn tuple_set(items: &[Tuple]) -> BTreeSet<Tuple> {
    items.iter().copied().collect()
}

fn sorted(items: &BTreeSet<Path>) -> Vec<Path> {
    items.iter().copied().collect()
}

fn local_validation(command: &str) -> bool {
    !command.is_empty()
        && ["cargo test", "cargo nextest", "cargo check"]
            .iter()
            .any(|prefix| command.starts_with(prefix))
        && !["docker", "curl", "http://", "https://", "OPENAI_API_KEY"]
            .iter()
            .any(|forbidden| command.contains(forbidden))
}

fn summary_sections(summary: &Summary) -> usize {
    let public_interface = summary.functions.iter().all(|item| !item.is_empty())
        && summary
            .constants_or_types
            .iter()
            .all(|item| !item.is_empty());
    let external_deps = summary
        .external_dependencies
        .iter()
        .all(|item| !item.is_empty());

    usize::from(summary.core_purpose.is_some())
        + usize::from(public_interface)
        + 1
        + usize::from(external_deps)
        + usize::from(!summary.implementation_notes.is_empty())
}

fn interface_errors(writes: &[(Path, Summary)]) -> usize {
    writes
        .iter()
        .flat_map(|(_, summary)| summary.internal_dependencies.iter())
        .filter(|dependency| !dependency_signature_exists(dependency, writes))
        .count()
}

fn dependency_signature_exists(dependency: &str, writes: &[(Path, Summary)]) -> bool {
    let Some((file, function)) = dependency.rsplit_once("::") else {
        return false;
    };
    writes.iter().any(|(written_file, summary)| {
        *written_file == file
            && summary
                .functions
                .iter()
                .any(|signature| signature.starts_with(&format!("{function}(")))
    })
}

fn stale_claims(writes: &[(Path, Summary)], all_files: &BTreeSet<Path>) -> usize {
    writes
        .iter()
        .flat_map(|(_, summary)| summary.internal_dependencies.iter())
        .filter_map(|dependency| dependency.rsplit_once("::").map(|(file, _)| file))
        .filter(|file| !all_files.contains(file))
        .count()
}

fn forbidden_tuple((repo_file, target_file, _): &Tuple) -> bool {
    *repo_file == "reference/src/io.rs" && matches!(*target_file, "src/parser.rs" | "src/model.rs")
}

fn allowed_relation(relation: &str) -> bool {
    matches!(
        relation,
        "direct_match" | "partial_match" | "reference" | "utility" | "none"
    )
}

fn repeated_tool_warning(trace: &[(&str, Option<&str>, &str)], max_repeats: usize) -> bool {
    let mut previous = None;
    let mut run = 0;
    for (tool, _, _) in trace {
        if Some(*tool) == previous {
            run += 1;
        } else {
            previous = Some(*tool);
            run = 1;
        }
        if run >= max_repeats {
            return true;
        }
    }
    false
}
