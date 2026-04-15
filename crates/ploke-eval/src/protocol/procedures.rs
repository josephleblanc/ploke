mod components;
mod metrics;
mod procedures;

trait ProtocolStep<I, O> {
    fn run(input: I) -> O;
}

trait Executor<Spec, Input, Output> {
    fn execute(spec: Spec, input: Input) -> Output;
}

struct RunId(String);

struct PrintedToolCallLine {
    index: usize,
    summary: String,
}

struct Step1InspectToolCalls;

impl ProtocolStep<RunId, Vec<PrintedToolCallLine>> for Step1InspectToolCalls {
    fn run(input: RunId) -> Vec<PrintedToolCallLine> {
        // conceptually: `ploke-eval inspect tool-calls ...`
        todo!()
    }
}

struct SuspiciousToolCallIndex(usize);

struct Step2SelectSuspiciousCall;

trait LlmJudge<I, O> {
    fn judge(input: I) -> O;
}

struct ChatGpt54;

impl LlmJudge<Vec<PrintedToolCallLine>, SuspiciousToolCallIndex> for ChatGpt54 {
    fn judge(input: Vec<PrintedToolCallLine>) -> SuspiciousToolCallIndex {
        // prompt-governed judgment
        todo!()
    }
}

struct ToolSummary {
    index: usize,
    detail: String,
}

struct Step3InspectToolCallDetail;

impl ProtocolStep<SuspiciousToolCallIndex, ToolSummary> for Step3InspectToolCallDetail {
    fn run(input: SuspiciousToolCallIndex) -> ToolSummary {
        // conceptually: `ploke-eval inspect tool-call {index}`
        todo!()
    }
}
