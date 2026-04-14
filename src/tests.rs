use crate::{
    ast::{Restriction, Status},
    codegen::CodeGenerator,
    parse_str, TypeRegistry,
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

    let variants: Vec<_> = conn.type_stmt.restrictions.iter().filter_map(|r| {
        if let Restriction::Enum(e) = r { Some(e) } else { None }
    }).collect();

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

    let bits: Vec<_> = flags.type_stmt.restrictions.iter().filter_map(|r| {
        if let Restriction::Bit(b) = r { Some(b) } else { None }
    }).collect();

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
    assert!(hs.type_stmt.restrictions.iter().any(|r| matches!(r, Restriction::Pattern(_))));
    assert!(hs.type_stmt.restrictions.iter().any(|r| matches!(r, Restriction::Length(_))));
}

#[test]
fn union_members() {
    let tds = parse_str(EXAMPLE).unwrap();
    let mt = &tds[4];
    assert_eq!(mt.type_stmt.name, "union");
    let types: Vec<_> = mt.type_stmt.restrictions.iter().filter_map(|r| {
        if let Restriction::Type(t) = r { Some(t) } else { None }
    }).collect();
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
    assert!(code.contains("pub struct PortNumber(pub u16);"), "code:\n{code}");
    // host-string → String newtype
    assert!(code.contains("pub struct HostString(pub String);"), "code:\n{code}");
    // derived-port → PortNumber newtype
    assert!(code.contains("pub struct DerivedPort(pub PortNumber);"), "code:\n{code}");
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
    let tds = parse_str(r#"
        typedef ip-address {
            type string {
                pattern '.*';
            }
        }
        typedef host-entry {
            type ip-address;
        }
    "#).unwrap();

    let mut registry = TypeRegistry::new();
    registry.register("ip-address", "std::net::IpAddr");

    let code = CodeGenerator::new(&registry).generate(&tds);

    // ip-address itself gets the registered type
    assert!(code.contains("pub struct IpAddress(pub std::net::IpAddr);"), "code:\n{code}");
    // host-entry derives from ip-address, so it also gets the custom type
    assert!(code.contains("pub struct HostEntry(pub std::net::IpAddr);"), "code:\n{code}");
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

#[test]
fn module_prefix_registry_resolution() {
    let mut registry = TypeRegistry::new();
    registry.register("ietf-inet-types:ip-address", "std::net::IpAddr");

    // Should resolve both with and without prefix
    assert_eq!(registry.resolve("ietf-inet-types:ip-address"), Some("std::net::IpAddr"));
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
    let variants: Vec<_> = tds[0].type_stmt.restrictions.iter().filter_map(|r| {
        if let Restriction::Enum(e) = r { Some(e) } else { None }
    }).collect();

    assert_eq!(variants[0].status, Some(Status::Current));
    assert_eq!(variants[1].status, Some(Status::Deprecated));
}
