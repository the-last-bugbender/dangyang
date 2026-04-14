use crate::{
    TypeRegistry, YangLibrary, YangValue,
    ast::{Restriction, Status},
    codegen::CodeGenerator,
    parse_str,
};

const EXAMPLE: &str = r#"
module example {
    typedef port-number {
        type uint16 {
            range "0..65535";
        }
        description "A TCP/UDP port number.";
        units "port";
    }

    typedef connection-type {
        type enumeration {
            enum tcp {
                value 6;
                description "Transmission Control Protocol";
            }
            enum udp {
                value 17;
            }
            enum sctp;
        }
        description "Transport layer protocol.";
    }

    typedef flag-bits {
        type bits {
            bit active {
                position 0;
                description "Set when active.";
            }
            bit locked {
                position 1;
            }
        }
    }

    typedef host-string {
        type string {
            pattern '[a-zA-Z0-9\-\.]+';
            length "1..253";
        }
        description "A hostname or IP address as a string.";
    }

    typedef multi-type {
        type union {
            type string;
            type uint32;
        }
    }

    typedef derived-port {
        type port-number;
        description "A port derived from port-number.";
    }
}
"#;

#[test]
fn parse_typedefs_from_module() {
    let tds = parse_str(EXAMPLE).unwrap();
    assert_eq!(tds.len(), 6);
    assert_eq!(tds[0].name, "port-number");
    assert_eq!(tds[1].name, "connection-type");
    assert_eq!(tds[2].name, "flag-bits");
    assert_eq!(tds[3].name, "host-string");
    assert_eq!(tds[4].name, "multi-type");
    assert_eq!(tds[5].name, "derived-port");
}

#[test]
fn port_number_range() {
    let tds = parse_str(EXAMPLE).unwrap();
    let port = &tds[0];
    assert_eq!(port.type_stmt.name, "uint16");
    assert_eq!(port.description.as_deref(), Some("A TCP/UDP port number."));
    assert_eq!(port.units.as_deref(), Some("port"));
    assert!(matches!(
        port.type_stmt.restrictions[0],
        Restriction::Range(ref r) if r == "0..65535"
    ));
}

