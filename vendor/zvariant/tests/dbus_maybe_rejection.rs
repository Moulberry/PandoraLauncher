//! A GVariant maybe (`m`) type is not valid in the D-Bus wire format. When the maybe type is
//! available (either via zvariant's own `gvariant` feature or via `zvariant_utils/gvariant`
//! enabled elsewhere in the graph, e.g. by `zgvariant`), such a signature can still be parsed,
//! so the D-Bus serializer and deserializer must reject it with an error rather than panicking
//! or emitting invalid data. A maybe can enter as the codec's layout signature, or dynamically
//! as the content of a `g` (signature) or `v` (variant) value.

use zvariant::{
    LE, Signature, Value,
    serialized::{Context, Data},
    to_bytes_for_signature,
};

#[test]
fn dbus_deserialize_rejects_maybe_layout_signatures() {
    if !maybe_supported() {
        return;
    }
    for s in MAYBE_SIGNATURES {
        let sig = Signature::try_from(*s).unwrap();
        let bytes = [0u8, 0, 0, 0];
        let ctxt = Context::new_dbus(LE, 0);
        let data = Data::new(&bytes[..], ctxt);
        let res: zvariant::Result<(Vec<i32>, usize)> = data.deserialize_for_signature(&sig);
        assert_maybe_rejected(res, &format!("deserialize layout {s:?}"));
    }
}

#[test]
fn dbus_serialize_rejects_maybe_layout_signatures() {
    if !maybe_supported() {
        return;
    }
    for s in MAYBE_SIGNATURES {
        let sig = Signature::try_from(*s).unwrap();
        let ctxt = Context::new_dbus(LE, 0);
        assert_maybe_rejected(
            to_bytes_for_signature(ctxt, &sig, &0u8),
            &format!("serialize layout {s:?}"),
        );
    }
}

// A `g` (signature) *value* whose content carries a maybe type: the codec's layout is `g`, not a
// maybe, so this exercises the dynamic string-value branch rather than the layout check.
#[test]
fn dbus_rejects_signature_value_carrying_maybe() {
    if !maybe_supported() {
        return;
    }
    let g = Signature::try_from("g").unwrap();
    let maybe_sig = Signature::try_from("mi").unwrap();
    let ctxt = Context::new_dbus(LE, 0);

    assert_maybe_rejected(
        to_bytes_for_signature(ctxt, &g, &maybe_sig),
        "serialize `g` value",
    );

    // A `g` value on the wire: length byte, "mi", trailing nul.
    let bytes = [2u8, b'm', b'i', 0];
    let data = Data::new(&bytes[..], ctxt);
    let res: zvariant::Result<(Signature, usize)> = data.deserialize_for_signature(&g);
    assert_maybe_rejected(res, "deserialize `g` value");
}

// An actual `v` (variant) whose embedded signature carries a maybe: the layout is `v`, and the
// maybe only appears in the signature read off the wire. The `ami` payload is sized so that,
// absent the dynamic check, deserialization would reach the child-alignment `unreachable!()`: the
// 5-byte variant header (`\x03ami\0`) is followed by 3 bytes of array padding and the 4-byte array
// length, after which the maybe child's alignment is computed.
#[test]
fn dbus_deserialize_rejects_variant_carrying_maybe() {
    if !maybe_supported() {
        return;
    }
    let v = Signature::try_from("v").unwrap();
    let ctxt = Context::new_dbus(LE, 0);
    let bytes = [3u8, b'a', b'm', b'i', 0, 0, 0, 0, 0, 0, 0, 0];
    let data = Data::new(&bytes[..], ctxt);
    let res: zvariant::Result<(Value<'_>, usize)> = data.deserialize_for_signature(&v);
    assert_maybe_rejected(res, "deserialize variant carrying maybe");
}

// The maybe type is unavailable in a plain zvariant build (no own `gvariant` feature and no
// `zvariant_utils/gvariant` in the graph): `m` does not parse, so the hazard cannot arise and
// there is nothing to assert.
fn maybe_supported() -> bool {
    Signature::try_from("mi").is_ok()
}

// The D-Bus boundary's own maybe-rejection carries this wording, distinct from the direct-`Maybe`
// deserialize fallback, so asserting it confirms the intended branch fired (not merely any error).
fn assert_maybe_rejected<T>(res: zvariant::Result<T>, ctx: &str) {
    let err = res
        .err()
        .unwrap_or_else(|| panic!("{ctx}: expected an error, got Ok"));
    let msg = err.to_string();
    assert!(
        msg.contains("not valid in the D-Bus format"),
        "{ctx}: unexpected error: {msg}"
    );
}

// Maybe types nested at every position a D-Bus container can reach.
const MAYBE_SIGNATURES: &[&str] = &["mi", "ami", "(mi)", "a{smi}"];
