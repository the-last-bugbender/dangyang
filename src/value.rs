/// A runtime YANG value produced by [`YangLibrary::parse`].
///
/// Each variant corresponds to a YANG built-in type family.  For derived
/// types and unions the library resolves to the underlying built-in before
/// returning a value, so callers always get a flat, concrete variant.
#[derive(Debug, Clone, PartialEq)]
pub enum YangValue {
    /// `string`, `leafref`, `identityref`, `instance-identifier`
    Text(String),
    /// `int8`, `int16`, `int32`, `int64`
    Int(i64),
    /// `uint8`, `uint16`, `uint32`, `uint64`
    UInt(u64),
    /// `decimal64`
    Float(f64),
    /// `boolean`
    Bool(bool),
    /// `binary` — the raw bytes of the base64-encoded JSON string.
    /// Decode with your preferred base64 crate if needed.
    Bytes(Vec<u8>),
    /// `enumeration` — the matched variant name as it appears in the YANG source.
    Enum(String),
    /// `bits` — the names of all active bits, in the order supplied by the JSON.
    Bits(Vec<String>),
    /// `empty`
    Empty,
}

impl YangValue {
    /// Return the inner string if this is a [`Text`](Self::Text) or
    /// [`Enum`](Self::Enum) value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(s) | Self::Enum(s) => Some(s),
            _ => None,
        }
    }

    /// Return the inner value if this is a [`UInt`](Self::UInt).
    pub fn as_uint(&self) -> Option<u64> {
        match self {
            Self::UInt(n) => Some(*n),
            _ => None,
        }
    }

    /// Return the inner value if this is an [`Int`](Self::Int).
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Return the inner value if this is a [`Float`](Self::Float).
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Return the inner value if this is a [`Bool`](Self::Bool).
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Return the inner slice if this is a [`Bytes`](Self::Bytes) value.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Return the active-bit names if this is a [`Bits`](Self::Bits) value.
    pub fn as_bits(&self) -> Option<&[String]> {
        match self {
            Self::Bits(bits) => Some(bits),
            _ => None,
        }
    }

    /// Returns `true` if this is the [`Empty`](Self::Empty) variant.
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

impl std::fmt::Display for YangValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(s) | Self::Enum(s) => f.write_str(s),
            Self::Int(n) => write!(f, "{n}"),
            Self::UInt(n) => write!(f, "{n}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Bytes(b) => write!(f, "<{} bytes>", b.len()),
            Self::Bits(bits) => f.write_str(&bits.join(" ")),
            Self::Empty => Ok(()),
        }
    }
}
