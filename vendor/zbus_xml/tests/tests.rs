use std::error::Error;

use zbus_xml::{Arg, ArgDirection, Interface, Node, PropertyAccess};
use zvariant::Signature;

#[test]
fn serde() -> Result<(), Box<dyn Error>> {
    let example = include_str!("data/sample_object0.xml");
    let node_r = Node::from_reader(example.as_bytes())?;
    let node = Node::try_from(example)?;
    assert_eq!(node, node_r);
    assert_eq!(node.interfaces().len(), 1);
    assert_eq!(node.interfaces()[0].methods().len(), 3);
    assert_eq!(
        node.interfaces()[0].methods()[0].args()[0]
            .direction()
            .unwrap(),
        ArgDirection::In
    );
    assert_eq!(node.nodes().len(), 4);

    let node_str: Node<'_> = example.try_into()?;
    assert_eq!(node_str.interfaces().len(), 1);
    assert_eq!(node_str.nodes().len(), 4);

    let mut writer = Vec::with_capacity(128);
    node.to_writer(&mut writer).unwrap();

    // Round-trip: the written document parses back to an equal tree.
    let written = String::from_utf8(writer)?;
    let reparsed = Node::try_from(written.as_str())?;
    assert_eq!(node, reparsed);

    Ok(())
}

#[test]
fn invalid_arg_type() {
    let input = include_str!("data/invalid_arg_type.xml");
    assert!(matches!(
        Node::try_from(input),
        Err(zbus_xml::Error::Variant(_))
    ));
}

#[test]
fn multi_complete_arg_type() -> Result<(), Box<dyn Error>> {
    let input = r#"
        <!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
        "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
        <node>
            <interface name="org.test.testinterface">
                <method name="testmethod">
                    <arg name="testarg" direction="out" type="tt"/>
                </method>
            </interface>
        </node>
    "#;

    let node = Node::try_from(input)?;
    let arg = &node.interfaces()[0].methods()[0].args()[0];
    let Signature::Structure(fields) = arg.ty().inner() else {
        panic!("expected `tt` to parse as a structure");
    };

    assert_eq!(fields.len(), 2);
    assert_eq!(fields.get(0), Some(&Signature::U64));
    assert_eq!(fields.get(1), Some(&Signature::U64));

    Ok(())
}

