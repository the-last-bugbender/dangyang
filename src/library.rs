//! Runtime YANG model library with JSON parsing.
//!
//! [`YangLibrary`] holds any number of named YANG models (each a collection of
//! `typedef` statements).  Given a JSON object whose keys are typedef names,
//! [`YangLibrary::parse`] validates and converts every value into the
//! appropriate [`YangValue`], returning a [`YangObject`] you can query by field
//! name.
//!
//! # Example
//!
//! ```rust
//! use dang_yang::{YangLibrary, YangValue};
//!
//! const YANG: &str = r#"
//!     module netdev {
//!         typedef hostname {
//!             type string;
//!         }
//!         typedef interface-name {
//!             type string;
//!         }
//!         typedef interface-state {
//!             type enumeration {
//!                 enum up;
//!                 enum down;
//!                 enum testing;
//!                 enum unknown;
//!                 enum dormant;
//!                 enum not-present;
//!                 enum lower-layer-down;
//!             }
//!         }
//!         typedef link-duplex {
//!             type enumeration {
//!                 enum full;
//!                 enum half;
//!                 enum auto;
//!             }
//!         }
//!         typedef interface-flags {
//!             type bits {
//!                 bit up           { position 0; }
//!                 bit broadcast    { position 1; }
//!                 bit loopback     { position 2; }
//!                 bit point-to-point { position 3; }
//!                 bit multicast    { position 4; }
//!                 bit promisc      { position 5; }
//!             }
//!         }
//!         typedef port-number {
//!             type uint16;
//!         }
//!         typedef vlan-id {
//!             type uint16;
//!         }
//!         typedef bandwidth-bps {
//!             type uint64;
//!         }
//!         typedef mac-address {
//!             type string;
//!         }
//!     }
//! "#;
//!
//! let mut lib = YangLibrary::new();
//! lib.register_model("netdev", YANG).unwrap();
//!
//! let json = serde_json::json!({
//!     "hostname":        "core-router-01.example.net",
//!     "interface-name":  "GigabitEthernet0/0",
//!     "interface-state": "up",
//!     "link-duplex":     "full",
//!     "interface-flags": ["up", "broadcast", "multicast"],
//!     "port-number":     8080,
//!     "vlan-id":         100,
//!     "bandwidth-bps":   1000000000_u64,
//!     "mac-address":     "aa:bb:cc:dd:ee:ff",
//! });
//!
//! let obj = lib.parse("netdev", &json).unwrap();
//!
//! assert_eq!(obj["hostname"].as_str(),        Some("core-router-01.example.net"));
//! assert_eq!(obj["interface-name"].as_str(),  Some("GigabitEthernet0/0"));
//! assert_eq!(obj["interface-state"].as_str(), Some("up"));
//! assert_eq!(obj["link-duplex"].as_str(),     Some("full"));
//! assert_eq!(obj["port-number"].as_uint(),    Some(8080));
//! assert_eq!(obj["vlan-id"].as_uint(),        Some(100));
//! assert_eq!(obj["bandwidth-bps"].as_uint(),  Some(1_000_000_000));
//! assert_eq!(obj["mac-address"].as_str(),     Some("aa:bb:cc:dd:ee:ff"));
//!
//! let active_flags = obj["interface-flags"].as_bits().unwrap();
//! assert!(active_flags.contains(&"up".to_string()));
//! assert!(active_flags.contains(&"multicast".to_string()));
//! ```

use std::collections::HashMap;
use std::path::Path;