#[test]
fn enumeration_variants() {
    let tds = parse_str(EXAMPLE).unwrap();
    let conn = &tds[1];
    assert_eq!(conn.type_stmt.name, "enumeration");

    let variants: Vec<_> = conn
        .type_stmt
        .restrictions
        .iter()
        .filter_map(|r| {
            if let Restriction::Enum(e) = r {
                Some(e)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(variants.len(), 3);
    assert_eq!(variants[0].name, "tcp");
    assert_eq!(variants[0].value, Some(6));
    assert_eq!(variants[1].name, "udp");
    assert_eq!(variants[1].value, Some(17));
    assert_eq!(variants[2].name, "sctp");
    assert_eq!(variants[2].value, None);
}

#[test]
fn bits_definition() {
    let tds = parse_str(EXAMPLE).unwrap();
    let flags = &tds[2];
    assert_eq!(flags.type_stmt.name, "bits");

    let bits: Vec<_> = flags
        .type_stmt
        .restrictions
        .iter()
        .filter_map(|r| {
            if let Restriction::Bit(b) = r {
                Some(b)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(bits.len(), 2);
    assert_eq!(bits[0].name, "active");
    assert_eq!(bits[0].position, Some(0));
    assert_eq!(bits[1].name, "locked");
    assert_eq!(bits[1].position, Some(1));
}

#[test]
fn string_restrictions() {
    let tds = parse_str(EXAMPLE).unwrap();
    let hs = &tds[3];
    assert_eq!(hs.type_stmt.name, "string");
    assert!(
        hs.type_stmt
            .restrictions
            .iter()
            .any(|r| matches!(r, Restriction::Pattern(_)))
    );
    assert!(
        hs.type_stmt
            .restrictions
            .iter()
            .any(|r| matches!(r, Restriction::Length(_)))
    );
}

#[test]
fn union_members() {
    let tds = parse_str(EXAMPLE).unwrap();
    let mt = &tds[4];
    assert_eq!(mt.type_stmt.name, "union");
    let types: Vec<_> = mt
        .type_stmt
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
    assert_eq!(types.len(), 2);
    assert_eq!(types[0].name, "string");
    assert_eq!(types[1].name, "uint32");
}

#[test]
fn codegen_newtypes() {
    let tds = parse_str(EXAMPLE).unwrap();
    let registry = TypeRegistry::new();
    let code = CodeGenerator::new(&registry).generate(&tds);

    // port-number → u16 newtype
    assert!(
        code.contains("pub struct PortNumber(pub u16);"),
        "code:\n{code}"
    );
    // host-string → String newtype
    assert!(
        code.contains("pub struct HostString(pub String);"),
        "code:\n{code}"
    );
    // derived-port → PortNumber newtype
    assert!(
        code.contains("pub struct DerivedPort(pub PortNumber);"),
        "code:\n{code}"
    );
}

#[test]
fn codegen_enum() {
    let tds = parse_str(EXAMPLE).unwrap();
    let registry = TypeRegistry::new();
    let code = CodeGenerator::new(&registry).generate(&tds);

    assert!(code.contains("pub enum ConnectionType"), "code:\n{code}");
    assert!(code.contains("Tcp = 6,"), "code:\n{code}");
    assert!(code.contains("Udp = 17,"), "code:\n{code}");
    assert!(code.contains("Sctp,"), "code:\n{code}");
}

#[test]
fn codegen_bits_struct() {
    let tds = parse_str(EXAMPLE).unwrap();
    let registry = TypeRegistry::new();
    let code = CodeGenerator::new(&registry).generate(&tds);

    assert!(code.contains("pub struct FlagBits"), "code:\n{code}");
    assert!(code.contains("pub active: bool"), "code:\n{code}");
    assert!(code.contains("pub locked: bool"), "code:\n{code}");
}

#[test]
fn codegen_custom_type_mapping() {
    let tds = parse_str(
        r#"
        typedef ip-address {
            type string {
                pattern '.*';
            }
        }
        typedef host-entry {
            type ip-address;
        }
    "#,
    )
    .unwrap();

    let mut registry = TypeRegistry::new();
    registry.register("ip-address", "std::net::IpAddr");

    let code = CodeGenerator::new(&registry).generate(&tds);

    // ip-address itself gets the registered type
    assert!(
        code.contains("pub struct IpAddress(pub std::net::IpAddr);"),
        "code:\n{code}"
    );
    // host-entry derives from ip-address, so it also gets the custom type
    assert!(
        code.contains("pub struct HostEntry(pub std::net::IpAddr);"),
        "code:\n{code}"
    );
}

#[test]
fn codegen_union() {
    let tds = parse_str(EXAMPLE).unwrap();
    let registry = TypeRegistry::new();
    let code = CodeGenerator::new(&registry).generate(&tds);

    assert!(code.contains("pub enum MultiType"), "code:\n{code}");
    assert!(code.contains("String(String)"), "code:\n{code}");
    assert!(code.contains("Uint32(u32)"), "code:\n{code}");
}

#[test]
fn flat_file_no_module_wrapper() {
    let yang = r#"
        typedef my-id {
            type uint32;
        }
    "#;
    let tds = parse_str(yang).unwrap();
    assert_eq!(tds.len(), 1);
    assert_eq!(tds[0].name, "my-id");
}

// ---------------------------------------------------------------------------
// YangLibrary tests
// ---------------------------------------------------------------------------

const LIBRARY_YANG: &str = r#"
module netdev {
    typedef admin-state {
        type enumeration {
            enum up   { value 1; }
            enum down { value 2; }
            enum testing;
        }
    }

    typedef port-number {
        type uint16;
    }

    typedef retry-count {
        type int32;
    }

    typedef enabled {
        type boolean;
    }

    typedef description {
        type string;
    }

    typedef load-avg {
        type decimal64 { fraction-digits 2; }
    }

    typedef interface-flags {
        type bits {
            bit up       { position 0; }
            bit loopback { position 1; }
            bit multicast { position 2; }
        }
    }

    typedef address-or-port {
        type union {
            type string;
            type uint32;
        }
    }

    typedef derived-port {
        type port-number;
    }
}
"#;

#[test]
fn library_register_and_model_names() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();
    lib.register_model("other", "typedef x { type string; }")
        .unwrap();

    let mut names: Vec<&str> = lib.model_names().collect();
    names.sort_unstable();
    assert_eq!(names, ["netdev", "other"]);
}

#[test]
fn library_typedef_names() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    let mut names: Vec<&str> = lib.typedef_names("netdev").unwrap().collect();
    names.sort_unstable();
    assert!(names.contains(&"admin-state"));
    assert!(names.contains(&"port-number"));
    assert!(names.contains(&"interface-flags"));
}

#[test]
fn library_parse_scalar_types() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    let json = serde_json::json!({
        "admin-state":  "up",
        "port-number":  8080u64,
        "retry-count":  -3i64,
        "enabled":      true,
        "description":  "primary interface",
        "load-avg":     1.75,
    });

    let obj = lib.parse("netdev", &json).unwrap();

    assert_eq!(obj["admin-state"], YangValue::Enum("up".to_string()));
    assert_eq!(obj["port-number"], YangValue::UInt(8080));
    assert_eq!(obj["retry-count"], YangValue::Int(-3));
    assert_eq!(obj["enabled"], YangValue::Bool(true));
    assert_eq!(
        obj["description"],
        YangValue::Text("primary interface".to_string())
    );
    assert_eq!(obj["load-avg"], YangValue::Float(1.75));
}

#[test]
fn library_parse_bits_array() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    let json = serde_json::json!({ "interface-flags": ["up", "multicast"] });
    let obj = lib.parse("netdev", &json).unwrap();

    assert_eq!(
        obj["interface-flags"],
        YangValue::Bits(vec!["up".to_string(), "multicast".to_string()])
    );
}

#[test]
fn library_parse_bits_space_separated_string() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    let json = serde_json::json!({ "interface-flags": "up loopback" });
    let obj = lib.parse("netdev", &json).unwrap();

    assert_eq!(
        obj["interface-flags"],
        YangValue::Bits(vec!["up".to_string(), "loopback".to_string()])
    );
}

#[test]
fn library_parse_union_string_branch() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    let json = serde_json::json!({ "address-or-port": "192.168.1.1" });
    let obj = lib.parse("netdev", &json).unwrap();
    assert_eq!(
        obj["address-or-port"],
        YangValue::Text("192.168.1.1".to_string())
    );
}