#[test]
fn escaped_attributes() -> Result<(), Box<dyn Error>> {
    let input = r#"
        <node>
            <interface name="org.test.testinterface">
                <annotation name="org.test.Escapes" value="&lt;b&gt; &amp; &quot;q&quot; &apos;a&apos; &#65;&#x42;"/>
            </interface>
        </node>
    "#;

    let node = Node::try_from(input)?;
    let annotation = &node.interfaces()[0].annotations()[0];
    assert_eq!(annotation.value(), r#"<b> & "q" 'a' AB"#);

    // Escaping survives a write/parse round-trip.
    let mut writer = Vec::new();
    node.to_writer(&mut writer)?;
    let written = String::from_utf8(writer)?;
    let reparsed = Node::try_from(written.as_str())?;
    assert_eq!(node, reparsed);

    Ok(())
}

#[test]
fn attribute_whitespace_normalization() -> Result<(), Box<dyn Error>> {
    // Literal whitespace in attribute values is normalized to spaces, while whitespace escaped
    // through character references is kept.
    let input = "<node>\n  <interface name=\"org.test.testinterface\">\n    \
                 <annotation name=\"org.test.Ws\" value=\"a\nb\tc\r&#10;&#9;&#13;d\"/>\n  \
                 </interface>\n</node>";

    let node = Node::try_from(input)?;
    let annotation = &node.interfaces()[0].annotations()[0];
    assert_eq!(annotation.value(), "a b c \n\t\rd");

    // The writer escapes whitespace so it survives normalization by any parser.
    let mut writer = Vec::new();
    node.to_writer(&mut writer)?;
    let written = String::from_utf8(writer)?;
    assert!(written.contains("a b c &#10;&#9;&#13;d"));
    let reparsed = Node::try_from(written.as_str())?;
    assert_eq!(node, reparsed);

    Ok(())
}

#[test]
fn crlf_normalizes_to_single_space() -> Result<(), Box<dyn Error>> {
    // A literal CRLF is a single line ending, so attribute-value normalization collapses it to
    // one space (not two); a lone CR or LF likewise yields one space each.
    let input = "<node name=\"a\r\nb\rc\nd\"/>";
    let node = Node::try_from(input)?;
    assert_eq!(node.name(), Some("a b c d"));

    Ok(())
}

#[test]
fn many_attributes() -> Result<(), Box<dyn Error>> {
    // Elements with many attributes parse fine — duplicate detection falls back from a linear
    // scan to a set past a threshold — and duplicates are still caught beyond that threshold.
    let attrs: String = (0..100).map(|i| format!(" a{i}=\"{i}\"")).collect();
    assert!(Node::try_from(format!("<node{attrs}/>").as_str()).is_ok());
    assert!(matches!(
        Node::try_from(format!("<node{attrs} a50=\"dup\"/>").as_str()),
        Err(zbus_xml::Error::Xml(_))
    ));

    Ok(())
}

#[test]
fn doctype_with_quoted_literals() -> Result<(), Box<dyn Error>> {
    // `>` and brackets inside DOCTYPE quoted literals must not terminate the declaration.
    let input = r#"
        <!DOCTYPE node SYSTEM "weird>literal[with]brackets">
        <node>
            <interface name="org.test.testinterface"/>
        </node>
    "#;

    let node = Node::try_from(input)?;
    assert_eq!(node.interfaces().len(), 1);

    Ok(())
}

#[test]
fn ignores_unknown_elements_and_text() -> Result<(), Box<dyn Error>> {
    // Documents in the wild carry foreign elements (with text content, CDATA and comments) that
    // must be skipped, e.g. Telepathy's `tp:docstring`.
    let input = r#"
        <?xml version="1.0" encoding="UTF-8"?>
        <node xmlns:tp="http://telepathy.freedesktop.org/wiki/DbusSpec#extensions-v0">
            <tp:docstring>Some documentation.</tp:docstring>
            <interface name="org.test.testinterface">
                <!-- a comment -->
                <method name="testmethod">
                    <tp:docstring>More <tp:em>documentation</tp:em>.</tp:docstring>
                    <arg name="testarg" direction="out" type="s">
                        <tp:docstring><![CDATA[Even </more> documentation.]]></tp:docstring>
                    </arg>
                </method>
            </interface>
        </node>
    "#;

    let node = Node::try_from(input)?;
    assert_eq!(node.interfaces().len(), 1);
    let method = &node.interfaces()[0].methods()[0];
    assert_eq!(method.args().len(), 1);
    assert_eq!(method.args()[0].name(), Some("testarg"));

    Ok(())
}

/// The expected shape of an argument: its `name`, signature and `direction`.
type ArgSpec = (Option<&'static str>, &'static str, Option<ArgDirection>);

const IN: Option<ArgDirection> = Some(ArgDirection::In);
const OUT: Option<ArgDirection> = Some(ArgDirection::Out);
/// A signal argument has no direction.
const NONE: Option<ArgDirection> = None;

/// Look up an interface by name, panicking with a helpful message if it is absent.
fn interface<'n, 'a>(node: &'n Node<'a>, name: &str) -> &'n Interface<'a> {
    node.interfaces()
        .iter()
        .find(|iface| iface.name() == name)
        .unwrap_or_else(|| panic!("interface `{name}` not found"))
}

/// Assert that `args` matches `expected` exactly — count, and every name, signature and
/// direction.
fn assert_args(context: &str, args: &[Arg], expected: &[ArgSpec]) {
    assert_eq!(
        args.len(),
        expected.len(),
        "{context}: expected {} arg(s), got {}",
        expected.len(),
        args.len(),
    );
    for (i, (arg, &(name, ty, direction))) in args.iter().zip(expected).enumerate() {
        assert_eq!(arg.name(), name, "{context}: arg {i} name");
        assert!(
            arg.ty() == ty,
            "{context}: arg {i} type: expected `{ty}`, got `{}`",
            arg.ty().to_string(),
        );
        assert_eq!(arg.direction(), direction, "{context}: arg {i} direction");
    }
}

/// Assert that `iface`'s methods are exactly `expected` (name and full argument list each), in
/// no particular order.
fn assert_methods(iface: &Interface<'_>, expected: &[(&str, &[ArgSpec])]) {
    assert_eq!(
        iface.methods().len(),
        expected.len(),
        "{}: method count",
        iface.name(),
    );
    for (name, args) in expected {
        let method = iface
            .methods()
            .iter()
            .find(|m| m.name() == *name)
            .unwrap_or_else(|| panic!("method `{}.{name}` not found", iface.name()));
        assert_args(&format!("{}.{name}", iface.name()), method.args(), args);
    }
}

/// Assert that `iface`'s signals are exactly `expected` (name and full argument list each).
fn assert_signals(iface: &Interface<'_>, expected: &[(&str, &[ArgSpec])]) {
    assert_eq!(
        iface.signals().len(),
        expected.len(),
        "{}: signal count",
        iface.name(),
    );
    for (name, args) in expected {
        let signal = iface
            .signals()
            .iter()
            .find(|s| s.name() == *name)
            .unwrap_or_else(|| panic!("signal `{}.{name}` not found", iface.name()));
        assert_args(&format!("{}.{name}", iface.name()), signal.args(), args);
    }
}

/// Assert that `iface`'s properties are exactly `expected` (name, signature and access each).
fn assert_properties(iface: &Interface<'_>, expected: &[(&str, &str, PropertyAccess)]) {
    assert_eq!(
        iface.properties().len(),
        expected.len(),
        "{}: property count",
        iface.name(),
    );
    for &(name, ty, access) in expected {
        let property = iface
            .properties()
            .iter()
            .find(|p| p.name() == name)
            .unwrap_or_else(|| panic!("property `{}.{name}` not found", iface.name()));
        assert!(
            property.ty() == ty,
            "{}.{name}: type: expected `{ty}`, got `{}`",
            iface.name(),
            property.ty().to_string(),
        );
        assert_eq!(property.access(), access, "{}.{name}: access", iface.name());
    }
}

/// Introspection XML captured verbatim from live services (see `data/real_world/README.md`),
/// covering the XML flavors produced by sd-bus, libdbus and GDBus.
const REAL_WORLD_DATA: &[(&str, &str)] = &[
    (
        "dbus_daemon",
        include_str!("data/real_world/dbus_daemon.xml"),
    ),
    (
        "systemd1_manager",
        include_str!("data/real_world/systemd1_manager.xml"),
    ),
    (
        "systemd1_scope_unit",
        include_str!("data/real_world/systemd1_scope_unit.xml"),
    ),
    (
        "systemd1_unit_list",
        include_str!("data/real_world/systemd1_unit_list.xml"),
    ),
    ("hostname1", include_str!("data/real_world/hostname1.xml")),
    ("timedate1", include_str!("data/real_world/timedate1.xml")),
    ("locale1", include_str!("data/real_world/locale1.xml")),
    ("login1", include_str!("data/real_world/login1.xml")),
    ("network1", include_str!("data/real_world/network1.xml")),
    (
        "polkit1_authority",
        include_str!("data/real_world/polkit1_authority.xml"),
    ),
    ("packagekit", include_str!("data/real_world/packagekit.xml")),
    (
        "dconf_writer",
        include_str!("data/real_world/dconf_writer.xml"),
    ),
];

#[test]
fn real_world_data() -> Result<(), Box<dyn Error>> {
    for (name, xml) in REAL_WORLD_DATA {
        // `h` (file descriptor) signatures are only supported by zvariant on Unix, so documents
        // containing them (e.g. from systemd-logind) don't parse on other platforms.
        if cfg!(not(unix)) && xml.contains(r#"type="h""#) {
            assert!(
                matches!(Node::try_from(*xml), Err(zbus_xml::Error::Variant(_))),
                "{name}: expected fd signatures to be rejected on non-Unix"
            );
            continue;
        }

        let node = Node::try_from(*xml).map_err(|e| format!("{name}: {e}"))?;
        let node_r = Node::from_reader(xml.as_bytes()).map_err(|e| format!("{name}: {e}"))?;
        assert_eq!(node, node_r, "{name}: reader and str parses differ");

        // Every service implements the standard Introspectable interface.
        assert!(
            node.interfaces()
                .iter()
                .any(|i| i.name() == "org.freedesktop.DBus.Introspectable"),
            "{name}: missing org.freedesktop.DBus.Introspectable"
        );

        // The document survives a write/parse round-trip unchanged.
        let mut writer = Vec::new();
        node.to_writer(&mut writer)?;
        let written = String::from_utf8(writer)?;
        let reparsed = Node::try_from(written.as_str()).map_err(|e| format!("{name}: {e}"))?;
        assert_eq!(node, reparsed, "{name}: round-trip changed the tree");
    }

    Ok(())
}

#[test]
fn real_world_dbus_daemon() -> Result<(), Box<dyn Error>> {
    // The dbus-daemon document is small and stable, so check its main interface exhaustively:
    // every method, signal and property, with every argument's name, signature and direction.
    let node = Node::try_from(include_str!("data/real_world/dbus_daemon.xml"))?;
    let dbus = interface(&node, "org.freedesktop.DBus");

    assert_methods(
        dbus,
        &[
            ("Hello", &[(None, "s", OUT)]),
            (
                "RequestName",
                &[(None, "s", IN), (None, "u", IN), (None, "u", OUT)],
            ),
            ("ReleaseName", &[(None, "s", IN), (None, "u", OUT)]),
            (
                "StartServiceByName",
                &[(None, "s", IN), (None, "u", IN), (None, "u", OUT)],
            ),
            ("UpdateActivationEnvironment", &[(None, "a{ss}", IN)]),
            ("NameHasOwner", &[(None, "s", IN), (None, "b", OUT)]),
            ("ListNames", &[(None, "as", OUT)]),
            ("ListActivatableNames", &[(None, "as", OUT)]),
            ("AddMatch", &[(None, "s", IN)]),
            ("RemoveMatch", &[(None, "s", IN)]),
            ("GetNameOwner", &[(None, "s", IN), (None, "s", OUT)]),
            ("ListQueuedOwners", &[(None, "s", IN), (None, "as", OUT)]),
            (
                "GetConnectionUnixUser",
                &[(None, "s", IN), (None, "u", OUT)],
            ),
            (
                "GetConnectionUnixProcessID",
                &[(None, "s", IN), (None, "u", OUT)],
            ),
            (
                "GetAdtAuditSessionData",
                &[(None, "s", IN), (None, "ay", OUT)],
            ),
            (
                "GetConnectionSELinuxSecurityContext",
                &[(None, "s", IN), (None, "ay", OUT)],
            ),
            (
                "GetConnectionAppArmorSecurityContext",
                &[(None, "s", IN), (None, "s", OUT)],
            ),
            ("ReloadConfig", &[]),
            ("GetId", &[(None, "s", OUT)]),
            (
                "GetConnectionCredentials",
                &[(None, "s", IN), (None, "a{sv}", OUT)],
            ),
        ],
    );

    assert_signals(
        dbus,
        &[
            (
                "NameOwnerChanged",
                &[(None, "s", NONE), (None, "s", NONE), (None, "s", NONE)],
            ),
            ("NameLost", &[(None, "s", NONE)]),
            ("NameAcquired", &[(None, "s", NONE)]),
            ("ActivatableServicesChanged", &[]),
        ],
    );

    assert_properties(
        dbus,
        &[
            ("Features", "as", PropertyAccess::Read),
            ("Interfaces", "as", PropertyAccess::Read),
        ],
    );

    // GDBus-style named arguments on the standard Properties interface round-trip too.
    let properties = interface(&node, "org.freedesktop.DBus.Properties");
    assert_signals(
        properties,
        &[(
            "PropertiesChanged",
            &[
                (Some("interface_name"), "s", NONE),
                (Some("changed_properties"), "a{sv}", NONE),
                (Some("invalidated_properties"), "as", NONE),
            ],
        )],
    );

    Ok(())
}

#[test]
fn real_world_systemd() -> Result<(), Box<dyn Error>> {
    // The manager document contains `h` (file descriptor) signatures (e.g.
    // `DumpByFileDescriptor`), which zvariant only supports on Unix.
    if cfg!(unix) {
        let node = Node::try_from(include_str!("data/real_world/systemd1_manager.xml"))?;
        let manager = interface(&node, "org.freedesktop.systemd1.Manager");

        // sd-bus emits named, directioned arguments; check a representative set fully.
        assert_args(
            "Manager.GetUnit",
            manager
                .methods()
                .iter()
                .find(|m| m.name() == "GetUnit")
                .expect("GetUnit method")
                .args(),
            &[(Some("name"), "s", IN), (Some("unit"), "o", OUT)],
        );
        assert_args(
            "Manager.StartUnit",
            manager
                .methods()
                .iter()
                .find(|m| m.name() == "StartUnit")
                .expect("StartUnit method")
                .args(),
            &[
                (Some("name"), "s", IN),
                (Some("mode"), "s", IN),
                (Some("job"), "o", OUT),
            ],
        );
        assert_args(
            "Manager.UnitNew",
            manager
                .signals()
                .iter()
                .find(|s| s.name() == "UnitNew")
                .expect("UnitNew signal")
                .args(),
            &[(Some("id"), "s", NONE), (Some("unit"), "o", NONE)],
        );
        assert_args(
            "Manager.JobNew",
            manager
                .signals()
                .iter()
                .find(|s| s.name() == "JobNew")
                .expect("JobNew signal")
                .args(),
            &[
                (Some("id"), "u", NONE),
                (Some("job"), "o", NONE),
                (Some("unit"), "s", NONE),
            ],
        );

        let version = manager
            .properties()
            .iter()
            .find(|p| p.name() == "Version")
            .expect("Version property");
        assert!(version.ty() == "s", "Version type");
        assert_eq!(version.access(), PropertyAccess::Read);
        assert!(version.access().read() && !version.access().write());
        // sd-bus annotates properties with their change-signalling behavior.
        let emits_changed = version
            .annotations()
            .iter()
            .find(|a| a.name() == "org.freedesktop.DBus.Property.EmitsChangedSignal")
            .expect("EmitsChangedSignal annotation");
        assert_eq!(emits_changed.value(), "const");

        // Writable properties exercise the readwrite access path.
        for name in ["LogLevel", "LogTarget"] {
            let property = manager
                .properties()
                .iter()
                .find(|p| p.name() == name)
                .unwrap_or_else(|| panic!("Manager.{name} property"));
            assert!(property.ty() == "s", "Manager.{name}: type");
            assert_eq!(property.access(), PropertyAccess::ReadWrite);
            assert!(property.access().read() && property.access().write());
        }
    }

    // The unit-list object carries the children of /org/freedesktop/systemd1/unit as named,
    // interface-less sub-nodes.
    let node = Node::try_from(include_str!("data/real_world/systemd1_unit_list.xml"))?;
    assert!(!node.nodes().is_empty());
    let init_scope = node
        .nodes()
        .iter()
        .find(|n| n.name() == Some("init_2escope"))
        .expect("init_2escope node");
    assert!(init_scope.interfaces().is_empty());
    assert!(init_scope.nodes().is_empty());

    // A scope unit implements the generic Unit interface and the type-specific Scope interface,
    // both carrying properties.
    let node = Node::try_from(include_str!("data/real_world/systemd1_scope_unit.xml"))?;
    for name in [
        "org.freedesktop.systemd1.Unit",
        "org.freedesktop.systemd1.Scope",
    ] {
        let iface = interface(&node, name);
        assert!(!iface.properties().is_empty(), "{name}: has properties");
    }
    // The generic Unit interface exposes the well-known Id/ActiveState string properties.
    let unit = interface(&node, "org.freedesktop.systemd1.Unit");
    for name in ["Id", "ActiveState"] {
        let property = unit
            .properties()
            .iter()
            .find(|p| p.name() == name)
            .unwrap_or_else(|| panic!("Unit.{name} property"));
        assert!(property.ty() == "s", "Unit.{name}: type");
        assert!(property.access().read(), "Unit.{name}: readable");
    }

    Ok(())
}

#[test]
fn malformed_documents() {
    for input in [
        // Empty document.
        "",
        // Unclosed root element.
        "<node>",
        // Mismatched closing tag.
        "<node><interface name=\"org.test.testinterface\"></node>",
        // Unterminated comment.
        "<node><!-- comment </node>",
        // Missing attribute value quotes.
        "<node name=foo/>",
        // Unknown entity.
        "<node name=\"&unknown;\"/>",
        // Character references to characters invalid in XML.
        "<node name=\"&#0;\"/>",
        "<node name=\"&#x1F;\"/>",
        // Signed character references.
        "<node name=\"&#+65;\"/>",
        "<node name=\"&#x+41;\"/>",
        // Duplicate attribute.
        "<node name=\"a\" name=\"b\"/>",
        // Missing whitespace between attributes.
        "<node name=\"a\"name=\"b\"/>",
    ] {
        assert!(matches!(
            Node::try_from(input),
            Err(zbus_xml::Error::Xml(_))
        ));
    }

    // Missing required attribute.
    let input = "<node><interface name=\"org.test.testinterface\">\
                 <property name=\"foo\" type=\"s\"/></interface></node>";
    assert!(matches!(
        Node::try_from(input),
        Err(zbus_xml::Error::Xml(_))
    ));

    // Invalid interface name.
    let input = "<node><interface name=\"not a valid name\"/></node>";
    assert!(matches!(
        Node::try_from(input),
        Err(zbus_xml::Error::Name(_))
    ));
}

#[test]
fn error_position() {
    // Errors in attribute values point at the offending reference, not past the value.
    let input = r#"<node name="abc&unknown;def"/>"#;
    let Err(zbus_xml::Error::Xml(e)) = Node::try_from(input) else {
        panic!("expected an XML error");
    };
    assert_eq!(e.position(), input.find('&').unwrap());

    // A duplicate attribute points at the second (offending) name.
    let input = r#"<node name="a" name="b"/>"#;
    let Err(zbus_xml::Error::Xml(e)) = Node::try_from(input) else {
        panic!("expected an XML error");
    };
    assert_eq!(e.position(), input.rfind("name").unwrap());

    // A missing required attribute points at the element name.
    let input = "<node><interface name=\"org.test.I\">\
                 <property name=\"p\" type=\"s\"/></interface></node>";
    let Err(zbus_xml::Error::Xml(e)) = Node::try_from(input) else {
        panic!("expected an XML error");
    };
    assert_eq!(e.position(), input.find("property").unwrap());
}

#[test]
fn deeply_nested_documents() -> Result<(), Box<dyn Error>> {
    // The parser iterates over the axes where a document can nest arbitrarily deep, so any
    // depth up to the cap parses even on a small stack. (Recursing instead overflows even
    // multi-MiB stacks well before the cap in debug builds, whose stack frames are large.)
    let deep_nodes = format!("{}{}", "<node>".repeat(1024), "</node>".repeat(1024));
    let deep_foreign = format!(
        "<node><interface name=\"org.test.testinterface\">{}{}</interface></node>",
        "<x>".repeat(1022),
        "</x>".repeat(1022),
    );

    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(move || {
            let node = Node::try_from(deep_nodes.as_str()).unwrap();
            let mut depth = 1;
            let mut node = &node;
            while let [child] = node.nodes() {
                node = child;
                depth += 1;
            }
            assert_eq!(depth, 1024);

            Node::try_from(deep_foreign.as_str()).unwrap();
        })?
        .join()
        .expect("parsing deeply nested documents must not overflow the stack");

    // One level past the cap is rejected cleanly, on both nesting axes.
    let too_deep = format!("{}{}", "<node>".repeat(1025), "</node>".repeat(1025));
    assert!(matches!(
        Node::try_from(too_deep.as_str()),
        Err(zbus_xml::Error::Xml(_))
    ));
    let too_deep_foreign = format!(
        "<node><interface name=\"org.test.I\">{}{}</interface></node>",
        "<x>".repeat(1025),
        "</x>".repeat(1025),
    );
    assert!(matches!(
        Node::try_from(too_deep_foreign.as_str()),
        Err(zbus_xml::Error::Xml(_))
    ));

    Ok(())
}

#[test]
fn io_error() {
    // A failing reader surfaces as `Error::Io`.
    struct FailingReader;
    impl std::io::Read for FailingReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("boom"))
        }
    }
    assert!(matches!(
        Node::from_reader(FailingReader),
        Err(zbus_xml::Error::Io(_))
    ));
}

