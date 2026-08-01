use crate::cli::{
    ConversationsCommand, ConversationsOutputFormat, InspectOutputFormat, SelectAttemptCommand,
    SelectBatchCommand, SelectCampaignCommand, SelectClearCommand, SelectInstanceCommand,
    SelectStatusCommand, SelectUnsetCommand, TranscriptCommand, print_record_resolution_footer,
    print_selection_update, resolve_record_path,
};
use crate::projection::OperatorProjectionRead;
use crate::record::read_compressed_record;
use crate::run_history::print_assistant_messages_from_record_path;
use crate::selection::{
    clear_active_selection, load_active_selection, render_selection_warnings,
    save_active_selection, unset_active_selection_slot,
};
use crate::spec::PrepareError;

impl TranscriptCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(None, self.instance, None)?;
        print_assistant_messages_from_record_path(&resolution.record_path).await?;
        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ConversationsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();

        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        match self.format {
            ConversationsOutputFormat::Table => {
                println!(
                    "{:<6} {:<24} {:<24} {:<12} {}",
                    "Turn", "Started", "Ended", "Tools", "Outcome"
                );
                println!("{}", "-".repeat(80));
                for turn in record.conversations() {
                    let tool_count = turn.tool_calls().len();
                    let outcome_str = match &turn.outcome {
                        crate::record::TurnOutcome::ToolCalls { count } => {
                            format!("tool_calls({})", count)
                        }
                        crate::record::TurnOutcome::Content => "content".to_string(),
                        crate::record::TurnOutcome::Error { message } => {
                            format!("error: {}", message.chars().take(40).collect::<String>())
                        }
                        crate::record::TurnOutcome::Timeout { elapsed_secs } => {
                            format!("timeout({}s)", elapsed_secs)
                        }
                    };
                    println!(
                        "{:<6} {:<24} {:<24} {:<12} {}",
                        turn.turn_number,
                        turn.started_at.chars().take(23).collect::<String>(),
                        turn.ended_at.chars().take(23).collect::<String>(),
                        tool_count,
                        outcome_str
                    );
                }
                println!("\nTotal turns: {}", record.conversations().count());
            }
            ConversationsOutputFormat::Json => {
                let turns: Vec<_> = record.conversations().collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&turns).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl SelectStatusCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let selection = load_active_selection(OperatorProjectionRead::cli_operator())?;
        let warnings = render_selection_warnings(&selection);
        match self.format {
            InspectOutputFormat::Table => {
                println!("active selection");
                println!("{}", "-".repeat(40));
                println!(
                    "campaign: {}",
                    selection
                        .campaign
                        .as_ref()
                        .map(|id| id.as_str())
                        .unwrap_or("(none)")
                );
                println!("batch: {}", selection.batch.as_deref().unwrap_or("(none)"));
                println!(
                    "instance: {}",
                    selection.instance.as_deref().unwrap_or("(none)")
                );
                println!(
                    "attempt: {}",
                    selection
                        .attempt
                        .map(|attempt| attempt.to_string())
                        .unwrap_or_else(|| "(latest)".to_string())
                );
                for warning in warnings {
                    println!("warning: {warning}");
                }
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "selection": selection,
                    "warnings": warnings,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl SelectCampaignCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let mut selection = load_active_selection(OperatorProjectionRead::cli_operator())?;
        selection.campaign = Some(self.campaign);
        save_active_selection(&selection)?;
        print_selection_update(&selection);
        Ok(())
    }
}

impl SelectBatchCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let mut selection = load_active_selection(OperatorProjectionRead::cli_operator())?;
        selection.batch = Some(self.batch);
        save_active_selection(&selection)?;
        print_selection_update(&selection);
        Ok(())
    }
}

impl SelectInstanceCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let mut selection = load_active_selection(OperatorProjectionRead::cli_operator())?;
        selection.instance = Some(self.instance);
        selection.attempt = None;
        save_active_selection(&selection)?;
        print_selection_update(&selection);
        Ok(())
    }
}

impl SelectAttemptCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        if self.attempt == 0 {
            return Err(PrepareError::DatabaseSetup {
                phase: "select_attempt",
                detail: "attempt numbers are 1-based".to_string(),
            });
        }
        let mut selection = load_active_selection(OperatorProjectionRead::cli_operator())?;
        if let Some(instance) = self.instance {
            selection.instance = Some(instance);
        }
        if selection.instance.is_none() {
            return Err(PrepareError::DatabaseSetup {
                phase: "select_attempt",
                detail:
                    "attempt selection requires an active instance (set one first or pass --instance)"
                        .to_string(),
            });
        }
        selection.attempt = Some(self.attempt);
        save_active_selection(&selection)?;
        print_selection_update(&selection);
        Ok(())
    }
}

impl SelectUnsetCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        unset_active_selection_slot(self.scope)?;
        let selection = load_active_selection(OperatorProjectionRead::cli_operator())?;
        print_selection_update(&selection);
        Ok(())
    }
}

impl SelectClearCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        clear_active_selection()?;
        let selection = load_active_selection(OperatorProjectionRead::cli_operator())?;
        print_selection_update(&selection);
        Ok(())
    }
}
