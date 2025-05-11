use super::*;

pub trait HasId {
    fn to_cozo_id(self) -> DataValue;
}

pub trait HasName {
    fn to_cozo_name(self) -> DataValue;
}

pub trait HasSpan {
    fn to_cozo_span(self) -> DataValue;
}
