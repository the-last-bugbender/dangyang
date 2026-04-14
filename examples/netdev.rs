//! Demonstrates runtime JSON parsing against a network device YANG model.
//!
//! Run with:
//!
//!     cargo run --example netdev

use dang_yang::{YangLibrary, YangValue};

const YANG: &str = r#"
    module netdev {
        typedef hostname {
            type string;
            description "A fully qualified domain name or hostname.";
        }
        typedef interface-name {
            type string;
            description "The name of a network interface, e.g. eth0 or GigabitEthernet0/0.";
        }
        typedef interface-state {
            type enumeration {
                enum up           { value 1; }
                enum down         { value 2; }
                enum testing      { value 3; }
                enum unknown      { value 4; }
                enum dormant      { value 5; }
                enum not-present  { value 6; }
                enum lower-layer-down { value 7; }
            }
            description "The operational state of a network interface (RFC 2863 ifOperStatus).";
        }
        typedef link-duplex {
            type enumeration {
                enum full { value 1; }
                enum half { value 2; }
                enum auto { value 3; }
            }
            description "Duplex mode of a physical link.";
        }
        typedef interface-flags {
            type bits {
                bit up            { position 0; }
                bit broadcast     { position 1; }
                bit loopback      { position 2; }
                bit point-to-point { position 3; }
                bit multicast     { position 4; }
                bit promisc       { position 5; }
            }
            description "Linux-style interface flags.";
        }
        typedef port-number {
            type uint16;
            description "A TCP/UDP port number.";
        }
        typedef vlan-id {
            type uint16;
            description "An IEEE 802.1Q VLAN identifier.";
        }
        typedef bandwidth-bps {
            type uint64;
            description "Bandwidth in bits per second.";
        }
        typedef mac-address {
            type string;
            description "A 48-bit MAC address in colon-separated hex notation.";
        }
    }
"#;

const JSON: &str = r#"{
    "hostname":        "core-router-01.example.net",
    "interface-name":  "GigabitEthernet0/0",
    "interface-state": "up",
    "link-duplex":     "full",
    "interface-flags": ["up", "broadcast", "multicast"],
    "port-number":     8080,
    "vlan-id":         100,
    "bandwidth-bps":   1000000000,
    "mac-address":     "aa:bb:cc:dd:ee:ff"
}"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut lib = YangLibrary::new();
    lib.register_model("netdev", YANG)?;

    let json: serde_json::Value = serde_json::from_str(JSON)?;
    let obj = lib.parse("netdev", &json)?;

    if let Some(YangValue::Text(h)) = obj.get("hostname") {
        println!("hostname:        {h}");
    }
    if let Some(YangValue::Text(iface)) = obj.get("interface-name") {
        println!("interface-name:  {iface}");
    }
    if let Some(YangValue::Enum(state)) = obj.get("interface-state") {
        println!("interface-state: {state}");
    }
    if let Some(YangValue::Enum(duplex)) = obj.get("link-duplex") {
        println!("link-duplex:     {duplex}");
    }
    if let Some(val) = obj.get("interface-flags") {
        let bits = val.as_bits().unwrap_or_default();
        println!("interface-flags: {}", bits.join(", "));
    }
    if let Some(val) = obj.get("port-number") {
        println!("port-number:     {}", val.as_uint().unwrap());
    }
    if let Some(val) = obj.get("vlan-id") {
        println!("vlan-id:         {}", val.as_uint().unwrap());
    }
    if let Some(val) = obj.get("bandwidth-bps") {
        println!("bandwidth-bps:   {}", val.as_uint().unwrap());
    }
    if let Some(YangValue::Text(mac)) = obj.get("mac-address") {
        println!("mac-address:     {mac}");
    }

    Ok(())
}