#[test]
fn invalid_enum_attributes() -> Result<(), Box<dyn Error>> {
    // An invalid `direction` or `access` value is a clean XML error.
    let bad_direction = "<node><interface name=\"org.test.I\">\
                         <method name=\"M\"><arg type=\"s\" direction=\"sideways\"/></method>\
                         </interface></node>";
    assert!(matches!(
        Node::try_from(bad_direction),
        Err(zbus_xml::Error::Xml(_))
    ));

    let bad_access = "<node><interface name=\"org.test.I\">\
                      <property name=\"P\" type=\"s\" access=\"rw\"/></interface></node>";
    assert!(matches!(
        Node::try_from(bad_access),
        Err(zbus_xml::Error::Xml(_))
    ));

    // A write-only property exercises the `PropertyAccess::Write` arm.
    let write_only = "<node><interface name=\"org.test.I\">\
                      <property name=\"P\" type=\"s\" access=\"write\"/></interface></node>";
    let node = Node::try_from(write_only)?;
    let access = node.interfaces()[0].properties()[0].access();
    assert_eq!(access, PropertyAccess::Write);
    assert!(access.write() && !access.read());

    Ok(())
}

#[test]
fn invalid_member_names() {
    // An invalid method or signal name is an `Error::Name` (like the interface-name case in
    // `malformed_documents`), exercising the same validated-name path for both members.
    for input in [
        "<node><interface name=\"org.test.I\"><method name=\"not valid\"/></interface></node>",
        "<node><interface name=\"org.test.I\"><signal name=\"not valid\"/></interface></node>",
    ] {
        assert!(matches!(
            Node::try_from(input),
            Err(zbus_xml::Error::Name(_))
        ));
    }
}

