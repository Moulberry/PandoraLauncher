#![cfg_attr(feature = "gvariant", allow(deprecated))]

use zvariant::LE;
#[cfg(feature = "gvariant")]
use zvariant::{serialized::Context, to_bytes};

#[test]
fn bool_value() {
    let encoded = basic_type_test!(LE, DBus, true, 4, bool, 4, Bool, 8);
    assert_eq!(encoded.len(), 4);

    #[cfg(feature = "gvariant")]
    {
        let gvariant = basic_type_test!(LE, GVariant, true, 1, bool, 1, Bool, 3);
        assert_eq!(*gvariant.bytes(), [1]);
    }
}

#[test]
#[cfg(feature = "gvariant")]
fn bool_maybe_array_value() {
    let ctxt = Context::new_gvariant(LE, 0);
    let encoded = to_bytes(ctxt, &vec![Some(true); 3]).unwrap();
    assert_eq!(encoded.bytes(), b"\x01\x01\x01\x01\x02\x03");
    let decoded: Vec<Option<bool>> = encoded.deserialize().unwrap().0;
    assert_eq!(decoded, vec![Some(true); 3]);
}
