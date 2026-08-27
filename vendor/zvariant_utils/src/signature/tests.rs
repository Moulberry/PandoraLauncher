use super::*;
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    str::FromStr,
};

macro_rules! validate {
        ($($signature:literal => $expected:expr),+) => {
            $(
                assert!(validate($signature.as_bytes()).is_ok());
                let parsed = Signature::from_str($signature).unwrap();
                assert_eq!(parsed, $expected);
                assert_eq!(parsed, $signature);
                match parsed {
                    Signature::Structure(_) => {
                        assert!(
                            $signature.len() == parsed.string_len() - 2 ||
                            $signature.len() == parsed.string_len()
                        );
                    }
                    _ => {
                        assert_eq!(parsed.string_len(), $signature.len());
                    }
                }
            )+
        };
    }

#[test]
fn validate_strings() {
    validate!(
        "" => Signature::Unit,
        "y" => Signature::U8,
        "b" => Signature::Bool,
        "n" => Signature::I16,
        "q" => Signature::U16,
        "i" => Signature::I32,
        "u" => Signature::U32,
        "x" => Signature::I64,
        "t" => Signature::U64,
        "d" => Signature::F64,
        "s" => Signature::Str,
        "g" => Signature::Signature,
        "o" => Signature::ObjectPath,
        "v" => Signature::Variant,
        "xs" => Signature::structure(&[&Signature::I64, &Signature::Str][..]),
        "(ysa{sd})" => Signature::static_structure(
            &[
                &Signature::U8,
                &Signature::Str,
                &Signature::Dict {
                    key: Child::Static { child: &Signature::Str },
                    value: Child::Static { child: &Signature::F64 },
                },
            ]
        ),
        "a(y)" => Signature::static_array(
            &Signature::Structure(Fields::Static { fields: &[&Signature::U8] }),
        ),
        "a{yy}" => Signature::static_dict(&Signature::U8, &Signature::U8),
        "(yy)" => Signature::static_structure(&[&Signature::U8, &Signature::U8]),
        "a{sd}" => Signature::static_dict(&Signature::Str, &Signature::F64),
        "a{yy}" => Signature::static_dict(&Signature::U8, &Signature::U8),
        "a{sv}" => Signature::static_dict(&Signature::Str, &Signature::Variant),
        "a{sa{sv}}" => Signature::static_dict(
            &Signature::Str,
            &Signature::Dict {
                key: Child::Static {
                child: &Signature::Str,
                },
                value: Child::Static {
                child: &Signature::Variant
                }
            },
        ),
        "a{sa(ux)}" => Signature::static_dict(
            &Signature::Str,
            &Signature::Array(Child::Static {
                child: &Signature::Structure(Fields::Static {
                    fields: &[&Signature::U32, &Signature::I64]
                }),
            }),
        ),
        "(x)" => Signature::static_structure(&[&Signature::I64]),
        "(x(isy))" => Signature::static_structure(&[
            &Signature::I64,
            &Signature::Structure(Fields::Static {
                fields: &[&Signature::I32, &Signature::Str, &Signature::U8]
            }),
        ]),
        "(xa(isy))" => Signature::static_structure(&[
            &Signature::I64,
            &Signature::Array(Child::Static {
                child: &Signature::Structure(Fields::Static {
                    fields: &[&Signature::I32, &Signature::Str, &Signature::U8]
                }),
            }),
        ]),
        "(xa(s))" => Signature::static_structure(&[
            &Signature::I64,
            &Signature::Array(Child::Static {
                child: &Signature::Structure(Fields::Static {
                    fields: &[&Signature::Str]
                }),
            }),
        ]),
        "((yyyyuu)a(yv))" => Signature::static_structure(&[
            &Signature::Structure(Fields::Static {
                fields: &[
                    &Signature::U8, &Signature::U8, &Signature::U8, &Signature::U8,
                    &Signature::U32, &Signature::U32,
                ],
            }),
            &Signature::Array(Child::Static {
                child: &Signature::Structure(Fields::Static {
                    fields: &[&Signature::U8, &Signature::Variant],
                }),
            }),
        ])
    );
    #[cfg(unix)]
    validate!("h" => Signature::Fd);
}

macro_rules! invalidate {
        ($($signature:literal),+) => {
            $(
                assert!(validate($signature.as_bytes()).is_err());
            )+
        };
    }

#[test]
fn invalid_strings() {
    invalidate!(
        "a",
        "a{}",
        "a{y",
        "a{y}",
        "a{y}a{y}",
        "a{y}a{y}a{y}",
        "z",
        "()",
        "(x",
        "(x())",
        "(xa()",
        "(xa(s)",
        "(xs",
        "xs)",
        "s/",
        "a{yz}"
    );
}

#[test]
fn recursion_limits() {
    // Deeply nested structs or arrays are rejected rather than overflowing the stack — the
    // parser fails as soon as the limit is passed, without recursing through the whole string.
    // Both the parsing and the allocation-free `check_only` (`validate`) paths reject them.
    let deep_structs = format!("{}i{}", "(".repeat(20_000), ")".repeat(20_000));
    assert!(Signature::from_str(&deep_structs).is_err());
    assert!(validate(deep_structs.as_bytes()).is_err());
    let deep_arrays = format!("{}i", "a".repeat(20_000));
    assert!(Signature::from_str(&deep_arrays).is_err());
    assert!(validate(deep_arrays.as_bytes()).is_err());

    // The exact per-container limits mirror zvariant's (de)serialization limits (see
    // `zvariant::container_depths`): 32 deep is accepted, 33 rejected — for structs and arrays.
    assert!(validate(format!("{}i{}", "(".repeat(32), ")".repeat(32)).as_bytes()).is_ok());
    assert!(validate(format!("{}i{}", "(".repeat(33), ")".repeat(33)).as_bytes()).is_err());
    assert!(validate(format!("{}i", "a".repeat(32)).as_bytes()).is_ok());
    assert!(validate(format!("{}i", "a".repeat(33)).as_bytes()).is_err());

    // A long but shallow signature (many fields, little nesting) is fine — there is no length
    // limit, so a wide struct or a top-level run of many types is accepted.
    assert!(validate("i".repeat(1000).as_bytes()).is_ok());
    assert!(validate(format!("({})", "i".repeat(1000)).as_bytes()).is_ok());
}

#[test]
fn hash() {
    // We need to test if all variants of Signature hold this invariant:
    test_hash(&Signature::U16, &Signature::U16);
    test_hash(
        &Signature::array(Signature::U16),
        &Signature::static_array(&Signature::U16),
    );
    test_hash(
        &Signature::dict(Signature::U32, Signature::Str),
        &Signature::static_dict(&Signature::U32, &Signature::Str),
    );
    test_hash(
        &Signature::structure([Signature::Str, Signature::U64]),
        &Signature::static_structure(&[&Signature::Str, &Signature::U64]),
    );
}

fn test_hash(signature1: &Signature, signature2: &Signature) {
    assert_eq!(signature1, signature2);

    let mut hasher = DefaultHasher::new();
    signature1.hash(&mut hasher);
    let hash1 = hasher.finish();

    let mut hasher = DefaultHasher::new();
    signature2.hash(&mut hasher);
    let hash2 = hasher.finish();

    assert_eq!(hash1, hash2);
}
