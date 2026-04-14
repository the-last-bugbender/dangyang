#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::String};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

/// Maps YANG derived type names to the Rust types that should represent them
/// in generated code.
///
/// Register a mapping before running the code generator so that any typedef
/// whose base type matches a registered name gets the correct Rust type
/// instead of the default String/numeric fallback.
///
/// # Example
/// ```rust
/// use dang_yang::TypeRegistry;
///
/// let mut registry = TypeRegistry::new();
/// // YANG type "ip-address"  →  Rust type `std::net::IpAddr`
/// registry.register("ip-address", "std::net::IpAddr");
/// // Module-prefixed names are also supported
/// registry.register("ietf-inet-types:ipv6-address", "std::net::Ipv6Addr");
/// ```
#[derive(Default)]
pub struct TypeRegistry {
    /// yang_name → rust_type_path
    mappings: BTreeMap<String, String>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a mapping from a YANG type name to a Rust type expression.
    ///
    /// `yang_name` may be a plain name (`"ip-address"`) or module-prefixed
    /// (`"ietf-inet-types:ip-address"`). Both forms are tried during resolution.
    ///
    /// `rust_type` is the Rust type to emit verbatim in generated code,
    /// e.g. `"std::net::IpAddr"` or `"crate::types::Port"`.
    pub fn register(&mut self, yang_name: impl Into<String>, rust_type: impl Into<String>) {
        self.mappings.insert(yang_name.into(), rust_type.into());
    }

    /// Resolve a YANG type name to its registered Rust type string.
    ///
    /// Resolution order:
    /// 1. Exact match (`"ietf-inet-types:ip-address"`)
    /// 2. Local part of a prefixed lookup (`"ietf-inet-types:ip-address"` → try `"ip-address"`)
    /// 3. Any registered key whose local part matches an unprefixed lookup
    ///    (`"ip-address"` → finds key `"ietf-inet-types:ip-address"`)
    ///
    /// Returns `None` if no mapping is registered (built-ins are handled
    /// separately by the code generator).
    pub fn resolve(&self, yang_name: &str) -> Option<&str> {
        // 1. Exact match.
        if let Some(v) = self.mappings.get(yang_name) {
            return Some(v.as_str());
        }
        // 2. Prefixed input → try the local part.
        if let Some(i) = yang_name.rfind(':')
            && let Some(v) = self.mappings.get(&yang_name[i + 1..])
        {
            return Some(v.as_str());
        }
        // 3. Unprefixed input → search for any key whose local part matches.
        self.mappings
            .iter()
            .find(|(k, _)| {
                k.rfind(':')
                    .map(|i| &k[i + 1..] == yang_name)
                    .unwrap_or(false)
            })
            .map(|(_, v)| v.as_str())
    }

    /// Returns `true` if a mapping is registered for `yang_name`.
    pub fn contains(&self, yang_name: &str) -> bool {
        self.resolve(yang_name).is_some()
    }
}