#[test]
fn root_element_name_is_ignored() -> Result<(), Box<dyn Error>> {
    // The root element's name is not checked, for compatibility with servers that don't name it
    // `node` (and with how the previous quick-xml-based versions behaved).
    let node = Node::try_from("<weirdroot><interface name=\"org.test.I\"/></weirdroot>")?;
    assert_eq!(node.interfaces().len(), 1);
    assert_eq!(node.interfaces()[0].name(), "org.test.I");

    Ok(())
}

#[test]
fn docstrings() -> Result<(), Box<dyn Error>> {
    // Telepathy-style docstrings are captured on interfaces, members and args, verbatim
    // (including markup and references) but with the surrounding whitespace trimmed.
    let input = r#"
        <node xmlns:tp="http://telepathy.freedesktop.org/wiki/DbusSpec#extensions-v0">
            <interface name="org.test.testinterface">
                <tp:docstring>
                    <p>An interface, closely modeled after the
                    <strong>MPRIS</strong> ones &amp; documented inline.</p>
                    <tp:rationale>
                        <p>With a nested rationale.</p>
                    </tp:rationale>
                </tp:docstring>
                <method name="TestMethod">
                    <tp:docstring>Does the thing.</tp:docstring>
                    <arg name="testarg" direction="in" type="s">
                        <tp:docstring>The thing to do.</tp:docstring>
                    </arg>
                    <arg name="undocumented" direction="out" type="u"/>
                </method>
                <signal name="TestSignal">
                    <tp:docstring>Emitted when the thing was done.</tp:docstring>
                    <arg name="testarg" type="s">
                        <tp:docstring>The thing that was done.</tp:docstring>
                    </arg>
                </signal>
                <property name="TestProperty" type="b" access="read">
                    <tp:docstring>Whether the thing has been done.</tp:docstring>
                </property>
                <property name="EmptyDocstring" type="b" access="read">
                    <tp:docstring>   </tp:docstring>
                    <tp:docstring/>
                </property>
            </interface>
        </node>
    "#;

    let node = Node::try_from(input)?;
    let interface = &node.interfaces()[0];
    let docstring = interface.docstring().expect("interface docstring");
    assert!(docstring.starts_with("<p>An interface,"));
    assert!(docstring.contains("<strong>MPRIS</strong> ones &amp; documented inline.</p>"));
    assert!(docstring.contains("<tp:rationale>"));
    assert!(docstring.ends_with("</tp:rationale>"));

    let method = &interface.methods()[0];
    assert_eq!(method.docstring(), Some("Does the thing."));
    assert_eq!(method.args()[0].docstring(), Some("The thing to do."));
    assert_eq!(method.args()[1].docstring(), None);

    let signal = &interface.signals()[0];
    assert_eq!(signal.docstring(), Some("Emitted when the thing was done."));
    assert_eq!(
        signal.args()[0].docstring(),
        Some("The thing that was done.")
    );

    assert_eq!(
        interface.properties()[0].docstring(),
        Some("Whether the thing has been done.")
    );
    // Whitespace-only and self-closing docstrings count as absent.
    assert_eq!(interface.properties()[1].docstring(), None);

    Ok(())
}

