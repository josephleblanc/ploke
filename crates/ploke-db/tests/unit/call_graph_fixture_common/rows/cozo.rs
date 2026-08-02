use super::*;

pub(in crate::unit) fn data_str<'a>(value: &'a DataValue, label: &str) -> &'a str {
    value
        .get_str()
        .unwrap_or_else(|| panic!("{label} should be a string, got {value:?}"))
}

pub(in crate::unit) fn optional_data_str<'a>(value: &'a DataValue, label: &str) -> Option<&'a str> {
    match value {
        DataValue::Null => None,
        other => Some(data_str(other, label)),
    }
}
