use std::collections::{BTreeMap, HashMap};

use endi::NATIVE_ENDIAN;
use zvariant::{
    DeserializeDict, Dict, SerializeDict, Str, Type, Value, as_value::optional,
    serialized::Context, to_bytes,
};

#[macro_use]
mod common {
    include!("common.rs");
}

#[test]
fn dict_value() {
    let mut map: HashMap<i64, &str> = HashMap::new();
    map.insert(1, "123");
    map.insert(2, "456");
    let ctxt = Context::new_dbus(NATIVE_ENDIAN, 0);
    let encoded = to_bytes(ctxt, &map).unwrap();
    assert_eq!(dbg!(encoded.len()), 40);
    let decoded: HashMap<i64, &str> = encoded.deserialize().unwrap().0;
    assert_eq!(decoded[&1], "123");
    assert_eq!(decoded[&2], "456");

    // GVariant format now
    #[cfg(feature = "gvariant")]
    {
        let ctxt = Context::new_gvariant(NATIVE_ENDIAN, 0);
        let gv_encoded = to_bytes(ctxt, &map).unwrap();
        assert_eq!(gv_encoded.len(), 30);
        let map: HashMap<i64, &str> = encoded.deserialize().unwrap().0;
        assert_eq!(map[&1], "123");
        assert_eq!(map[&2], "456");
    }
    let ctxt = Context::new_dbus(NATIVE_ENDIAN, 0);

    // As Value
    let v: Value<'_> = Dict::from(map).into();
    assert_eq!(v.value_signature(), "a{xs}");
    let encoded = to_bytes(ctxt, &v).unwrap();
    assert_eq!(encoded.len(), 48);
    // Convert it back
    let dict: Dict<'_, '_> = v.try_into().unwrap();
    let map: HashMap<i64, String> = dict.try_clone().unwrap().try_into().unwrap();
    assert_eq!(map[&1], "123");
    assert_eq!(map[&2], "456");
    // Also decode it back
    let v = encoded.deserialize().unwrap().0;
    if let Value::Dict(dict) = v {
        assert_eq!(dict.get::<i64, &str>(&1).unwrap().unwrap(), "123");
        assert_eq!(dict.get::<i64, &str>(&2).unwrap().unwrap(), "456");
    } else {
        panic!();
    }
    // Convert it to a BTreeMap too.
    let map: BTreeMap<i64, String> = dict.try_into().unwrap();
    assert_eq!(map[&1], "123");
    assert_eq!(map[&2], "456");
    // Use iterator
    let mut dict = Dict::from(map);
    let expect = [
        (Value::from(1i64), Value::from("123")),
        (Value::from(2i64), Value::from("456")),
    ];
    let expect_iter = expect.iter().map(|(k, v)| (k, v)).collect::<Vec<_>>();
    let actual = dict.iter().collect::<Vec<_>>();
    assert_eq!(actual, expect_iter);
    let actual = dict.iter().collect::<Vec<_>>();
    assert_eq!(actual, expect_iter);
    let actual = dict.iter().collect::<Vec<_>>();
    assert_eq!(actual, expect_iter);
    for (_, v) in dict.iter_mut() {
        if let Value::Str(vv) = v {
            *vv = Str::from(vv.to_string() + "-hello");
        }
    }
    let actual = dict.into_iter().collect::<Vec<_>>();
    let expect = vec![
        (Value::from(1i64), Value::from("123-hello")),
        (Value::from(2i64), Value::from("456-hello")),
    ];
    assert_eq!(actual, expect);

    #[cfg(feature = "gvariant")]
    {
        // GVariant-format requires framing offsets for dict entries with variable-length keys
        // so let's test that.
        let mut map: HashMap<&str, &str> = HashMap::new();
        map.insert("hi", "1234");
        map.insert("world", "561");
        let ctxt = Context::new_gvariant(NATIVE_ENDIAN, 0);
        let gv_encoded = to_bytes(ctxt, &map).unwrap();
        assert_eq!(gv_encoded.len(), 22);
        let map: HashMap<&str, &str> = gv_encoded.deserialize().unwrap().0;
        assert_eq!(map["hi"], "1234");
        assert_eq!(map["world"], "561");

        // Ensure SerializeValue produces the same result as Value
        // Tests for https://github.com/z-galaxy/zbus/issues/868
        let mut map = std::collections::HashMap::<&str, &str>::new();
        map.insert("k", "v");
        let gv_ser_value_encoded =
            zvariant::to_bytes(ctxt, &zvariant::as_value::Serialize(&map)).unwrap();
        let gv_value_encoded = to_bytes(ctxt, &zvariant::Value::new(map)).unwrap();
        assert_eq!(*gv_value_encoded, *gv_ser_value_encoded);

        // Now the same but empty dict this time
        let map: HashMap<&str, &str> = HashMap::new();
        let gv_encoded = to_bytes(ctxt, &map).unwrap();
        assert_eq!(gv_encoded.len(), 0);
        let map: HashMap<&str, &str> = gv_encoded.deserialize().unwrap().0;
        assert_eq!(map.len(), 0);
    }
    let ctxt = Context::new_dbus(NATIVE_ENDIAN, 0);

    // Now a hand-crafted Dict Value but with a Value as value
    let mut dict = Dict::new(<&str>::SIGNATURE, Value::SIGNATURE);
    dict.add("hello", Value::new("there")).unwrap();
    dict.add("bye", Value::new("now")).unwrap();
    let v: Value<'_> = dict.into();
    assert_eq!(v.value_signature(), "a{sv}");
    let encoded = to_bytes(ctxt, &v).unwrap();
    assert_eq!(dbg!(encoded.len()), 66);
    let v: Value<'_> = encoded.deserialize().unwrap().0;
    if let Value::Dict(dict) = v {
        assert_eq!(
            dict.get::<&str, Value<'_>>(&"hello").unwrap().unwrap(),
            Value::new("there")
        );
        assert_eq!(
            dict.get::<_, Value<'_>>(&"bye").unwrap().unwrap(),
            Value::new("now")
        );

        // Try converting to a HashMap
        let map = <HashMap<String, Value<'_>>>::try_from(dict.try_clone().unwrap()).unwrap();
        assert_eq!(map["hello"], Value::new("there"));
        assert_eq!(map["bye"], Value::new("now"));

        // Try converting to a BTreeMap
        let map = <BTreeMap<String, Value<'_>>>::try_from(dict).unwrap();
        assert_eq!(map["hello"], Value::new("there"));
        assert_eq!(map["bye"], Value::new("now"));
    } else {
        panic!();
    }

    #[derive(DeserializeDict, SerializeDict, Type, PartialEq, Debug, Default)]
    #[zvariant(signature = "a{sv}")]
    struct Test {
        process_id: Option<u32>,
        group_id: Option<u32>,
        user: String,
    }

    let test = Test {
        process_id: Some(42),
        group_id: None,
        user: "me".to_string(),
    };

    let encoded = to_bytes(ctxt, &test).unwrap();
    assert_eq!(encoded.len(), 51);

    let decoded: HashMap<&str, Value<'_>> = encoded.deserialize().unwrap().0;
    assert_eq!(decoded["process_id"], Value::U32(42));
    assert_eq!(decoded["user"], Value::new("me"));
    assert!(!decoded.contains_key("group_id"));

    let decoded: Test = encoded.deserialize().unwrap().0;
    assert_eq!(decoded, test);

    #[derive(DeserializeDict, SerializeDict, Type, PartialEq, Debug)]
    #[zvariant(signature = "a{sv}")]
    struct TestMissing {
        process_id: Option<u32>,
        group_id: Option<u32>,
        user: String,
        quota: u8,
    }
    let decoded = encoded.deserialize::<TestMissing>();
    assert!(decoded.is_err());

    #[derive(DeserializeDict, SerializeDict, Type, PartialEq, Debug, Default)]
    #[zvariant(signature = "a{sv}")]
    struct TestSkipUnknown {
        process_id: Option<u32>,
        group_id: Option<u32>,
    }
    let _: TestSkipUnknown = encoded.deserialize().unwrap().0;

    #[derive(DeserializeDict, SerializeDict, Type, PartialEq, Debug, Default)]
    #[zvariant(signature = "a{sv}", deny_unknown_fields)]
    struct TestDenyUnknown {
        process_id: Option<u32>,
        group_id: Option<u32>,
    }
    let decoded = encoded.deserialize::<TestDenyUnknown>();
    assert!(decoded.is_err());

    #[derive(serde::Serialize, serde::Deserialize, Type, PartialEq, Debug, Default)]
    #[serde(default)]
    #[zvariant(signature = "a{sv}")]
    struct TestParseUnknown<'s> {
        #[serde(with = "optional", skip_serializing_if = "Option::is_none")]
        process_id: Option<u32>,
        #[serde(with = "optional", skip_serializing_if = "Option::is_none")]
        group_id: Option<u32>,
        #[serde(flatten, borrow)]
        rest: HashMap<&'s str, Value<'s>>,
    }
    let decoded: TestParseUnknown<'_> = encoded.deserialize().unwrap().0;
    assert_eq!(decoded.rest.len(), 1);
    assert_eq!(decoded.rest["user"], Value::new("me"));

    #[cfg(feature = "gvariant")]
    {
        let test = Test {
            process_id: Some(42),
            group_id: None,
            user: "me".to_string(),
        };

        let ctxt = Context::new_gvariant(NATIVE_ENDIAN, 0);
        let encoded = to_bytes(ctxt, &test).unwrap();
        let _: Test = encoded.deserialize().unwrap().0;
        let decoded = encoded.deserialize::<TestMissing>();
        assert!(decoded.is_err());
        let _: TestSkipUnknown = encoded.deserialize().unwrap().0;
        let decoded = encoded.deserialize::<TestDenyUnknown>();
        assert!(decoded.is_err());
    }

    #[derive(Default, Debug, SerializeDict, DeserializeDict, Type)]
    #[zvariant(signature = "dict")]
    struct TestEmpty {}

    // Empty dict should be serialized to contain 4 bytes for the ARRAY length and 4 bytes for the
    // DICT_ENTRY alignment to 8 bytes boundary.
    let data = to_bytes(ctxt, &TestEmpty::default()).unwrap();

    assert_eq!(data.bytes(), &[0, 0, 0, 0, 0, 0, 0, 0]);
}
