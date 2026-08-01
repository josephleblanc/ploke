use crate::cli::{
    JustCommand, JustSubcommand, RunBatchWorkflowCommand, RunBatchWorkflowSubcommand, RunCommand,
    RunDatasetsCommand, RunDatasetsSubcommand, RunPrepareCommand, RunPrepareSubcommand,
    RunReplayCommand, RunReplaySubcommand, RunRepoCommand, RunRepoSubcommand,
    RunSingleWorkflowCommand, RunSingleWorkflowSubcommand, RunSubcommand, SelectCommand,
    SelectSubcommand, print_builtin_dataset_entries,
};
use crate::spec::PrepareError;

impl RunCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunSubcommand::Repo(cmd) => cmd.run().await,
            RunSubcommand::Datasets(cmd) => cmd.run().await,
            RunSubcommand::Prepare(cmd) => cmd.run().await,
            RunSubcommand::List(cmd) => cmd.run().await,
            RunSubcommand::Single(cmd) => cmd.run().await,
            RunSubcommand::Batch(cmd) => cmd.run().await,
            RunSubcommand::Replay(cmd) => cmd.run().await,
        }
    }
}

impl RunRepoCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunRepoSubcommand::Fetch(cmd) => cmd.run(),
        }
    }
}

impl RunDatasetsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunDatasetsSubcommand::List => {
                print_builtin_dataset_entries();
                Ok(())
            }
        }
    }
}

impl RunPrepareCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunPrepareSubcommand::Custom(cmd) => cmd.run(),
            RunPrepareSubcommand::Instance(cmd) => cmd.run(),
            RunPrepareSubcommand::Batch(cmd) => cmd.run(),
        }
    }
}

impl SelectCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            SelectSubcommand::Status(cmd) => cmd.run(),
            SelectSubcommand::Campaign(cmd) => cmd.run(),
            SelectSubcommand::Batch(cmd) => cmd.run(),
            SelectSubcommand::Instance(cmd) => cmd.run(),
            SelectSubcommand::Attempt(cmd) => cmd.run(),
            SelectSubcommand::Unset(cmd) => cmd.run(),
            SelectSubcommand::Clear(cmd) => cmd.run(),
        }
    }
}

impl RunSingleWorkflowCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunSingleWorkflowSubcommand::Setup(cmd) => cmd.run().await,
            RunSingleWorkflowSubcommand::Agent(cmd) => cmd.run().await,
        }
    }
}

impl RunBatchWorkflowCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunBatchWorkflowSubcommand::Setup(cmd) => cmd.run().await,
            RunBatchWorkflowSubcommand::Agent(cmd) => cmd.run().await,
        }
    }
}

impl RunReplayCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunReplaySubcommand::Batch(cmd) => cmd.run().await,
            RunReplaySubcommand::Inspect(cmd) => cmd.run(),
            RunReplaySubcommand::SelfEditLive(cmd) => cmd.run().await,
            RunReplaySubcommand::TurnLive(cmd) => cmd.run().await,
        }
    }
}

impl JustCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            JustSubcommand::FetchRepo(cmd) => cmd.run(),
            JustSubcommand::ListDatasets => {
                print_builtin_dataset_entries();
                Ok(())
            }
            JustSubcommand::PrepareCustom(cmd) => cmd.run(),
            JustSubcommand::PrepareInstance(cmd) => cmd.run(),
            JustSubcommand::PrepareBatch(cmd) => cmd.run(),
            JustSubcommand::SingleSetup(cmd) => cmd.run().await,
            JustSubcommand::Single(cmd) => cmd.run().await,
            JustSubcommand::BatchSetup(cmd) => cmd.run().await,
            JustSubcommand::Batch(cmd) => cmd.run().await,
            JustSubcommand::ReplayBatch(cmd) => cmd.run().await,
        }
    }
}
