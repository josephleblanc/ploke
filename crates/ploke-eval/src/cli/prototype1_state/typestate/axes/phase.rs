macro_rules! phase_marker {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct $name;
    };
}

phase_marker!(R0, "R0: CLI dispatch / failure hook.");
phase_marker!(R1, "R1: prelude and turn coordinates.");
phase_marker!(R2a, "R2a: gen0 identity init terminal branch.");
phase_marker!(R3, "R3: parent identity source resolved.");
phase_marker!(R4a, "R4a: ParentIdentity -> Parent<Unchecked>.");
phase_marker!(R4b, "R4b: genesis checkout checked.");
phase_marker!(R4c, "R4c: parent startup complete as Parent<Ready>.");
phase_marker!(R5, "R5: parent-start evidence recorded.");
phase_marker!(R6, "R6: parent baseline established.");
phase_marker!(R7, "R7: planning policy and child budget ready.");
phase_marker!(R8, "R8: child plan resolved and parent selectable.");
phase_marker!(R9, "R9: child schedule shaped.");
phase_marker!(R10, "R10: selection strategy ready.");
phase_marker!(R11a, "R11a: rejected-only child branch.");
phase_marker!(R11, "R11b/c: child fanout complete.");
phase_marker!(R12, "R12: report projection facts ready.");
phase_marker!(R13a, "R13a: successor continuation stopped.");
phase_marker!(R13b, "R13b: successor handoff committed.");
phase_marker!(R14, "R14: final report emitted.");
