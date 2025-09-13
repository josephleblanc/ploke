use serde::{Deserialize, Serialize};

// Marker for response_format -> { "type": "json_object" }
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

impl<'de> Deserialize<'de> for JsonObjMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct JsonObjMarkerVisitor;

        impl<'de> Visitor<'de> for JsonObjMarkerVisitor {
            type Value = JsonObjMarker;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map with 'type' key set to 'json_object'")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut found_type = false;
                
                while let Some(key) = map.next_key::<String>()? {
                    if key == "type" {
                        let value: String = map.next_value()?;
                        if value == "json_object" {
                            found_type = true;
                        } else {
                            return Err(de::Error::custom(
                                format!("expected 'json_object', got '{}'", value)
                            ));
                        }
                    } else {
                        // Skip any other keys
                        let _: serde::de::IgnoredAny = map.next_value()?;
                    }
                }
                
                if found_type {
                    Ok(JsonObjMarker)
                } else {
                    Err(de::Error::missing_field("type"))
                }
            }
        }

        deserializer.deserialize_map(JsonObjMarkerVisitor)
    }
}