#[test]
fn unsupported_element_warnings() -> Result<(), Box<dyn Error>> {
    // Elements that have no place in the introspection format are skipped with a warning.
    let input = r#"
        <node xmlns:tp="http://telepathy.freedesktop.org/wiki/DbusSpec#extensions-v0">
            <tp:flags name="Media_Caps" value-prefix="Cap" type="u">
                <tp:flag suffix="Audio" value="1"/>
            </tp:flags>
            <interface name="org.test.testinterface">
                <tp:docstring>Documented, not warned about.</tp:docstring>
                <method name="TestMethod">
                    <tp:possible-errors/>
                </method>
                <annotation name="org.test.Annotation" value="v">
                    <unexpected/>
                </annotation>
            </interface>
        </node>
    "#;

    let (node, warnings) = Node::from_reader_with_warnings(input.as_bytes())?;
    assert_eq!(node.interfaces().len(), 1);
    assert_eq!(
        node.interfaces()[0].docstring(),
        Some("Documented, not warned about.")
    );

    let elements: Vec<_> = warnings.iter().map(|w| w.element()).collect();
    assert_eq!(elements, ["tp:flags", "tp:possible-errors", "unexpected"]);
    // Nested children of a skipped element (`tp:flag`) are not warned about separately, and
    // warnings point at the start of the offending element.
    assert_eq!(warnings[0].position(), input.find("<tp:flags").unwrap());
    assert!(warnings[0].message().contains("`<tp:flags>`"));
    assert!(warnings[0].to_string().contains("`<tp:flags>`"));

    // The warning-less API still just ignores everything.
    assert_eq!(Node::try_from(input)?, node);

    Ok(())
}

