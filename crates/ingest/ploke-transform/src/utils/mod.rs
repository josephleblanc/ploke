use syn_parser::utils::LogStyle;

#[cfg(test)]
pub(crate) mod test_utils;

pub(crate) fn log_db_result(db_result: cozo::NamedRows) {
    log::info!(target: "db",
        "{} {:?}",
        "  Db return: ".log_step(),
        db_result,
    );
}
