use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RuntimeApi {
    pub application_version: String,
    pub api_version: u32,
    pub classes: Vec<Class>,
    pub events: Vec<Event>,
    pub defines: Vec<Define>,
    pub global_objects: Vec<GlobalObject>,
    #[serde(default)]
    pub global_functions: Vec<Method>,
    #[serde(default)]
    pub concepts: Vec<Concept>,
}

#[derive(Debug, Deserialize)]
pub struct Class {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub r#abstract: bool,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub methods: Vec<Method>,
    #[serde(default)]
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Deserialize)]
pub struct Event {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(default)]
    pub data: Vec<Parameter>,
}

#[derive(Debug, Deserialize)]
pub struct Concept {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "type")]
    pub type_name: ApiType,
}

#[derive(Debug, Deserialize)]
pub struct Define {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub values: Vec<DefineValue>,
    #[serde(default)]
    pub subkeys: Vec<Define>,
}

#[derive(Debug, Deserialize)]
pub struct DefineValue {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct GlobalObject {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "type")]
    pub type_name: ApiType,
}

#[derive(Debug, Deserialize)]
pub struct Method {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    #[serde(default)]
    pub return_values: Vec<Parameter>,
    #[serde(default)]
    pub format: MethodFormat,
}

#[derive(Debug, Default, Deserialize)]
pub struct MethodFormat {
    #[serde(default)]
    pub takes_table: bool,
}

#[derive(Debug, Deserialize)]
pub struct Attribute {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub read_type: Option<ApiType>,
    #[serde(default)]
    pub write_type: Option<ApiType>,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Deserialize)]
pub struct Parameter {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "type")]
    pub type_name: ApiType,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct ApiType(pub serde_json::Value);

impl ApiType {
    pub fn as_simple_name(&self) -> Option<&str> {
        self.0.as_str()
    }

    pub fn complex_type(&self) -> Option<&str> {
        self.0.get("complex_type").and_then(|value| value.as_str())
    }

    pub fn child_type(&self, key: &str) -> Option<ApiType> {
        self.0.get(key).cloned().map(ApiType)
    }

    pub fn options(&self) -> Vec<ApiType> {
        self.0
            .get("options")
            .and_then(|value| value.as_array())
            .map(|values| values.iter().cloned().map(ApiType).collect())
            .unwrap_or_default()
    }

    /// Element types of a `tuple` complex type (from the `"values"` array).
    pub fn tuple_values(&self) -> Vec<ApiType> {
        self.0
            .get("values")
            .and_then(|v| v.as_array())
            .map(|values| values.iter().cloned().map(ApiType).collect())
            .unwrap_or_default()
    }

    /// For a `literal` complex type, returns the primitive kind: `"string"`,
    /// `"number"`, or `"boolean"`, based on the JSON type of the `"value"` field.
    ///
    /// Returns `None` for non-literal types (e.g. `array`, `union`) even if they
    /// happen to have a `"value"` key - the `complex_type` must be `"literal"`.
    pub fn literal_kind(&self) -> Option<&'static str> {
        // Guard: only `literal` complex types are valid here.
        if self.complex_type() != Some("literal") {
            return None;
        }
        let value = self.0.get("value")?;
        if value.is_string() {
            Some("string")
        } else if value.is_number() {
            Some("number")
        } else if value.is_boolean() {
            Some("boolean")
        } else {
            None
        }
    }

    /// Parameters of a `table` complex type, as `(name, type, optional)` triples.
    pub fn parameters(&self) -> Vec<(String, ApiType, bool)> {
        self.0
            .get("parameters")
            .and_then(|value| value.as_array())
            .map(|params| {
                params
                    .iter()
                    .filter_map(|p| {
                        let name = p.get("name")?.as_str()?.to_string();
                        let ty = ApiType(p.get("type")?.clone());
                        let optional = p.get("optional").and_then(|v| v.as_bool()).unwrap_or(false);
                        Some((name, ty, optional))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Non-`nil` arms of a `union` complex type.
    pub fn non_nil_options(&self) -> Vec<ApiType> {
        self.options()
            .into_iter()
            .filter(|option| option.as_simple_name() != Some("nil"))
            .collect()
    }

    /// Whether this type is a `union` whose non-nil arms are all string literals.
    pub fn is_homog_string_literal_union(&self) -> bool {
        if self.complex_type() != Some("union") {
            return false;
        }
        let non_nil = self.non_nil_options();
        !non_nil.is_empty()
            && non_nil
                .iter()
                .all(|option| option.literal_kind() == Some("string"))
    }

    /// String values of a homogeneous string-literal union, in Factorio order.
    ///
    /// Returns an empty vec when this is not a homog string-literal union.
    pub fn string_literal_values(&self) -> Vec<String> {
        if !self.is_homog_string_literal_union() {
            return Vec::new();
        }
        self.non_nil_options()
            .into_iter()
            .filter_map(|option| {
                option
                    .0
                    .get("value")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .collect()
    }

    /// Whether this union includes a `nil` arm.
    pub fn union_has_nil(&self) -> bool {
        self.complex_type() == Some("union")
            && self
                .options()
                .iter()
                .any(|option| option.as_simple_name() == Some("nil"))
    }
}

#[cfg(test)]
mod tests {
    use super::ApiType;

    fn parse(json: &str) -> ApiType {
        ApiType(serde_json::from_str(json).expect("json"))
    }

    #[test]
    fn detects_homog_string_literal_union() {
        let ty = parse(
            r#"{
                "complex_type": "union",
                "options": [
                    {"complex_type": "literal", "value": "left"},
                    {"complex_type": "literal", "value": "right"},
                    "nil"
                ]
            }"#,
        );
        assert!(ty.is_homog_string_literal_union());
        assert!(ty.union_has_nil());
        assert_eq!(
            ty.string_literal_values(),
            vec!["left".to_string(), "right".to_string()]
        );
    }

    #[test]
    fn rejects_heterogeneous_union() {
        let ty = parse(
            r#"{
                "complex_type": "union",
                "options": ["LuaEntity", "LuaEquipment"]
            }"#,
        );
        assert!(!ty.is_homog_string_literal_union());
        assert!(ty.string_literal_values().is_empty());
    }

    #[test]
    fn rejects_non_union() {
        let ty = parse(r#""string""#);
        assert!(!ty.is_homog_string_literal_union());
    }
}
