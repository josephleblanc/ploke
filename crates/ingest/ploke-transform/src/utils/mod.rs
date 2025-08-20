use cozo::format_error_as_json;
use serde_json::to_string_pretty;
use syn_parser::utils::LogStyle;

use crate::error::TransformError;

// #[cfg(test)]
// pub(crate) mod test_utils;

pub(crate) fn log_db_result(db_result: cozo::NamedRows) {
    tracing::info!(target: "db",
        "{} {:?}",
        "  Db return: ".log_step(),
        db_result,
    );
}

pub fn log_db_error(e: cozo::Error) -> TransformError {
    tracing::error!(target: "db", "{}", to_string_pretty(&format_error_as_json(e, None)).unwrap());
    TransformError::Database("when executing against relation".to_string())
}
