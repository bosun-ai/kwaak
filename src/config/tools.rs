//! Configuration for enabling and disabling tools

use std::{collections::HashMap, ops::Deref};

use serde::{
    de::{MapAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

#[derive(Debug, Default, Clone)]
pub struct Tools(HashMap<String, bool>);

impl Deref for Tools {
    type Target = HashMap<String, bool>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Serialize for Tools {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_some(&self.0)
    }
}

impl<'de> Deserialize<'de> for Tools {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ToolsVisitor;

        impl<'de> Visitor<'de> for ToolsVisitor {
            type Value = Tools;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map of tool names to their enabled/disabled status")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut tools = HashMap::new();

                while let Some((key, value)) = map.next_entry::<String, bool>()? {
                    tools.insert(key, value);
                }

                Ok(Tools(tools))
            }
        }

        deserializer.deserialize_map(ToolsVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_serialize_tools() {
        let mut tools = HashMap::new();
        tools.insert("tool1".to_string(), true);
        tools.insert("tool2".to_string(), false);
        let tools = Tools(tools);

        let serialized = serde_json::to_string(&tools).unwrap();
        assert_eq!(serialized, r#"{"tool1":true,"tool2":false}"#);
    }

    #[test]
    fn test_deserialize_tools() {
        let json = r#"{"tool1":true,"tool2":false}"#;
        let tools: Tools = serde_json::from_str(json).unwrap();

        assert_eq!(tools.0.get("tool1"), Some(&true));
        assert_eq!(tools.0.get("tool2"), Some(&false));
    }
}
