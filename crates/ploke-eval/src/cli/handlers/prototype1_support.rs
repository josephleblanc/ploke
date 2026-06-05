use std::time::Instant;

use chrono::Utc;

use crate::cli::Prototype1LoopStopAfter;

pub(crate) struct TimingTrace;

pub(crate) struct TimingScope {
    label: String,
    started_at: Instant,
}

impl TimingTrace {
    fn mark(label: &str) {
        #[cfg(not(feature = "demo"))]
        eprintln!("{} {}", Utc::now().format("%H:%M:%S"), label);
        #[cfg(feature = "demo")]
        let _ = label;
    }

    pub(crate) fn scope(label: impl Into<String>) -> TimingScope {
        let label = label.into();
        Self::mark(&format!("{label}.start"));
        TimingScope {
            label,
            started_at: Instant::now(),
        }
    }
}

impl Drop for TimingScope {
    fn drop(&mut self) {
        #[cfg(not(feature = "demo"))]
        eprintln!(
            "{} {}.end +{:.3}s",
            Utc::now().format("%H:%M:%S"),
            self.label,
            self.started_at.elapsed().as_secs_f64()
        );
        #[cfg(feature = "demo")]
        let _ = (&self.label, self.started_at);
    }
}

const REMAINING_PROTOTYPE1_STAGES: &[&str] = &[
    "baseline protocol",
    "target selection",
    "intervention apply",
    "treatment arm",
    "compare",
];

pub(crate) fn pending_prototype1_stages(
    stage_reached: Prototype1LoopStopAfter,
) -> Vec<&'static str> {
    let start = match stage_reached {
        Prototype1LoopStopAfter::BaselineEval => 0,
        Prototype1LoopStopAfter::BaselineProtocol => 1,
        Prototype1LoopStopAfter::TargetSelection => 2,
        Prototype1LoopStopAfter::InterventionApply => 3,
        Prototype1LoopStopAfter::Compare => REMAINING_PROTOTYPE1_STAGES.len(),
    };
    REMAINING_PROTOTYPE1_STAGES[start..].to_vec()
}