#[test]
fn library_parse_union_uint_branch() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    let json = serde_json::json!({ "address-or-port": 443u32 });
    let obj = lib.parse("netdev", &json).unwrap();
    assert_eq!(obj["address-or-port"], YangValue::UInt(443));
}

#[test]
fn library_parse_derived_typedef() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    // derived-port derives from port-number (uint16)
    let json = serde_json::json!({ "derived-port": 22u64 });
    let obj = lib.parse("netdev", &json).unwrap();
    assert_eq!(obj["derived-port"], YangValue::UInt(22));
}

#[test]
fn library_parse_as_single_field() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    let val = lib
        .parse_as("netdev", "admin-state", &serde_json::json!("down"))
        .unwrap();
    assert_eq!(val, YangValue::Enum("down".to_string()));
}

#[test]
fn library_partial_object_ok() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    // Providing only a subset of typedefs is fine.
    let json = serde_json::json!({ "port-number": 443u64 });
    let obj = lib.parse("netdev", &json).unwrap();
    assert_eq!(obj.len(), 1);
    assert_eq!(obj["port-number"], YangValue::UInt(443));
}

#[test]
fn library_error_unknown_model() {
    let lib = YangLibrary::new();
    let err = lib.parse("nope", &serde_json::json!({})).unwrap_err();
    assert!(matches!(err, crate::LibraryError::ModelNotFound(_)));
}

#[test]
fn library_error_unknown_field() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    let json = serde_json::json!({ "not-a-typedef": 1 });
    let err = lib.parse("netdev", &json).unwrap_err();
    assert!(matches!(err, crate::LibraryError::TypedefNotFound { .. }));
}

