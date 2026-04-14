# dang-yang

A Rust library for parsing YANG `typedef` statements and generating Rust types from them. Designed for use in `build.rs` scripts so that YANG data models become native Rust types at compile time.

## Overview

YANG ([RFC 7950](https://datatracker.ietf.org/doc/html/rfc7950)) is a data modeling language used heavily in network configuration (NETCONF, RESTCONF). Its `typedef` statement defines named, reusable types — this library parses those definitions and turns them into Rust `struct`s and `enum`s, with full support for mapping any derived YANG type to a Rust type of your choosing.

## Getting started

Add dang-yang to your `build-dependencies`:

```toml
[build-dependencies]
dang-yang = "0.0.3"
```

Create a `build.rs`:

```rust
use dang_yang::{parse_file, CodeGenerator, TypeRegistry};

fn main() {
    // Tell the generator which YANG types map to which Rust types.
    let mut registry = TypeRegistry::new();
    registry.register("ip-address",  "std::net::IpAddr");
    registry.register("port-number", "u16");

    // Parse all typedef statements from a YANG source file.
    let typedefs = parse_file("src/model.yang").unwrap();

    // Generate Rust source.
    let code = CodeGenerator::new(&registry).generate(&typedefs);

    let out = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{out}/yang_types.rs"), code).unwrap();

    println!("cargo:rerun-if-changed=src/model.yang");
}
```

Include the generated file in your crate:

```rust
// src/lib.rs or src/main.rs
include!(concat!(env!("OUT_DIR"), "/yang_types.rs"));
```

## What gets generated

Given this YANG:

```yang
module example {
    typedef port-number {
        type uint16 {
            range "0..65535";
        }
        description "A TCP/UDP port number.";
    }

    typedef connection-type {
        type enumeration {
            enum tcp { value 6; }
            enum udp { value 17; }
            enum sctp;
        }
    }

    typedef feature-flags {
        type bits {
            bit active   { position 0; }
            bit read-only { position 1; }
        }
    }

    typedef address-or-port {
        type union {
            type string;
            type uint16;
        }
    }
}
```

dang_yang produces:

```rust
/// A TCP/UDP port number.
#[derive(Debug, Clone, PartialEq)]
pub struct PortNumber(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionType {
    Tcp = 6,
    Udp = 17,
    Sctp,
}

impl std::str::FromStr for ConnectionType { /* ... */ }
impl std::fmt::Display for ConnectionType { /* ... */ }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct FeatureFlags {
    pub active: bool,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AddressOrPort {
    String(String),
    Uint16(u16),
}
```

## Custom type mappings

Register any YANG derived type to have the generator use a specific Rust type instead of the default fallback.

```rust
let mut registry = TypeRegistry::new();

// A YANG string-based type → std::net::IpAddr
registry.register("ip-address", "std::net::IpAddr");

// A type from your own crate
registry.register("transaction-id", "crate::TransactionId");

// Module-prefixed names are also supported — both forms resolve correctly
registry.register("ietf-inet-types:ipv6-address", "std::net::Ipv6Addr");
```

When a typedef's own name is registered, its inner YANG type is ignored and the registered Rust type is used directly. When a typedef *derives from* a registered type, the generated newtype wraps the registered Rust type.

```yang
typedef ip-address { type string { pattern "..."; } }  // registered → IpAddr
typedef host-address { type ip-address; }              // derives → wraps IpAddr
```

```rust
pub struct IpAddress(pub std::net::IpAddr);
pub struct HostAddress(pub std::net::IpAddr);
```

## Built-in type mapping

| YANG built-in | Rust type |
|---|---|
| `string` | `String` |
| `boolean` | `bool` |
| `int8` / `int16` / `int32` / `int64` | `i8` / `i16` / `i32` / `i64` |
| `uint8` / `uint16` / `uint32` / `uint64` | `u8` / `u16` / `u32` / `u64` |
| `decimal64` | `f64` |
| `binary` | `Vec<u8>` |
| `empty` | `()` |
| `enumeration` | Generated `enum` |
| `bits` | Generated `struct` with `bool` fields |
| `union` | Generated `enum` with one variant per member type |
| `leafref` / `identityref` / `instance-identifier` | `String` |

## Runtime parsing with YangLibrary

For cases where the YANG model is not known at compile time — loading models from disk at startup, validating dynamic JSON payloads, building tooling that works with arbitrary schemas — `YangLibrary` lets you register YANG models and parse JSON objects against them at runtime.

Add `dang-yang` to your regular `[dependencies]` (not `[build-dependencies]`):

```toml
[dependencies]
dang-yang = "0.1"
serde_json = "1"
```

### Register models and parse objects

```rust
use dang_yang::{YangLibrary, YangValue};

const NETDEV: &str = r#"
    module netdev {
        typedef admin-state {
            type enumeration {
                enum up;
                enum down;
                enum testing;
            }
        }
        typedef port-number {
            type uint16;
        }
        typedef interface-flags {
            type bits {
                bit up       { position 0; }
                bit loopback { position 1; }
            }
        }
    }
"#;

let mut lib = YangLibrary::new();
lib.register_model("netdev", NETDEV)?;

// Or load from a file:
// lib.register_model_file("netdev", "models/netdev.yang")?;

let json = serde_json::json!({
    "admin-state":     "up",
    "port-number":     8080,
    "interface-flags": ["up", "loopback"],
});

let obj = lib.parse("netdev", &json)?;

// Access fields with the Index operator (panics if absent)
assert_eq!(obj["admin-state"].as_str(),  Some("up"));
assert_eq!(obj["port-number"].as_uint(), Some(8080));

// Or with get() for graceful None on missing fields
if let Some(flags) = obj.get("interface-flags") {
    println!("flags: {flags}");  // "up loopback"
}
```

### Parse a single field

```rust
let val = lib.parse_as("netdev", "port-number", &serde_json::json!(443))?;
assert_eq!(val, YangValue::UInt(443));
```

### Match on YangValue variants

```rust
match obj["admin-state"] {
    YangValue::Enum(ref s) => println!("state = {s}"),
    _ => unreachable!(),
}

// Bits values carry the names of all active bits
if let YangValue::Bits(ref active) = obj["interface-flags"] {
    for bit in active {
        println!("bit active: {bit}");
    }
}
```

### Multiple models

```rust
lib.register_model("ifconfig", IFCONFIG_YANG)?;
lib.register_model("routing",  ROUTING_YANG)?;

let iface = lib.parse("ifconfig", &iface_json)?;
let route = lib.parse("routing",  &route_json)?;
```

Fields present in the YANG model but absent from the JSON are simply omitted from the returned `YangObject` — not an error. Extra JSON keys that have no corresponding typedef produce a `LibraryError::TypedefNotFound`.

## Parsing only

If you only need the parsed AST without code generation:

```rust
use dang_yang::{parse_str, parse_file, TypedefNode, Restriction};

let typedefs: Vec<TypedefNode> = dang_yang::parse_file("model.yang")?;

for td in &typedefs {
    println!("{} : {}", td.name, td.type_stmt.name);

    for r in &td.type_stmt.restrictions {
        match r {
            Restriction::Pattern(p) => println!("  pattern: {p}"),
            Restriction::Range(r)   => println!("  range:   {r}"),
            Restriction::Enum(e)    => println!("  enum:    {}", e.name),
            _ => {}
        }
    }
}
```

`module` and `submodule` wrappers are handled transparently; all non-`typedef` statements are skipped.

## Contributing

After cloning, activate the pre-commit hook (runs `cargo fmt --check` and `cargo clippy`):

```bash
make setup
# or: git config core.hooksPath .githooks
```

## License

MIT
