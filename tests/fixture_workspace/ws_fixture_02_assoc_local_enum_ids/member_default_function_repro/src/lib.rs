pub struct RunOpts;

impl Default for RunOpts {
    fn default() -> Self {
        RunOpts
    }
}

pub struct CompileOpts;

impl Default for CompileOpts {
    fn default() -> Self {
        CompileOpts
    }
}

pub fn exercise_fixture() -> (RunOpts, CompileOpts) {
    (RunOpts::default(), CompileOpts::default())
}