use crate::{
    ast::{Restriction, TypeStmt, TypedefNode},
    error::ParseError,
    parse_file, parse_str,
    value::YangValue,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors produced by [`YangLibrary`] operations.
#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    /// No model with the given name has been registered.
    #[error("model {0:?} is not registered")]
    ModelNotFound(String),

    /// The JSON object contained a key that is not a typedef name in the model.
    #[error("field {field:?} is not a typedef in model {model:?}")]
    TypedefNotFound { model: String, field: String },

    /// A YANG source file could not be parsed.
    #[error("failed to parse YANG source: {0}")]
    ParseError(#[from] ParseError),

    /// A JSON value did not conform to the YANG type for that field.
    #[error("invalid value for field {field:?}: {reason}")]
    InvalidValue { field: String, reason: String },

    /// The top-level JSON value was not an object.
    #[error("expected a JSON object at the top level")]
    NotAnObject,
}

// ---------------------------------------------------------------------------
// YangObject
// ---------------------------------------------------------------------------

/// A parsed YANG model instance — a map from typedef names to [`YangValue`]s.
///
/// Produced by [`YangLibrary::parse`].  Access fields with [`get`](Self::get)
/// or the [`Index`](std::ops::Index) operator.
#[derive(Debug)]
pub struct YangObject {
    fields: HashMap<String, YangValue>,
}

impl YangObject {
    /// Return a reference to the value for `field`, or `None` if absent.
    pub fn get(&self, field: &str) -> Option<&YangValue> {
        self.fields.get(field)
    }

    /// Borrow the underlying field map.
    pub fn fields(&self) -> &HashMap<String, YangValue> {
        &self.fields
    }

    /// Consume the object and return the underlying field map.
    pub fn into_fields(self) -> HashMap<String, YangValue> {
        self.fields
    }

    /// Number of fields in this object.
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns `true` if the object has no fields.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Iterate over `(field_name, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &YangValue)> {
        self.fields.iter().map(|(k, v)| (k.as_str(), v))
    }
}

/// Index by field name — panics if the field is absent, just like `HashMap`.
impl std::ops::Index<&str> for YangObject {
    type Output = YangValue;

    fn index(&self, field: &str) -> &YangValue {
        &self.fields[field]
    }
}

impl IntoIterator for YangObject {
    type Item = (String, YangValue);
    type IntoIter = std::collections::hash_map::IntoIter<String, YangValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

// ---------------------------------------------------------------------------
// YangLibrary
// ---------------------------------------------------------------------------

/// A registry of named YANG models that can parse JSON values at runtime.
///
/// Register one or more models with [`register_model`](Self::register_model) /
/// [`register_model_file`](Self::register_model_file), then call
/// [`parse`](Self::parse) to convert a JSON object into a [`YangObject`].
///
/// For single-field parsing, see [`parse_as`](Self::parse_as).
#[derive(Default)]
pub struct YangLibrary {
    /// model name → index of typedef nodes
    models: HashMap<String, Vec<TypedefNode>>,
}

impl YangLibrary {
    /// Create an empty library.
    pub fn new() -> Self {
        Self::default()
    }

    // ------------------------------------------------------------------
    // Registration
    // ------------------------------------------------------------------

    /// Register a YANG model from a source string.
    ///
    /// Overwrites any previously registered model with the same name.
    pub fn register_model(
        &mut self,
        name: impl Into<String>,
        yang_source: &str,
    ) -> Result<(), ParseError> {
        let typedefs = parse_str(yang_source)?;
        self.models.insert(name.into(), typedefs);
        Ok(())
    }

    /// Register a YANG model from a file on disk.
    ///
    /// Overwrites any previously registered model with the same name.
    pub fn register_model_file(
        &mut self,
        name: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<(), ParseError> {
        let typedefs = parse_file(path)?;
        self.models.insert(name.into(), typedefs);
        Ok(())
    }

    // ------------------------------------------------------------------
    // Introspection
    // ------------------------------------------------------------------

    /// Names of all registered models.
    pub fn model_names(&self) -> impl Iterator<Item = &str> {
        self.models.keys().map(|k| k.as_str())
    }

    /// Typedef names registered under `model_name`, or `None` if the model
    /// does not exist.
    pub fn typedef_names(&self, model_name: &str) -> Option<impl Iterator<Item = &str>> {
        self.models
            .get(model_name)
            .map(|tds| tds.iter().map(|td| td.name.as_str()))
    }

    // ------------------------------------------------------------------
    // Parsing
    // ------------------------------------------------------------------

    /// Parse a JSON object against the named model.
    ///
    /// Every key in `json` must be a typedef name in the model.  Each value is
    /// validated and converted to the appropriate [`YangValue`] variant.
    ///
    /// Fields present in the model but absent from `json` are simply omitted
    /// from the returned [`YangObject`].  Extra keys in `json` that have no
    /// corresponding typedef produce [`LibraryError::TypedefNotFound`].
    pub fn parse(
        &self,
        model_name: &str,
        json: &serde_json::Value,
    ) -> Result<YangObject, LibraryError> {
        let typedefs = self
            .models
            .get(model_name)
            .ok_or_else(|| LibraryError::ModelNotFound(model_name.to_string()))?;

        let by_name: HashMap<&str, &TypedefNode> =
            typedefs.iter().map(|t| (t.name.as_str(), t)).collect();

        let obj = json.as_object().ok_or(LibraryError::NotAnObject)?;

        let mut fields = HashMap::with_capacity(obj.len());

        for (key, value) in obj {
            let typedef =
                by_name
                    .get(key.as_str())
                    .ok_or_else(|| LibraryError::TypedefNotFound {
                        model: model_name.to_string(),
                        field: key.clone(),
                    })?;

            let yang_value = parse_value(value, typedef, &by_name).map_err(|reason| {
                LibraryError::InvalidValue {
                    field: key.clone(),
                    reason,
                }
            })?;

            fields.insert(key.clone(), yang_value);
        }

        Ok(YangObject { fields })
    }

    /// Parse a single JSON value as the named typedef in the named model.
    ///
    /// Useful when you already know which type to apply rather than parsing a
    /// whole object.
    ///
    /// ```rust
    /// # use dang_yang::{YangLibrary, YangValue};
    /// # let mut lib = YangLibrary::new();
    /// # lib.register_model("m", "typedef port-number { type uint16; }").unwrap();
    /// let val = lib.parse_as("m", "port-number", &serde_json::json!(443)).unwrap();
    /// assert_eq!(val, YangValue::UInt(443));
    /// ```
    pub fn parse_as(
        &self,
        model_name: &str,
        typedef_name: &str,
        json: &serde_json::Value,
    ) -> Result<YangValue, LibraryError> {
        let typedefs = self
            .models
            .get(model_name)
            .ok_or_else(|| LibraryError::ModelNotFound(model_name.to_string()))?;

        let by_name: HashMap<&str, &TypedefNode> =
            typedefs.iter().map(|t| (t.name.as_str(), t)).collect();

        let typedef = by_name
            .get(typedef_name)
            .ok_or_else(|| LibraryError::TypedefNotFound {
                model: model_name.to_string(),
                field: typedef_name.to_string(),
            })?;

        parse_value(json, typedef, &by_name).map_err(|reason| LibraryError::InvalidValue {
            field: typedef_name.to_string(),
            reason,
        })
    }
}

// ---------------------------------------------------------------------------
// Internal parsing — free functions
// ---------------------------------------------------------------------------

/// Entry point: parse `json` according to `typedef`'s type statement.
fn parse_value(
    json: &serde_json::Value,
    typedef: &TypedefNode,
    all_typedefs: &HashMap<&str, &TypedefNode>,
) -> Result<YangValue, String> {
    parse_type(json, &typedef.type_stmt, all_typedefs)
}

/// Recursively parse `json` against `type_stmt`.
fn parse_type(
    json: &serde_json::Value,
    type_stmt: &TypeStmt,
    all_typedefs: &HashMap<&str, &TypedefNode>,
) -> Result<YangValue, String> {
    // Strip an optional module prefix (e.g. "ietf-inet-types:string" → "string").
    let type_name = type_stmt.name.as_str();
    let local = type_name
        .rfind(':')
        .map(|i| &type_name[i + 1..])
        .unwrap_or(type_name);

    match local {
        // String-like built-ins
        "string" | "leafref" | "identityref" | "instance-identifier" => json
            .as_str()
            .map(|s| YangValue::Text(s.to_string()))
            .ok_or_else(|| format!("expected a JSON string for YANG type {type_name:?}")),

        "boolean" => json
            .as_bool()
            .map(YangValue::Bool)
            .ok_or_else(|| format!("expected true or false for YANG type {type_name:?}")),

        // Unsigned integers
        "uint8" | "uint16" | "uint32" | "uint64" => json
            .as_u64()
            .map(YangValue::UInt)
            .ok_or_else(|| format!("expected a non-negative integer for YANG type {type_name:?}")),

        // Signed integers
        "int8" | "int16" | "int32" | "int64" => json
            .as_i64()
            .map(YangValue::Int)
            .ok_or_else(|| format!("expected an integer for YANG type {type_name:?}")),

        "decimal64" => json
            .as_f64()
            .map(YangValue::Float)
            .ok_or_else(|| format!("expected a number for YANG type {type_name:?}")),

        // Binary: accept a JSON string and treat its bytes as raw binary.
        // Callers that need base64 decoding can do so with the inner Vec<u8>.
        "binary" => match json {
            serde_json::Value::String(s) => Ok(YangValue::Bytes(s.as_bytes().to_vec())),
            _ => Err("expected a string (base64-encoded) for YANG type \"binary\"".to_string()),
        },

        "empty" => Ok(YangValue::Empty),

        "enumeration" => parse_enumeration(json, type_stmt),

        "bits" => parse_bits(json, type_stmt),

        "union" => parse_union(json, type_stmt, all_typedefs),

        other => {
            // Try resolving as a reference to another typedef in the same model.
            if let Some(td) = all_typedefs.get(other) {
                parse_type(json, &td.type_stmt, all_typedefs)
            } else {
                Err(format!(
                    "unknown YANG type {other:?} — no built-in match and no typedef \
                     with that name in the model"
                ))
            }
        }
    }
}

/// Parse a YANG `enumeration` value from a JSON string.
fn parse_enumeration(json: &serde_json::Value, type_stmt: &TypeStmt) -> Result<YangValue, String> {
    let s = json
        .as_str()
        .ok_or_else(|| "expected a JSON string for an enumeration value".to_string())?;

    let known: Vec<&str> = type_stmt
        .restrictions
        .iter()
        .filter_map(|r| {
            if let Restriction::Enum(e) = r {
                Some(e.name.as_str())
            } else {
                None
            }
        })
        .collect();

    if known.contains(&s) {
        Ok(YangValue::Enum(s.to_string()))
    } else {
        Err(format!(
            "unknown enumeration variant {s:?}, expected one of: {}",
            known.join(", ")
        ))
    }
}

/// Parse a YANG `bits` value from a JSON array-of-strings or a space-separated
/// JSON string.
fn parse_bits(json: &serde_json::Value, type_stmt: &TypeStmt) -> Result<YangValue, String> {
    let known: Vec<&str> = type_stmt
        .restrictions
        .iter()
        .filter_map(|r| {
            if let Restriction::Bit(b) = r {
                Some(b.name.as_str())
            } else {
                None
            }
        })
        .collect();

    let active: Vec<String> = match json {
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| "bits array must contain strings".to_string())
            })
            .collect::<Result<_, _>>()?,

        serde_json::Value::String(s) => s.split_whitespace().map(str::to_string).collect(),

        _ => {
            return Err(
                "expected a JSON array of strings or a space-separated string for a bits value"
                    .to_string(),
            );
        }
    };

    // Validate each active bit against the known set.
    for bit in &active {
        if !known.contains(&bit.as_str()) {
            return Err(format!(
                "unknown bit {bit:?}, expected one of: {}",
                known.join(", ")
            ));
        }
    }

    Ok(YangValue::Bits(active))
}

/// Try each union member type in declaration order, returning the first
/// successful parse.  The union is transparent — the inner `YangValue` is
/// returned directly without wrapping.
fn parse_union(
    json: &serde_json::Value,
    type_stmt: &TypeStmt,
    all_typedefs: &HashMap<&str, &TypedefNode>,
) -> Result<YangValue, String> {
    let member_types: Vec<&TypeStmt> = type_stmt
        .restrictions
        .iter()
        .filter_map(|r| {
            if let Restriction::Type(t) = r {
                Some(t)
            } else {
                None
            }
        })
        .collect();

    if member_types.is_empty() {
        return Err("union has no member types".to_string());
    }

    let mut last_err = String::new();
    for member in member_types {
        match parse_type(json, member, all_typedefs) {
            Ok(val) => return Ok(val),
            Err(e) => last_err = e,
        }
    }

    Err(format!(
        "JSON value did not match any union member (last error: {last_err})"
    ))
}
