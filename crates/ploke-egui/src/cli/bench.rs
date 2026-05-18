//! Dev-only benchmark command parsing.

use std::path::PathBuf;

#[cfg(feature = "dev")]
use clap::{ArgGroup, Args as ClapArgs};

#[derive(Debug)]
pub enum BenchRun {
    AllocationBreakdown { report_or_dir: Option<PathBuf> },
}

#[cfg(feature = "dev")]
#[derive(Debug, ClapArgs)]
#[command(group(
    ArgGroup::new("bench_action")
        .required(true)
        .multiple(false)
        .args(["breakdown", "bd"])
))]
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
}

#[cfg(feature = "dev")]
impl BenchArgs {
    pub fn into_run(self) -> BenchRun {
        let selected = if self.breakdown.is_some() {
            self.breakdown
        } else {
            self.bd
        };
        BenchRun::AllocationBreakdown {
            report_or_dir: selected.unwrap_or(None),
        }
    }
}
