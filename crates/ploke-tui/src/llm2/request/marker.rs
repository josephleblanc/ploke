use serde::{Deserialize, Serialize};

// Marker for response_format -> { "type": "json_object" }
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct JsonObjMarker;

impl Serialize for JsonObjMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("type", "json_object")?;
        map.end()
    }
}
