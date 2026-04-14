//! Rust source code generator driven by parsed YANG typedef nodes.
//!
//! Intended for use inside a `build.rs` script:
//!
//! ```rust,no_run
//! // build.rs
//! use dangyang::{parse_file, TypeRegistry, CodeGenerator};
//!
//! fn main() {
//!     let mut registry = TypeRegistry::new();
//!     registry.register("ip-address",  "std::net::IpAddr");
//!     registry.register("port-number", "u16");
//!
//!     let typedefs = parse_file("src/model.yang").unwrap();
//!     let code     = CodeGenerator::new(&registry).generate(&typedefs);
//!
//!     let out = std::env::var("OUT_DIR").unwrap();
//!     std::fs::write(format!("{out}/yang_types.rs"), code).unwrap();
//!
//!     println!("cargo:rerun-if-changed=src/model.yang");
//! }
//! ```
//!
//! And in your main crate:
//!
//! ```rust,ignore
//! include!(concat!(env!("OUT_DIR"), "/yang_types.rs"));
//! ```

use std::collections::HashMap;

use crate::{
    ast::{Restriction, TypedefNode, TypeStmt},
    registry::TypeRegistry,
};

/// Generates Rust source code from a list of parsed YANG typedef nodes.
pub struct CodeGenerator<'r> {
    registry: &'r TypeRegistry,
}

impl<'r> CodeGenerator<'r> {
    pub fn new(registry: &'r TypeRegistry) -> Self {
        Self { registry }
    }

    /// Generate a complete Rust source string for all typedefs.
    ///
    /// The returned string is ready to be written to a file and
    /// `include!`-d from the main crate.
    pub fn generate(&self, typedefs: &[TypedefNode]) -> String {
        // Build a quick lookup so we can resolve cross-references within the
        // same file (e.g. a typedef that derives from another typedef).
        let by_name: HashMap<&str, &TypedefNode> =
            typedefs.iter().map(|t| (t.name.as_str(), t)).collect();

        let mut out = String::from(
            "// @generated — produced by dangyang from YANG typedef statements.\n\
             // Do not edit by hand.\n\n",
        );

        for td in typedefs {
            if let Some(doc) = &td.description {
                for line in doc.lines() {
                    out.push_str("/// ");
                    out.push_str(line.trim());
                    out.push('\n');
                }
            }
            out.push_str(&self.generate_typedef(td, &by_name));
            out.push('\n');
        }

        out
    }

    // ------------------------------------------------------------------
    // Per-typedef dispatch
    // ------------------------------------------------------------------

