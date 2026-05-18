//! Dev-only benchmark command parsing.

use std::path::PathBuf;

#[cfg(feature = "dev")]
use clap::{ArgGroup, Args as ClapArgs, Subcommand};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchReportArgs {
    pub report_or_dir: Option<PathBuf>,
    pub short: bool,
    pub fast_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BenchRun {
    AllocationBreakdown(BenchReportArgs),
    AllocationDelta(BenchReportArgs),
}

#[cfg(feature = "dev")]
#[derive(Debug, ClapArgs)]
#[command(
    arg_required_else_help = true,
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true,
    group(
        ArgGroup::new("bench_action")
            .required(true)
            .multiple(false)
            .args(["breakdown", "bd"])
    )
)]
pub struct BenchArgs {
    #[arg(
        long,
        value_name = "REPORT_OR_DIR",
        num_args = 0..=1,
        help = "Print allocation-span breakdown for a benchmark report",
        long_help = "Print allocation-span breakdown for a benchmark report. \
Loads the given report.json file or benchmark output directory, or defaults to \
the most recently modified benchmark report. Rows include allocation counts, \
object and wrapped bytes, percentages of scenario totals, and per-frame means."
    )]
    breakdown: Option<Option<PathBuf>>,

    #[arg(
        long = "bd",
        value_name = "REPORT_OR_DIR",
        num_args = 0..=1,
        help = "Alias for --breakdown",
        long_help = "Alias for --breakdown. Accepts the same optional report.json file \
or benchmark output directory, and defaults to the most recent benchmark report."
    )]
    bd: Option<Option<PathBuf>>,

    #[arg(
        long,
        help = "Only print the first five rows in each rendered benchmark section"
    )]
    short: bool,

    #[arg(
        long,
        help = "Only print 30-frame benchmark scenarios such as inspector phase sequences"
    )]
    fast_only: bool,

    #[command(subcommand)]
    command: Option<BenchCommand>,
}

#[cfg(feature = "dev")]
#[derive(Debug, Subcommand)]
enum BenchCommand {
    #[command(
        alias = "diff",
        about = "Compare the selected benchmark report against the previous matching run"
    )]
    Delta(BenchDeltaArgs),
}

#[cfg(feature = "dev")]
#[derive(Debug, ClapArgs)]
#[command(
    arg_required_else_help = true,
    group(
        ArgGroup::new("bench_delta_action")
            .required(true)
            .multiple(false)
            .args(["breakdown", "bd"])
    )
)]
struct BenchDeltaArgs {
    #[arg(
        long,
        value_name = "REPORT_OR_DIR",
        num_args = 0..=1,
        help = "Compare allocation-span breakdown for a benchmark report",
        long_help = "Compare allocation-span breakdown for a benchmark report against \
the previous report with the same suite and run root. Loads the given report.json file \
or benchmark output directory, or defaults to the most recently modified benchmark report."
    )]
    breakdown: Option<Option<PathBuf>>,

    #[arg(
        long = "bd",
        value_name = "REPORT_OR_DIR",
        num_args = 0..=1,
        help = "Alias for --breakdown"
    )]
    bd: Option<Option<PathBuf>>,

    #[arg(
        long,
        help = "Only print the first five rows in each rendered benchmark section"
    )]
    short: bool,

    #[arg(
        long,
        help = "Only print 30-frame benchmark scenarios such as inspector phase sequences"
    )]
    fast_only: bool,
}

#[cfg(feature = "dev")]
impl BenchArgs {
    pub fn into_run(self) -> BenchRun {
        match self.command {
            Some(BenchCommand::Delta(args)) => BenchRun::AllocationDelta(args.into_report_args()),
            None => BenchRun::AllocationBreakdown(BenchReportArgs {
                report_or_dir: selected_report_or_dir(self.breakdown, self.bd),
                short: self.short,
                fast_only: self.fast_only,
            }),
        }
    }
}

#[cfg(feature = "dev")]
impl BenchDeltaArgs {
    fn into_report_args(self) -> BenchReportArgs {
        BenchReportArgs {
            report_or_dir: selected_report_or_dir(self.breakdown, self.bd),
            short: self.short,
            fast_only: self.fast_only,
        }
    }
}

#[cfg(feature = "dev")]
fn selected_report_or_dir(
    breakdown: Option<Option<PathBuf>>,
    bd: Option<Option<PathBuf>>,
) -> Option<PathBuf> {
    breakdown.or(bd).unwrap_or(None)
}