#[test]
fn telepathy_type_definitions() -> Result<(), Box<dyn Error>> {
    let input = r#"
        <node xmlns:tp="http://telepathy.freedesktop.org/wiki/DbusSpec#extensions-v0">
            <tp:simple-type name="Playlist_Id" type="o" array-name="Playlist_Id_List">
                <tp:docstring>Unique playlist identifier.</tp:docstring>
            </tp:simple-type>
            <interface name="org.test.testinterface">
                <tp:enum name="Playlist_Ordering" type="s">
                    <tp:docstring>The way to order playlists.</tp:docstring>
                    <tp:enumvalue suffix="Alphabetical" value="Alphabetical">
                        <tp:docstring>Alphabetical ordering, ascending.</tp:docstring>
                    </tp:enumvalue>
                    <tp:enumvalue suffix="UserDefined" value="User"/>
                </tp:enum>
                <tp:struct name="Playlist" array-name="Playlist_List">
                    <tp:docstring>A data structure describing a playlist.</tp:docstring>
                    <tp:member type="o" tp:type="Playlist_Id" name="Id">
                        <tp:docstring>A unique identifier.</tp:docstring>
                    </tp:member>
                    <tp:member type="s" name="Name"/>
                </tp:struct>
                <tp:mapping name="String_Variant_Map">
                    <tp:member type="s" name="Key"/>
                    <tp:member type="v" name="Value"/>
                </tp:mapping>
                <method name="GetPlaylists">
                    <arg direction="out" name="playlists" type="a(os)" tp:type="Playlist[]"/>
                </method>
                <property name="Orderings" type="as" tp:type="Playlist_Ordering[]" access="read"/>
            </interface>
        </node>
    "#;

    let (node, warnings) = Node::from_reader_with_warnings(input.as_bytes())?;
    assert_eq!(warnings, []);

    use zbus_xml::telepathy::TypeDef;

    // Node-level definitions.
    let [TypeDef::SimpleType(id)] = node.telepathy_types() else {
        panic!("expected a simple type on the node");
    };
    assert_eq!(id.name(), "Playlist_Id");
    assert_eq!(*id.ty().inner(), Signature::ObjectPath);
    assert_eq!(id.docstring(), Some("Unique playlist identifier."));

    // Interface-level definitions.
    let interface = &node.interfaces()[0];
    let [
        TypeDef::Enum(ordering),
        TypeDef::Struct(playlist),
        TypeDef::Mapping(map),
    ] = interface.telepathy_types()
    else {
        panic!("expected enum, struct and mapping on the interface");
    };

    assert_eq!(ordering.name(), "Playlist_Ordering");
    assert_eq!(*ordering.ty().inner(), Signature::Str);
    assert_eq!(ordering.docstring(), Some("The way to order playlists."));
    assert_eq!(ordering.values().len(), 2);
    assert_eq!(ordering.values()[0].suffix(), "Alphabetical");
    assert_eq!(ordering.values()[0].value(), "Alphabetical");
    assert_eq!(
        ordering.values()[0].docstring(),
        Some("Alphabetical ordering, ascending.")
    );
    assert_eq!(ordering.values()[1].suffix(), "UserDefined");
    assert_eq!(ordering.values()[1].value(), "User");
    assert_eq!(ordering.values()[1].docstring(), None);

    assert_eq!(playlist.name(), "Playlist");
    assert_eq!(
        playlist.docstring(),
        Some("A data structure describing a playlist.")
    );
    assert_eq!(playlist.members().len(), 2);
    assert_eq!(playlist.members()[0].name(), "Id");
    assert_eq!(*playlist.members()[0].ty().inner(), Signature::ObjectPath);
    assert_eq!(playlist.members()[0].tp_type(), Some("Playlist_Id"));
    assert_eq!(
        playlist.members()[0].docstring(),
        Some("A unique identifier.")
    );
    assert_eq!(playlist.members()[1].tp_type(), None);
    assert_eq!(playlist.signature().to_string(), "(os)");

    assert_eq!(map.name(), "String_Variant_Map");
    assert_eq!(map.key().name(), "Key");
    assert_eq!(map.value().name(), "Value");
    assert_eq!(map.signature().to_string(), "a{sv}");

    // `tp:type` references on args and properties.
    let method = &interface.methods()[0];
    assert_eq!(method.args()[0].tp_type(), Some("Playlist[]"));
    assert_eq!(
        interface.properties()[0].tp_type(),
        Some("Playlist_Ordering[]")
    );

    Ok(())
}

