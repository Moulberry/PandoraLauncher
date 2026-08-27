//! Test Cases

mod utils;

use crate::utils::{
    check_deserialization, check_error_deserialization, check_serialization, is_equal,
};
use expect_test::expect;
use jiff_0_2::{civil, tz::TimeZone, SignedDuration, Timestamp, Zoned};
use serde::{Deserialize, Serialize};
use serde_with::{
    formats::Flexible, serde_as, DurationMicroSeconds, DurationMicroSecondsWithFrac,
    DurationMilliSeconds, DurationMilliSecondsWithFrac, DurationNanoSeconds,
    DurationNanoSecondsWithFrac, DurationSeconds, DurationSecondsWithFrac, TimestampMicroSeconds,
    TimestampMicroSecondsWithFrac, TimestampMilliSeconds, TimestampMilliSecondsWithFrac,
    TimestampNanoSeconds, TimestampNanoSecondsWithFrac, TimestampSeconds, TimestampSecondsWithFrac,
};

macro_rules! smoketest {
    ($($valuety:ty, $adapter:literal, $value:expr, $expect:tt;)*) => {
        $({
            #[serde_as]
            #[derive(Debug, Serialize, Deserialize, PartialEq)]
            struct S(#[serde_as(as = $adapter)] $valuety);
            #[allow(unused_braces)]
            is_equal(S($value), $expect);
        })*
    };
}

#[test]
fn test_duration_smoketest() {
    let zero = SignedDuration::ZERO;
    let one_second = SignedDuration::from_secs(1);

    smoketest! {
        SignedDuration, "DurationSeconds<i64>", one_second, {expect![[r#"1"#]]};
        SignedDuration, "DurationSeconds<f64>", one_second, {expect![[r#"1.0"#]]};
        SignedDuration, "DurationMilliSeconds<i64>", one_second, {expect![[r#"1000"#]]};
        SignedDuration, "DurationMilliSeconds<f64>", one_second, {expect![[r#"1000.0"#]]};
        SignedDuration, "DurationMicroSeconds<i64>", one_second, {expect![[r#"1000000"#]]};
        SignedDuration, "DurationMicroSeconds<f64>", one_second, {expect![[r#"1000000.0"#]]};
        SignedDuration, "DurationNanoSeconds<i64>", one_second, {expect![[r#"1000000000"#]]};
        SignedDuration, "DurationNanoSeconds<f64>", one_second, {expect![[r#"1000000000.0"#]]};
    };

    smoketest! {
        SignedDuration, "DurationSecondsWithFrac", one_second, {expect![[r#"1.0"#]]};
        SignedDuration, "DurationSecondsWithFrac<String>", one_second, {expect![[r#""1""#]]};
        SignedDuration, "DurationMilliSecondsWithFrac", one_second, {expect![[r#"1000.0"#]]};
        SignedDuration, "DurationMilliSecondsWithFrac<String>", one_second, {expect![[r#""1000""#]]};
        SignedDuration, "DurationMicroSecondsWithFrac", one_second, {expect![[r#"1000000.0"#]]};
        SignedDuration, "DurationMicroSecondsWithFrac<String>", one_second, {expect![[r#""1000000""#]]};
        SignedDuration, "DurationNanoSecondsWithFrac", one_second, {expect![[r#"1000000000.0"#]]};
        SignedDuration, "DurationNanoSecondsWithFrac<String>", one_second, {expect![[r#""1000000000""#]]};
    };

    smoketest! {
        SignedDuration, "DurationSecondsWithFrac", zero, {expect![[r#"0.0"#]]};
        SignedDuration, "DurationSecondsWithFrac", SignedDuration::new(0, 500_000_000), {expect![[r#"0.5"#]]};
        SignedDuration, "DurationSecondsWithFrac", SignedDuration::from_secs(1), {expect![[r#"1.0"#]]};
        SignedDuration, "DurationSecondsWithFrac", SignedDuration::new(0, -500_000_000), {expect![[r#"-0.5"#]]};
        SignedDuration, "DurationSecondsWithFrac", SignedDuration::from_secs(-1), {expect![[r#"-1.0"#]]};
    };
}

#[test]
fn test_timestamp_smoketest() {
    let zero = Timestamp::UNIX_EPOCH;
    let one_second = Timestamp::new(1, 0).unwrap();

    smoketest! {
        Timestamp, "TimestampSeconds<i64>", one_second, {expect![[r#"1"#]]};
        Timestamp, "TimestampSeconds<f64>", one_second, {expect![[r#"1.0"#]]};
        Timestamp, "TimestampMilliSeconds<i64>", one_second, {expect![[r#"1000"#]]};
        Timestamp, "TimestampMilliSeconds<f64>", one_second, {expect![[r#"1000.0"#]]};
        Timestamp, "TimestampMicroSeconds<i64>", one_second, {expect![[r#"1000000"#]]};
        Timestamp, "TimestampMicroSeconds<f64>", one_second, {expect![[r#"1000000.0"#]]};
        Timestamp, "TimestampNanoSeconds<i64>", one_second, {expect![[r#"1000000000"#]]};
        Timestamp, "TimestampNanoSeconds<f64>", one_second, {expect![[r#"1000000000.0"#]]};
    };

    smoketest! {
        Timestamp, "TimestampSecondsWithFrac", one_second, {expect![[r#"1.0"#]]};
        Timestamp, "TimestampSecondsWithFrac<String>", one_second, {expect![[r#""1""#]]};
        Timestamp, "TimestampMilliSecondsWithFrac", one_second, {expect![[r#"1000.0"#]]};
        Timestamp, "TimestampMilliSecondsWithFrac<String>", one_second, {expect![[r#""1000""#]]};
        Timestamp, "TimestampMicroSecondsWithFrac", one_second, {expect![[r#"1000000.0"#]]};
        Timestamp, "TimestampMicroSecondsWithFrac<String>", one_second, {expect![[r#""1000000""#]]};
        Timestamp, "TimestampNanoSecondsWithFrac", one_second, {expect![[r#"1000000000.0"#]]};
        Timestamp, "TimestampNanoSecondsWithFrac<String>", one_second, {expect![[r#""1000000000""#]]};
    };

    smoketest! {
        Timestamp, "TimestampSecondsWithFrac", zero, {expect![[r#"0.0"#]]};
        Timestamp, "TimestampSecondsWithFrac", Timestamp::new(0, 500_000_000).unwrap(), {expect![[r#"0.5"#]]};
        Timestamp, "TimestampSecondsWithFrac", Timestamp::new(1, 0).unwrap(), {expect![[r#"1.0"#]]};
        Timestamp, "TimestampSecondsWithFrac", Timestamp::new(0, -500_000_000).unwrap(), {expect![[r#"-0.5"#]]};
        Timestamp, "TimestampSecondsWithFrac", Timestamp::new(-1, 0).unwrap(), {expect![[r#"-1.0"#]]};
    };
}

#[test]
fn test_zoned_smoketest() {
    // Zoned equality compares the instant, so deserializing into the system
    // time zone round-trips on every machine.
    let zoned_utc = |second: i64, nanosecond: i32| {
        Timestamp::new(second, nanosecond)
            .unwrap()
            .to_zoned(TimeZone::UTC)
    };
    let one_second = zoned_utc(1, 0);

    smoketest! {
        Zoned, "TimestampSeconds<i64>", one_second.clone(), {expect![[r#"1"#]]};
        Zoned, "TimestampSeconds<f64>", one_second.clone(), {expect![[r#"1.0"#]]};
        Zoned, "TimestampMilliSeconds<i64>", one_second.clone(), {expect![[r#"1000"#]]};
        Zoned, "TimestampMilliSeconds<f64>", one_second.clone(), {expect![[r#"1000.0"#]]};
        Zoned, "TimestampMicroSeconds<i64>", one_second.clone(), {expect![[r#"1000000"#]]};
        Zoned, "TimestampMicroSeconds<f64>", one_second.clone(), {expect![[r#"1000000.0"#]]};
        Zoned, "TimestampNanoSeconds<i64>", one_second.clone(), {expect![[r#"1000000000"#]]};
        Zoned, "TimestampNanoSeconds<f64>", one_second.clone(), {expect![[r#"1000000000.0"#]]};
    };

    smoketest! {
        Zoned, "TimestampSecondsWithFrac", one_second.clone(), {expect![[r#"1.0"#]]};
        Zoned, "TimestampSecondsWithFrac<String>", one_second.clone(), {expect![[r#""1""#]]};
        Zoned, "TimestampMilliSecondsWithFrac", one_second.clone(), {expect![[r#"1000.0"#]]};
        Zoned, "TimestampMilliSecondsWithFrac<String>", one_second.clone(), {expect![[r#""1000""#]]};
        Zoned, "TimestampMicroSecondsWithFrac", one_second.clone(), {expect![[r#"1000000.0"#]]};
        Zoned, "TimestampMicroSecondsWithFrac<String>", one_second.clone(), {expect![[r#""1000000""#]]};
        Zoned, "TimestampNanoSecondsWithFrac", one_second.clone(), {expect![[r#"1000000000.0"#]]};
        Zoned, "TimestampNanoSecondsWithFrac<String>", one_second, {expect![[r#""1000000000""#]]};
    };

    smoketest! {
        Zoned, "TimestampSecondsWithFrac", zoned_utc(0, 0), {expect![[r#"0.0"#]]};
        Zoned, "TimestampSecondsWithFrac", zoned_utc(0, 500_000_000), {expect![[r#"0.5"#]]};
        Zoned, "TimestampSecondsWithFrac", zoned_utc(1, 0), {expect![[r#"1.0"#]]};
        Zoned, "TimestampSecondsWithFrac", zoned_utc(0, -500_000_000), {expect![[r#"-0.5"#]]};
        Zoned, "TimestampSecondsWithFrac", zoned_utc(-1, 0), {expect![[r#"-1.0"#]]};
    };
}

#[test]
fn test_civil_datetime_smoketest() {
    let zero = civil::date(1970, 1, 1).at(0, 0, 0, 0);
    let one_second = civil::date(1970, 1, 1).at(0, 0, 1, 0);

    smoketest! {
        civil::DateTime, "TimestampSeconds<i64>", one_second, {expect![[r#"1"#]]};
        civil::DateTime, "TimestampSeconds<f64>", one_second, {expect![[r#"1.0"#]]};
        civil::DateTime, "TimestampMilliSeconds<i64>", one_second, {expect![[r#"1000"#]]};
        civil::DateTime, "TimestampMilliSeconds<f64>", one_second, {expect![[r#"1000.0"#]]};
        civil::DateTime, "TimestampMicroSeconds<i64>", one_second, {expect![[r#"1000000"#]]};
        civil::DateTime, "TimestampMicroSeconds<f64>", one_second, {expect![[r#"1000000.0"#]]};
        civil::DateTime, "TimestampNanoSeconds<i64>", one_second, {expect![[r#"1000000000"#]]};
        civil::DateTime, "TimestampNanoSeconds<f64>", one_second, {expect![[r#"1000000000.0"#]]};
    };

    smoketest! {
        civil::DateTime, "TimestampSecondsWithFrac", one_second, {expect![[r#"1.0"#]]};
        civil::DateTime, "TimestampSecondsWithFrac<String>", one_second, {expect![[r#""1""#]]};
        civil::DateTime, "TimestampMilliSecondsWithFrac", one_second, {expect![[r#"1000.0"#]]};
        civil::DateTime, "TimestampMilliSecondsWithFrac<String>", one_second, {expect![[r#""1000""#]]};
        civil::DateTime, "TimestampMicroSecondsWithFrac", one_second, {expect![[r#"1000000.0"#]]};
        civil::DateTime, "TimestampMicroSecondsWithFrac<String>", one_second, {expect![[r#""1000000""#]]};
        civil::DateTime, "TimestampNanoSecondsWithFrac", one_second, {expect![[r#"1000000000.0"#]]};
        civil::DateTime, "TimestampNanoSecondsWithFrac<String>", one_second, {expect![[r#""1000000000""#]]};
    };

    smoketest! {
        civil::DateTime, "TimestampSecondsWithFrac", zero, {expect![[r#"0.0"#]]};
        civil::DateTime, "TimestampSecondsWithFrac", civil::date(1970, 1, 1).at(0, 0, 0, 500_000_000), {expect![[r#"0.5"#]]};
        civil::DateTime, "TimestampSecondsWithFrac", one_second, {expect![[r#"1.0"#]]};
        civil::DateTime, "TimestampSecondsWithFrac", civil::date(1969, 12, 31).at(23, 59, 59, 500_000_000), {expect![[r#"-0.5"#]]};
        civil::DateTime, "TimestampSecondsWithFrac", civil::date(1969, 12, 31).at(23, 59, 59, 0), {expect![[r#"-1.0"#]]};
    };
}

#[test]
fn test_duration_seconds() {
    let zero = SignedDuration::ZERO;
    let one_second = SignedDuration::from_secs(1);
    let half_second = SignedDuration::new(0, 500_000_000);
    let minus_one_second = SignedDuration::from_secs(-1);
    let minus_half_second = SignedDuration::new(0, -500_000_000);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructIntStrict(#[serde_as(as = "DurationSeconds<i64>")] SignedDuration);

    is_equal(StructIntStrict(zero), expect![[r#"0"#]]);
    is_equal(StructIntStrict(one_second), expect![[r#"1"#]]);
    is_equal(StructIntStrict(minus_one_second), expect![[r#"-1"#]]);
    check_serialization(StructIntStrict(half_second), expect![[r#"1"#]]);
    check_serialization(StructIntStrict(minus_half_second), expect![[r#"-1"#]]);
    check_error_deserialization::<StructIntStrict>(
        r#""1""#,
        expect![[r#"invalid type: string "1", expected i64 at line 1 column 3"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructIntFlexible(#[serde_as(as = "DurationSeconds<i64, Flexible>")] SignedDuration);

    is_equal(StructIntFlexible(zero), expect![[r#"0"#]]);
    is_equal(StructIntFlexible(one_second), expect![[r#"1"#]]);
    is_equal(StructIntFlexible(minus_one_second), expect![[r#"-1"#]]);
    check_deserialization(StructIntFlexible(half_second), r#""0.5""#);
    check_deserialization(StructIntFlexible(minus_half_second), r#""-0.5""#);
    check_deserialization(StructIntFlexible(one_second), r#""1""#);
    check_deserialization(StructIntFlexible(zero), r#""0""#);
    check_error_deserialization::<StructIntFlexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Structf64Strict(#[serde_as(as = "DurationSeconds<f64>")] SignedDuration);

    is_equal(Structf64Strict(zero), expect![[r#"0.0"#]]);
    is_equal(Structf64Strict(one_second), expect![[r#"1.0"#]]);
    is_equal(Structf64Strict(minus_one_second), expect![[r#"-1.0"#]]);
    check_serialization(Structf64Strict(half_second), expect![[r#"1.0"#]]);
    check_serialization(Structf64Strict(minus_half_second), expect![[r#"-1.0"#]]);
    check_error_deserialization::<Structf64Strict>(
        r#""1""#,
        expect![[r#"invalid type: string "1", expected f64 at line 1 column 3"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructStringStrict(#[serde_as(as = "DurationSeconds<String>")] SignedDuration);

    is_equal(StructStringStrict(zero), expect![[r#""0""#]]);
    is_equal(StructStringStrict(one_second), expect![[r#""1""#]]);
    is_equal(StructStringStrict(minus_one_second), expect![[r#""-1""#]]);
    check_serialization(StructStringStrict(half_second), expect![[r#""1""#]]);
    check_serialization(StructStringStrict(minus_half_second), expect![[r#""-1""#]]);
    check_error_deserialization::<StructStringStrict>(
        r#"1"#,
        expect![[
            r#"invalid type: integer `1`, expected a string containing a number at line 1 column 1"#
        ]],
    );
}

#[test]
fn test_duration_seconds_with_frac() {
    let zero = SignedDuration::ZERO;
    let one_second = SignedDuration::from_secs(1);
    let half_second = SignedDuration::new(0, 500_000_000);
    let minus_one_second = SignedDuration::from_secs(-1);
    let minus_half_second = SignedDuration::new(0, -500_000_000);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Structf64Strict(#[serde_as(as = "DurationSecondsWithFrac<f64>")] SignedDuration);

    is_equal(Structf64Strict(zero), expect![[r#"0.0"#]]);
    is_equal(Structf64Strict(one_second), expect![[r#"1.0"#]]);
    is_equal(Structf64Strict(minus_one_second), expect![[r#"-1.0"#]]);
    is_equal(Structf64Strict(half_second), expect![[r#"0.5"#]]);
    is_equal(Structf64Strict(minus_half_second), expect![[r#"-0.5"#]]);
    check_error_deserialization::<Structf64Strict>(
        r#""1""#,
        expect![[r#"invalid type: string "1", expected f64 at line 1 column 3"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Structf64Flexible(
        #[serde_as(as = "DurationSecondsWithFrac<f64, Flexible>")] SignedDuration,
    );

    is_equal(Structf64Flexible(zero), expect![[r#"0.0"#]]);
    is_equal(Structf64Flexible(minus_half_second), expect![[r#"-0.5"#]]);
    check_deserialization(Structf64Flexible(half_second), r#""0.5""#);
    check_deserialization(Structf64Flexible(minus_one_second), r#""-1""#);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructStringStrict(#[serde_as(as = "DurationSecondsWithFrac<String>")] SignedDuration);

    is_equal(StructStringStrict(zero), expect![[r#""0""#]]);
    is_equal(StructStringStrict(half_second), expect![[r#""0.5""#]]);
    is_equal(
        StructStringStrict(minus_half_second),
        expect![[r#""-0.5""#]],
    );
    is_equal(StructStringStrict(minus_one_second), expect![[r#""-1""#]]);
}

// The full error messages contain jiff error text, which differs between jiff versions.
// Only check for the version independent prefix added by serde_with.

#[test]
fn test_duration_out_of_range() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "DurationSeconds<i64, Flexible>")] SignedDuration);

    let err = serde_json::from_str::<S>(r#"18446744073709551615"#).unwrap_err();
    assert!(err
        .to_string()
        .starts_with("Duration is outside of the representable range:"));
}

#[test]
fn test_timestamp_out_of_range() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "TimestampSeconds<i64>")] Timestamp);

    let err = serde_json::from_str::<S>(r#"253402207201"#).unwrap_err();
    assert!(err
        .to_string()
        .starts_with("Timestamp is outside of the representable range:"));
}

#[test]
fn test_civil_datetime_out_of_range() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "TimestampSeconds<i64>")] civil::DateTime);

    let err = serde_json::from_str::<S>(r#"300000000000"#).unwrap_err();
    assert!(err
        .to_string()
        .starts_with("DateTime is outside of the representable range:"));
}
