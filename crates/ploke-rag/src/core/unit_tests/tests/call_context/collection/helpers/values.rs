use super::*;

#[cfg(feature = "call_graph")]
pub(super) fn uuid(value: Uuid) -> DataValue {
    DataValue::Uuid(UuidWrapper(value))
}

#[cfg(feature = "call_graph")]
pub(super) fn span((start, end): (i64, i64)) -> DataValue {
    DataValue::List(vec![DataValue::from(start), DataValue::from(end)])
}

#[cfg(feature = "call_graph")]
pub(super) fn list(items: &[&str]) -> DataValue {
    DataValue::List(items.iter().map(|item| DataValue::from(*item)).collect())
}

#[cfg(feature = "call_graph")]
pub(super) fn option_list(items: Option<Vec<&str>>) -> DataValue {
    items.map(|items| list(&items)).unwrap_or(DataValue::Null)
}

#[cfg(feature = "call_graph")]
pub(super) fn option_str(value: Option<&str>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}

#[cfg(feature = "call_graph")]
pub(super) fn option_int(value: Option<i64>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}
