/// A parsed YANG `typedef` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedefNode {
    /// The name given to this typedef (e.g. `"ip-address"`).
    pub name: String,
    /// The `type` sub-statement.
    pub type_stmt: TypeStmt,
    pub description: Option<String>,
    pub units: Option<String>,
    pub default: Option<String>,
}

/// The `type` statement inside a typedef (or union member).
#[derive(Debug, Clone, PartialEq)]
pub struct TypeStmt {
    /// Raw type name as it appears in the YANG source.
    /// For built-ins this is e.g. `"string"`, `"uint32"`;
    /// for derived types it is the typedef name, possibly module-prefixed
    /// (e.g. `"ietf-inet-types:ip-address"`).
    pub name: String,
    /// Zero or more restrictions / sub-statements.
    pub restrictions: Vec<Restriction>,
}

/// A restriction or sub-statement that can appear inside a `type` block.
#[derive(Debug, Clone, PartialEq)]
pub enum Restriction {
    /// `pattern "regex";`
    Pattern(String),
    /// `length "expr";` — e.g. `"1..253"`
    Length(String),
    /// `range "expr";` — e.g. `"0..65535"`
    Range(String),
    /// `fraction-digits N;`
    FractionDigits(u8),
    /// One `enum NAME { ... }` member of an enumeration.
    Enum(EnumVariant),
    /// One `bit NAME { ... }` member of a bits type.
    Bit(BitDef),
    /// `path "xpath-expr";` (leafref)
    Path(String),
    /// `require-instance true|false;`
    RequireInstance(bool),
    /// `base NAME;` (identityref)
    Base(String),
    /// A nested `type` statement inside a union.
    Type(TypeStmt),
}

/// A single variant in a YANG `enumeration`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<i64>,
    pub description: Option<String>,
    pub status: Option<Status>,
}

/// A single bit in a YANG `bits` type.
#[derive(Debug, Clone, PartialEq)]
pub struct BitDef {
    pub name: String,
    pub position: Option<u32>,
    pub description: Option<String>,
    pub status: Option<Status>,
}

/// YANG status values.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Current,
    Deprecated,
    Obsolete,
}