#[test]
fn malformed_telepathy_type_definitions() -> Result<(), Box<dyn Error>> {
    // A type definition that cannot be parsed doesn't fail the document; it is skipped with a
    // warning.
    let input = r#"
        <node xmlns:tp="http://telepathy.freedesktop.org/wiki/DbusSpec#extensions-v0">
            <tp:simple-type type="o"/>
            <tp:simple-type name="Bad_Type" type="!"/>
            <tp:simple-type name="Empty_Type" type=""/>
            <tp:enum name="No_Type">
                <tp:enumvalue suffix="A" value="0"/>
            </tp:enum>
            <tp:enum name="Bad_Value" type="u">
                <tp:enumvalue value="0"/>
            </tp:enum>
            <tp:struct name="Empty"/>
            <tp:struct name="Bad_Member">
                <tp:member name="No_Type_Either"/>
            </tp:struct>
            <tp:mapping name="Not_A_Pair">
                <tp:member type="s" name="Key"/>
            </tp:mapping>
            <tp:mapping name="Bad_Key">
                <tp:member type="v" name="Key"/>
                <tp:member type="s" name="Value"/>
            </tp:mapping>
            <interface name="org.test.testinterface"/>
        </node>
    "#;

    let (node, warnings) = Node::from_reader_with_warnings(input.as_bytes())?;
    assert_eq!(node.telepathy_types(), []);
    assert_eq!(node.interfaces().len(), 1);

    let messages: Vec<_> = warnings.iter().map(|w| w.message()).collect();
    assert_eq!(
        messages,
        [
            "malformed element `<tp:simple-type>` ignored: missing attribute `name`",
            "malformed element `<tp:simple-type>` ignored: invalid signature in `type`",
            "malformed element `<tp:simple-type>` ignored: invalid signature in `type`",
            "malformed element `<tp:enum>` ignored: missing attribute `type`",
            "malformed element `<tp:enum>` ignored: an enumvalue is missing its `suffix` or \
             `value` attribute",
            "malformed element `<tp:struct>` ignored: no members",
            "malformed element `<tp:struct>` ignored: a member is missing its `name` or `type` \
             attribute, or has an invalid signature",
            "malformed element `<tp:mapping>` ignored: expected exactly 2 members",
            "malformed element `<tp:mapping>` ignored: the key is not a basic type",
        ]
    );
    assert_eq!(
        warnings[0].position(),
        input.find("<tp:simple-type").unwrap()
    );

    // An empty struct member type composes into an invalid struct signature, so it must be
    // rejected too.
    let input = r#"
        <node xmlns:tp="http://telepathy.freedesktop.org/wiki/DbusSpec#extensions-v0">
            <tp:struct name="S"><tp:member name="m" type=""/></tp:struct>
        </node>
    "#;
    let (node, warnings) = Node::from_reader_with_warnings(input.as_bytes())?;
    assert_eq!(node.telepathy_types(), []);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].message().contains("`<tp:struct>`"));

    Ok(())
}