#[test]
fn library_error_invalid_enum_value() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    let json = serde_json::json!({ "admin-state": "rebooting" });
    let err = lib.parse("netdev", &json).unwrap_err();
    assert!(matches!(err, crate::LibraryError::InvalidValue { .. }));
}

#[test]
fn library_error_not_an_object() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    let err = lib
        .parse("netdev", &serde_json::json!([1, 2, 3]))
        .unwrap_err();
    assert!(matches!(err, crate::LibraryError::NotAnObject));
}

#[test]
fn library_error_wrong_type() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    // port-number expects a uint, not a string
    let json = serde_json::json!({ "port-number": "eight-thousand" });
    let err = lib.parse("netdev", &json).unwrap_err();
    assert!(matches!(err, crate::LibraryError::InvalidValue { .. }));
}

#[test]
fn library_yang_value_display() {
    assert_eq!(YangValue::Text("hello".into()).to_string(), "hello");
    assert_eq!(YangValue::Enum("up".into()).to_string(), "up");
    assert_eq!(YangValue::UInt(42).to_string(), "42");
    assert_eq!(YangValue::Int(-7).to_string(), "-7");
    assert_eq!(YangValue::Bool(true).to_string(), "true");
    assert_eq!(YangValue::Empty.to_string(), "");
    assert_eq!(
        YangValue::Bits(vec!["up".into(), "loopback".into()]).to_string(),
        "up loopback"
    );
    assert_eq!(YangValue::Bytes(vec![1, 2, 3]).to_string(), "<3 bytes>");
}

#[test]
fn library_yang_value_accessors() {
    assert_eq!(YangValue::Text("x".into()).as_str(), Some("x"));
    assert_eq!(YangValue::Enum("y".into()).as_str(), Some("y"));
    assert_eq!(YangValue::UInt(5).as_uint(), Some(5));
    assert_eq!(YangValue::Int(-1).as_int(), Some(-1));
    assert_eq!(YangValue::Float(3.14).as_float(), Some(3.14));
    assert_eq!(YangValue::Bool(false).as_bool(), Some(false));
    assert!(YangValue::Empty.is_empty());
    assert_eq!(
        YangValue::Bits(vec!["up".into()]).as_bits(),
        Some(["up".to_string()].as_slice())
    );
    // Cross-type mismatches return None
    assert_eq!(YangValue::UInt(1).as_str(), None);
    assert_eq!(YangValue::Text("x".into()).as_uint(), None);
}

#[test]
fn library_object_iter_and_into_iter() {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", LIBRARY_YANG).unwrap();

    let json = serde_json::json!({
        "port-number": 80u64,
        "enabled": true,
    });
    let obj = lib.parse("netdev", &json).unwrap();

    assert_eq!(obj.len(), 2);

    let mut keys: Vec<&str> = obj.iter().map(|(k, _)| k).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["enabled", "port-number"]);

    // into_iter consumes the object
    let map = obj.into_fields();
    assert_eq!(map.len(), 2);
}

#[test]
fn module_prefix_registry_resolution() {
    let mut registry = TypeRegistry::new();
    registry.register("ietf-inet-types:ip-address", "std::net::IpAddr");

    // Should resolve both with and without prefix
    assert_eq!(
        registry.resolve("ietf-inet-types:ip-address"),
        Some("std::net::IpAddr")
    );
    assert_eq!(registry.resolve("ip-address"), Some("std::net::IpAddr"));
}

#[test]
fn parse_status_field() {
    let yang = r#"
        typedef my-enum {
            type enumeration {
                enum active {
                    status current;
                }
                enum old {
                    status deprecated;
                }
            }
        }
    "#;
    let tds = parse_str(yang).unwrap();
    let variants: Vec<_> = tds[0]
        .type_stmt
        .restrictions
        .iter()
        .filter_map(|r| {
            if let Restriction::Enum(e) = r {
                Some(e)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(variants[0].status, Some(Status::Current));
    assert_eq!(variants[1].status, Some(Status::Deprecated));
}
