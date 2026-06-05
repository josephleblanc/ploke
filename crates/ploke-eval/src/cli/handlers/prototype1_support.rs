use std::time::Instant;

use chrono::Utc;

use crate::cli::Prototype1LoopStopAfter;

pub(crate) struct TimingTrace;

pub(crate) struct TimingScope {
    label: String,
    started_at: Instant,
}

impl TimingTrace {
    pub(crate) fn mark(label: &str) {
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

pub(crate) fn pending_prototype1_stages(
    stage_reached: Prototype1LoopStopAfter,
) -> Vec<&'static str> {
    match stage_reached {
        Prototype1LoopStopAfter::BaselineEval => {
            vec![
                "baseline protocol",
                "target selection",
                "intervention apply",
                "treatment arm",
                "compare",
            ]
        }
        Prototype1LoopStopAfter::BaselineProtocol => {
            vec![
                "target selection",
                "intervention apply",
                "treatment arm",
                "compare",
            ]
        }
        Prototype1LoopStopAfter::TargetSelection => {
            vec!["intervention apply", "treatment arm", "compare"]
        }
        Prototype1LoopStopAfter::InterventionApply => vec!["treatment arm", "compare"],
        Prototype1LoopStopAfter::Compare => Vec::new(),
    }
}