#[test]
fn unknown_children_of_telepathy_definitions() -> Result<(), Box<dyn Error>> {
    // Unknown elements are warned about wherever they appear, including inside the leaf
    // elements of type definitions.
    let input = r#"
        <node xmlns:tp="http://telepathy.freedesktop.org/wiki/DbusSpec#extensions-v0">
            <tp:simple-type name="Token" type="s">
                <tp:added version="0.1"/>
            </tp:simple-type>
            <interface name="org.test.testinterface">
                <tp:enum name="Level" type="u">
                    <tp:enumvalue suffix="Low" value="0">
                        <tp:changed version="0.2"/>
                    </tp:enumvalue>
                </tp:enum>
                <tp:struct name="Pair">
                    <tp:member name="first" type="s">
                        <bogus/>
                    </tp:member>
                    <tp:member name="second" type="s"/>
                </tp:struct>
            </interface>
        </node>
    "#;

    let (node, warnings) = Node::from_reader_with_warnings(input.as_bytes())?;
    // The definitions themselves still parse fine.
    assert_eq!(node.telepathy_types().len(), 1);
    assert_eq!(node.interfaces()[0].telepathy_types().len(), 2);

    let elements: Vec<_> = warnings.iter().map(|w| w.element()).collect();
    assert_eq!(elements, ["tp:added", "tp:changed", "bogus"]);

    Ok(())
}