    fn generate_typedef(
        &self,
        td: &TypedefNode,
        by_name: &HashMap<&str, &TypedefNode>,
    ) -> String {
        let rust_name = to_pascal_case(&td.name);
        let type_name = td.type_stmt.name.as_str();

        // 1. If the typedef's *own name* is registered, the user wants this
        //    specific typedef to be represented by that Rust type regardless of
        //    what the YANG base type says.
        if let Some(rust_type) = self.registry.resolve(&td.name) {
            return newtype_struct(&rust_name, rust_type);
        }

        // 2. If the *base type* has a custom mapping, use that.
        if let Some(rust_type) = self.registry.resolve(type_name) {
            return newtype_struct(&rust_name, rust_type);
        }

        // 2. YANG built-in types.
        match type_name {
            "enumeration" => self.generate_enum(&rust_name, &td.type_stmt),
            "bits" => self.generate_bits_struct(&rust_name, &td.type_stmt),
            "union" => self.generate_union(&rust_name, &td.type_stmt, by_name),
            other => {
                if let Some(rust_prim) = builtin_rust_type(other) {
                    newtype_struct(&rust_name, rust_prim)
                } else if by_name.contains_key(other) {
                    // Derives from another typedef defined in the same file.
                    newtype_struct(&rust_name, &to_pascal_case(other))
                } else {
                    // Unknown — fall back to String with a warning comment.
                    format!(
                        "// WARNING: YANG type {other:?} is not a known built-in \
                         and has no registered mapping; defaulting to String.\n\
                         {}\n",
                        newtype_struct(&rust_name, "String")
                    )
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Enumeration
    // ------------------------------------------------------------------

    fn generate_enum(&self, rust_name: &str, type_stmt: &TypeStmt) -> String {
        let variants: Vec<_> = type_stmt
            .restrictions
            .iter()
            .filter_map(|r| {
                if let Restriction::Enum(e) = r { Some(e) } else { None }
            })
            .collect();

        let mut s = String::new();
        s.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n");
        s.push_str(&format!("pub enum {rust_name} {{\n"));

        for v in &variants {
            if let Some(doc) = &v.description {
                for line in doc.lines() {
                    s.push_str(&format!("    /// {}\n", line.trim()));
                }
            }
            let variant_name = to_pascal_case(&v.name);
            if let Some(val) = v.value {
                s.push_str(&format!("    {variant_name} = {val},\n"));
            } else {
                s.push_str(&format!("    {variant_name},\n"));
            }
        }

        s.push_str("}\n");

        // Generate a FromStr / Display impl for round-tripping.
        s.push('\n');
        s.push_str(&format!(
            "impl ::std::str::FromStr for {rust_name} {{\n\
             {INDENT}type Err = String;\n\
             {INDENT}fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {{\n\
             {INDENT2}match s {{\n"
        ));
        for v in &variants {
            let variant_name = to_pascal_case(&v.name);
            s.push_str(&format!(
                "{INDENT3}{:?} => Ok(Self::{variant_name}),\n",
                v.name
            ));
        }
        s.push_str(&format!(
            "{INDENT3}other => Err(format!(\"unknown {rust_name} value: {{other:?}}\"))\n\
             {INDENT2}}}\n\
             {INDENT}}}\n\
             }}\n"
        ));

        s.push('\n');
        s.push_str(&format!(
            "impl ::std::fmt::Display for {rust_name} {{\n\
             {INDENT}fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {{\n\
             {INDENT2}match self {{\n"
        ));
        for v in &variants {
            let variant_name = to_pascal_case(&v.name);
            s.push_str(&format!(
                "{INDENT3}Self::{variant_name} => f.write_str({:?}),\n",
                v.name
            ));
        }
        s.push_str(&format!("{INDENT2}}}\n{INDENT}}}\n}}\n"));

        s
    }

    // ------------------------------------------------------------------
    // Bits
    // ------------------------------------------------------------------

    fn generate_bits_struct(&self, rust_name: &str, type_stmt: &TypeStmt) -> String {
        let bits: Vec<_> = type_stmt
            .restrictions
            .iter()
            .filter_map(|r| {
                if let Restriction::Bit(b) = r { Some(b) } else { None }
            })
            .collect();

        let mut s = String::new();
        s.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]\n");
        s.push_str(&format!("pub struct {rust_name} {{\n"));

        for b in &bits {
            if let Some(doc) = &b.description {
                for line in doc.lines() {
                    s.push_str(&format!("    /// {}\n", line.trim()));
                }
            }
            let field = to_snake_case(&b.name);
            s.push_str(&format!("    pub {field}: bool,\n"));
        }

        s.push_str("}\n");
        s
    }

    // ------------------------------------------------------------------
    // Union
    // ------------------------------------------------------------------

    fn generate_union(
        &self,
        rust_name: &str,
        type_stmt: &TypeStmt,
        by_name: &HashMap<&str, &TypedefNode>,
    ) -> String {
        let member_types: Vec<_> = type_stmt
            .restrictions
            .iter()
            .filter_map(|r| {
                if let Restriction::Type(t) = r { Some(t) } else { None }
            })
            .collect();

        let mut s = String::new();
        s.push_str("#[derive(Debug, Clone, PartialEq)]\n");
        s.push_str(&format!("pub enum {rust_name} {{\n"));

        for member in &member_types {
            let variant_name = to_pascal_case(&member.name);
            let rust_type = self.resolve_type(&member.name, by_name);
            s.push_str(&format!("    {variant_name}({rust_type}),\n"));
        }

        s.push_str("}\n");
        s
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Resolve a YANG type name to the Rust type string that should be emitted.
    fn resolve_type(&self, yang_name: &str, by_name: &HashMap<&str, &TypedefNode>) -> String {
        if let Some(rt) = self.registry.resolve(yang_name) {
            return rt.to_string();
        }
        if let Some(prim) = builtin_rust_type(yang_name) {
            return prim.to_string();
        }
        if by_name.contains_key(yang_name) {
            return to_pascal_case(yang_name);
        }
        // Last resort
        "String".to_string()
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

const INDENT: &str = "    ";
const INDENT2: &str = "        ";
const INDENT3: &str = "            ";

fn newtype_struct(rust_name: &str, inner: &str) -> String {
    format!(
        "#[derive(Debug, Clone, PartialEq)]\n\
         pub struct {rust_name}(pub {inner});\n"
    )
}

/// Map a YANG built-in type name to its natural Rust equivalent.
fn builtin_rust_type(yang_type: &str) -> Option<&'static str> {
    // Strip a module prefix if present.
    let local = yang_type.rfind(':').map(|i| &yang_type[i + 1..]).unwrap_or(yang_type);
    match local {
        "binary"              => Some("Vec<u8>"),
        "boolean"             => Some("bool"),
        "decimal64"           => Some("f64"),
        "empty"               => Some("()"),
        "instance-identifier" => Some("String"),
        "int8"                => Some("i8"),
        "int16"               => Some("i16"),
        "int32"               => Some("i32"),
        "int64"               => Some("i64"),
        "string"              => Some("String"),
        "uint8"               => Some("u8"),
        "uint16"              => Some("u16"),
        "uint32"              => Some("u32"),
        "uint64"              => Some("u64"),
        "leafref"             => Some("String"),
        "identityref"         => Some("String"),
        _                     => None,
    }
}

/// Convert a YANG kebab-case (or snake_case) identifier to Rust `PascalCase`.
pub fn to_pascal_case(s: &str) -> String {
    s.split(['-', '_'])
        .filter(|p| !p.is_empty())
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

/// Convert a YANG kebab-case identifier to Rust `snake_case`.
pub fn to_snake_case(s: &str) -> String {
    s.replace('-', "_")
}
