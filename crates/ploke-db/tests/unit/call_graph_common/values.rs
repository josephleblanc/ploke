use cozo::{DataValue, UuidWrapper};
use uuid::Uuid;

pub(super) fn uuid(value: Uuid) -> DataValue {
    DataValue::Uuid(UuidWrapper(value))
}

pub(super) fn span((start, end): (i64, i64)) -> DataValue {
    DataValue::List(vec![DataValue::from(start), DataValue::from(end)])
}

pub(in crate::unit) fn list(items: &[&str]) -> DataValue {
    DataValue::List(items.iter().map(|item| DataValue::from(*item)).collect())
}

pub(super) fn option_list(items: Option<Vec<&str>>) -> DataValue {
    items.map(|items| list(&items)).unwrap_or(DataValue::Null)
}

pub(super) fn option_str(value: Option<&str>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}

pub(super) fn option_int(value: Option<i64>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}
