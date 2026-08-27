//! Internal Helper types

// Serialization of large numbers can result in overflows
// The time calculations are prone to this, so lint here extra
// https://github.com/jonasbb/serde_with/issues/771
#![warn(clippy::as_conversions)]

use crate::{
    formats::{Flexible, Format, Strict, Strictness},
    prelude::*,
};

#[derive(Copy, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub(crate) enum Sign {
    Positive,
    Negative,
}

impl Sign {
    #[allow(dead_code)]
    pub(crate) fn is_positive(&self) -> bool {
        *self == Sign::Positive
    }

    #[allow(dead_code)]
    pub(crate) fn is_negative(&self) -> bool {
        *self == Sign::Negative
    }

    pub(crate) fn apply_f64(&self, value: f64) -> f64 {
        match *self {
            Sign::Positive => value,
            Sign::Negative => -value,
        }
    }

    pub(crate) fn apply_i64(&self, value: i64) -> Option<i64> {
        match *self {
            Sign::Positive => Some(value),
            Sign::Negative => value.checked_neg(),
        }
    }
}

#[derive(Copy, Clone)]
pub(crate) struct DurationSigned {
    pub(crate) sign: Sign,
    pub(crate) duration: Duration,
}

impl DurationSigned {
    pub(crate) fn new(sign: Sign, secs: u64, nanosecs: u32) -> Self {
        Self {
            sign,
            duration: Duration::new(secs, nanosecs),
        }
    }

    pub(crate) fn checked_mul(mut self, rhs: u32) -> Option<Self> {
        self.duration = self.duration.checked_mul(rhs)?;
        Some(self)
    }

    pub(crate) fn checked_div(mut self, rhs: u32) -> Option<Self> {
        self.duration = self.duration.checked_div(rhs)?;
        Some(self)
    }

    #[cfg(any(feature = "chrono_0_4", feature = "jiff_0_2", feature = "time_0_3"))]
    pub(crate) fn with_duration(sign: Sign, duration: Duration) -> Self {
        Self { sign, duration }
    }

    #[cfg(feature = "std")]
    pub(crate) fn to_system_time<'de, D>(self) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        match self.sign {
            Sign::Positive => SystemTime::UNIX_EPOCH.checked_add(self.duration),
            Sign::Negative => SystemTime::UNIX_EPOCH.checked_sub(self.duration),
        }
        .ok_or_else(|| DeError::custom("timestamp is outside the range for std::time::SystemTime"))
    }

    pub(crate) fn to_std_duration<'de, D>(self) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        match self.sign {
            Sign::Positive => Ok(self.duration),
            Sign::Negative => Err(DeError::custom("std::time::Duration cannot be negative")),
        }
    }
}

impl From<&Duration> for DurationSigned {
    fn from(&duration: &Duration) -> Self {
        Self {
            sign: Sign::Positive,
            duration,
        }
    }
}

#[cfg(feature = "std")]
impl From<&SystemTime> for DurationSigned {
    fn from(time: &SystemTime) -> Self {
        match time.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(dur) => DurationSigned {
                sign: Sign::Positive,
                duration: dur,
            },
            Err(err) => DurationSigned {
                sign: Sign::Negative,
                duration: err.duration(),
            },
        }
    }
}

impl<STRICTNESS> SerializeAs<DurationSigned> for DurationSeconds<u64, STRICTNESS>
where
    STRICTNESS: Strictness,
{
    fn serialize_as<S>(source: &DurationSigned, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if source.sign.is_negative() {
            return Err(SerError::custom(
                "cannot serialize a negative Duration as u64",
            ));
        }

        let mut secs = source.duration.as_secs();

        // Properly round the value
        if source.duration.subsec_millis() >= 500 {
            if source.sign.is_positive() {
                secs += 1;
            } else {
                secs -= 1;
            }
        }
        secs.serialize(serializer)
    }
}

impl<STRICTNESS> SerializeAs<DurationSigned> for DurationSeconds<i64, STRICTNESS>
where
    STRICTNESS: Strictness,
{
    fn serialize_as<S>(source: &DurationSigned, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut secs = source
            .sign
            .apply_i64(i64::try_from(source.duration.as_secs()).map_err(|_| {
                SerError::custom("The Duration of Timestamp is outside the supported range.")
            })?)
            .ok_or_else(|| {
                S::Error::custom("The Duration of Timestamp is outside the supported range.")
            })?;

        // Properly round the value
        // TODO check for overflows BUG771
        if source.duration.subsec_millis() >= 500 {
            if source.sign.is_positive() {
                secs += 1;
            } else {
                secs -= 1;
            }
        }
        secs.serialize(serializer)
    }
}

impl<STRICTNESS> SerializeAs<DurationSigned> for DurationSeconds<f64, STRICTNESS>
where
    STRICTNESS: Strictness,
{
    fn serialize_as<S>(source: &DurationSigned, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // as conversions are necessary for floats
        #[allow(clippy::as_conversions)]
        let mut secs = source.sign.apply_f64(source.duration.as_secs() as f64);

        // Properly round the value
        if source.duration.subsec_millis() >= 500 {
            if source.sign.is_positive() {
                secs += 1.;
            } else {
                secs -= 1.;
            }
        }
        secs.serialize(serializer)
    }
}

#[cfg(feature = "alloc")]
impl<STRICTNESS> SerializeAs<DurationSigned> for DurationSeconds<String, STRICTNESS>
where
    STRICTNESS: Strictness,
{
    fn serialize_as<S>(source: &DurationSigned, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut secs = source
            .sign
            .apply_i64(i64::try_from(source.duration.as_secs()).map_err(|_| {
                SerError::custom("The Duration of Timestamp is outside the supported range.")
            })?)
            .ok_or_else(|| {
                S::Error::custom("The Duration of Timestamp is outside the supported range.")
            })?;

        // Properly round the value
        if source.duration.subsec_millis() >= 500 {
            if source.sign.is_positive() {
                secs += 1;
            } else {
                secs -= 1;
            }
        }
        secs.to_string().serialize(serializer)
    }
}

impl<STRICTNESS> SerializeAs<DurationSigned> for DurationSecondsWithFrac<f64, STRICTNESS>
where
    STRICTNESS: Strictness,
{
    fn serialize_as<S>(source: &DurationSigned, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source
            .sign
            .apply_f64(source.duration.as_secs_f64())
            .serialize(serializer)
    }
}

#[cfg(feature = "alloc")]
impl<STRICTNESS> SerializeAs<DurationSigned> for DurationSecondsWithFrac<String, STRICTNESS>
where
    STRICTNESS: Strictness,
{
    fn serialize_as<S>(source: &DurationSigned, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source
            .sign
            .apply_f64(source.duration.as_secs_f64())
            .to_string()
            .serialize(serializer)
    }
}

macro_rules! duration_impls {
    ($($inner:ident { $($factor:literal => $outer:ident,)+ })+) => {
        $($(

        impl<FORMAT, STRICTNESS> SerializeAs<DurationSigned> for $outer<FORMAT, STRICTNESS>
        where
            FORMAT: Format,
            STRICTNESS: Strictness,
            $inner<FORMAT, STRICTNESS>: SerializeAs<DurationSigned>
        {
            fn serialize_as<S>(source: &DurationSigned, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let value = source.checked_mul($factor).ok_or_else(|| S::Error::custom("Failed to serialize value as the value cannot be represented."))?;
                $inner::<FORMAT, STRICTNESS>::serialize_as(&value, serializer)
            }
        }

        impl<'de, FORMAT, STRICTNESS> DeserializeAs<'de, DurationSigned> for $outer<FORMAT, STRICTNESS>
        where
            FORMAT: Format,
            STRICTNESS: Strictness,
            $inner<FORMAT, STRICTNESS>: DeserializeAs<'de, DurationSigned>,
        {
            fn deserialize_as<D>(deserializer: D) -> Result<DurationSigned, D::Error>
            where
                D: Deserializer<'de>,
            {
                let dur = $inner::<FORMAT, STRICTNESS>::deserialize_as(deserializer)?;
                let dur = dur.checked_div($factor).ok_or_else(|| D::Error::custom("Failed to deserialize value as the value cannot be represented."))?;
                Ok(dur)
            }
        }

        )+)+    };
}
duration_impls!(
    DurationSeconds {
        1000u32 => DurationMilliSeconds,
        1_000_000u32 => DurationMicroSeconds,
        1_000_000_000u32 => DurationNanoSeconds,
    }
    DurationSecondsWithFrac {
        1000u32 => DurationMilliSecondsWithFrac,
        1_000_000u32 => DurationMicroSecondsWithFrac,
        1_000_000_000u32 => DurationNanoSecondsWithFrac,
    }
);

struct DurationVisitorFlexible;
impl Visitor<'_> for DurationVisitorFlexible {
    type Value = DurationSigned;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an integer, a float, or a string containing a number")
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        let sign = if value >= 0 {
            Sign::Positive
        } else {
            Sign::Negative
        };
        Ok(DurationSigned::new(sign, value.unsigned_abs(), 0))
    }

    fn visit_u64<E>(self, secs: u64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(DurationSigned::new(Sign::Positive, secs, 0))
    }

    fn visit_f64<E>(self, secs: f64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        utils::duration_signed_from_secs_f64(secs).map_err(DeError::custom)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        match parse_float_into_time_parts(value) {
            Ok((sign, seconds, subseconds)) => Ok(DurationSigned::new(sign, seconds, subseconds)),
            Err(ParseFloatError::InvalidValue) => {
                Err(DeError::invalid_value(Unexpected::Str(value), &self))
            }
            Err(ParseFloatError::Custom(msg)) => Err(DeError::custom(msg)),
        }
    }
}

impl<'de> DeserializeAs<'de, DurationSigned> for DurationSeconds<u64, Strict> {
    fn deserialize_as<D>(deserializer: D) -> Result<DurationSigned, D::Error>
    where
        D: Deserializer<'de>,
    {
        u64::deserialize(deserializer).map(|secs: u64| DurationSigned::new(Sign::Positive, secs, 0))
    }
}

impl<'de> DeserializeAs<'de, DurationSigned> for DurationSeconds<i64, Strict> {
    fn deserialize_as<D>(deserializer: D) -> Result<DurationSigned, D::Error>
    where
        D: Deserializer<'de>,
    {
        i64::deserialize(deserializer).map(|secs: i64| {
            let sign = match secs.is_negative() {
                true => Sign::Negative,
                false => Sign::Positive,
            };
            DurationSigned::new(sign, secs.abs_diff(0), 0)
        })
    }
}

// round() only works on std
#[cfg(feature = "std")]
impl<'de> DeserializeAs<'de, DurationSigned> for DurationSeconds<f64, Strict> {
    fn deserialize_as<D>(deserializer: D) -> Result<DurationSigned, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = f64::deserialize(deserializer)?.round();
        utils::duration_signed_from_secs_f64(val).map_err(DeError::custom)
    }
}

#[cfg(feature = "alloc")]
impl<'de> DeserializeAs<'de, DurationSigned> for DurationSeconds<String, Strict> {
    fn deserialize_as<D>(deserializer: D) -> Result<DurationSigned, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DurationDeserializationVisitor;

        impl Visitor<'_> for DurationDeserializationVisitor {
            type Value = DurationSigned;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a string containing a number")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                let secs: i64 = value.parse().map_err(DeError::custom)?;
                let sign = match secs.is_negative() {
                    true => Sign::Negative,
                    false => Sign::Positive,
                };
                Ok(DurationSigned::new(sign, secs.abs_diff(0), 0))
            }
        }

        deserializer.deserialize_str(DurationDeserializationVisitor)
    }
}

impl<'de, FORMAT> DeserializeAs<'de, DurationSigned> for DurationSeconds<FORMAT, Flexible>
where
    FORMAT: Format,
{
    fn deserialize_as<D>(deserializer: D) -> Result<DurationSigned, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DurationVisitorFlexible)
    }
}

impl<'de> DeserializeAs<'de, DurationSigned> for DurationSecondsWithFrac<f64, Strict> {
    fn deserialize_as<D>(deserializer: D) -> Result<DurationSigned, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = f64::deserialize(deserializer)?;
        utils::duration_signed_from_secs_f64(val).map_err(DeError::custom)
    }
}

#[cfg(feature = "alloc")]
impl<'de> DeserializeAs<'de, DurationSigned> for DurationSecondsWithFrac<String, Strict> {
    fn deserialize_as<D>(deserializer: D) -> Result<DurationSigned, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match parse_float_into_time_parts(&value) {
            Ok((sign, seconds, subseconds)) => Ok(DurationSigned {
                sign,
                duration: Duration::new(seconds, subseconds),
            }),
            Err(ParseFloatError::InvalidValue) => Err(DeError::invalid_value(
                Unexpected::Str(&value),
                &"a string containing an integer or float",
            )),
            Err(ParseFloatError::Custom(msg)) => Err(DeError::custom(msg)),
        }
    }
}

impl<'de, FORMAT> DeserializeAs<'de, DurationSigned> for DurationSecondsWithFrac<FORMAT, Flexible>
where
    FORMAT: Format,
{
    fn deserialize_as<D>(deserializer: D) -> Result<DurationSigned, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DurationVisitorFlexible)
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
pub(crate) enum ParseFloatError {
    InvalidValue,
    #[cfg(not(feature = "alloc"))]
    Custom(&'static str),
    #[cfg(feature = "alloc")]
    Custom(String),
}

fn parse_float_into_time_parts(mut value: &str) -> Result<(Sign, u64, u32), ParseFloatError> {
    let sign = match value.chars().next() {
        // Advance by the size of the parsed char
        Some('+') => {
            value = &value[1..];
            Sign::Positive
        }
        Some('-') => {
            value = &value[1..];
            Sign::Negative
        }
        _ => Sign::Positive,
    };

    let partslen = value.split('.').count();
    let mut parts = value.split('.');
    match partslen {
        1 => {
            let seconds = parts.next().expect("Float contains exactly one part");
            if let Ok(seconds) = seconds.parse() {
                Ok((sign, seconds, 0))
            } else {
                Err(ParseFloatError::InvalidValue)
            }
        }
        2 => {
            let seconds = parts.next().expect("Float contains exactly one part");
            if let Ok(seconds) = seconds.parse() {
                let subseconds = parts.next().expect("Float contains exactly one part");
                let subseclen = u32::try_from(subseconds.chars().count()).map_err(|_| {
                    #[cfg(feature = "alloc")]
                    return ParseFloatError::Custom(alloc::format!(
                        "Duration and Timestamps with no more than 9 digits precision, but '{value}' has more"
                    ));
                    #[cfg(not(feature = "alloc"))]
                    return ParseFloatError::Custom(
                        "Duration and Timestamps with no more than 9 digits precision",
                    );
                })?;
                if subseclen > 9 {
                    #[cfg(feature = "alloc")]
                    return Err(ParseFloatError::Custom(alloc::format!(
                        "Duration and Timestamps with no more than 9 digits precision, but '{value}' has more"
                    )));
                    #[cfg(not(feature = "alloc"))]
                    return Err(ParseFloatError::Custom(
                        "Duration and Timestamps with no more than 9 digits precision",
                    ));
                }

                if let Ok(mut subseconds) = subseconds.parse() {
                    // convert subseconds to nanoseconds (10^-9), require 9 places for nanoseconds
                    subseconds *= 10u32.pow(9 - subseclen);
                    Ok((sign, seconds, subseconds))
                } else {
                    Err(ParseFloatError::InvalidValue)
                }
            } else {
                Err(ParseFloatError::InvalidValue)
            }
        }

        _ => Err(ParseFloatError::InvalidValue),
    }
}

/// The following macros are used to implement `SerializeAs` and `DeserializeAs` for the various `DurationSigned` types
///
/// ```rust,ignore
/// use_duration_signed_ser!(
///     DurationSeconds DurationSeconds,
///     DurationMilliSeconds DurationMilliSeconds,
///     DurationMicroSeconds DurationMicroSeconds,
///     DurationNanoSeconds DurationNanoSeconds,
///     => {
///         SignedDuration; duration_into_duration_signed =>
///         {i64, STRICTNESS => STRICTNESS: Strictness}
///         {f64, STRICTNESS => STRICTNESS: Strictness}
///         #[cfg(feature = "alloc")] {String, STRICTNESS => STRICTNESS: Strictness}
///     }
/// );
/// ```
///
/// They take the conversion that should be implemented publicly (`DurationSeconds`, `TimestampSeconds`, etc.) and the internal
/// conversion that is used with the `DurationSigned` (`DurationSeconds`) and then the types that should be implemented
/// for (`i64`, `f64`, `String`) and the strictness that should be used (Strict, Flexible).
#[cfg(any(feature = "chrono_0_4", feature = "jiff_0_2", feature = "time_0_3"))]
macro_rules! use_duration_signed_ser {
    (
        $main_trait:ident $internal_trait:ident
        $(
            => {
                $ty:ty; $converter:ident =>
                $($(#[$attr:meta])? {
                    $format:ty, $strictness:ty =>
                    $($tbound:ident: $bound:ident $(,)?)*
                })*
            }
        )+
    ) => {
        $($(
            $(#[$attr])?
            impl<$($tbound ,)*> SerializeAs<$ty> for $main_trait<$format, $strictness>
            where
                $($tbound: $bound,)*
            {
                fn serialize_as<S>(source: &$ty, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: Serializer,
                {
                    let dur: DurationSigned = $converter(source);
                    $internal_trait::<$format, $strictness>::serialize_as(
                        &dur,
                        serializer,
                    )
                }
            }
        )*)+
    };
    (
        $main_traitA:ident $internal_traitA:ident,
        $main_traitB:ident $internal_traitB:ident,
        $main_traitC:ident $internal_traitC:ident,
        $main_traitD:ident $internal_traitD:ident,
        $(=> $rest:tt)+
    ) => {
        use_duration_signed_ser!($main_traitA $internal_traitA $(=> $rest)+);
        use_duration_signed_ser!($main_traitB $internal_traitB $(=> $rest)+);
        use_duration_signed_ser!($main_traitC $internal_traitC $(=> $rest)+);
        use_duration_signed_ser!($main_traitD $internal_traitD $(=> $rest)+);
    };
}
// Make the macros available to the rest of the crate
#[cfg(any(feature = "chrono_0_4", feature = "jiff_0_2", feature = "time_0_3"))]
pub(crate) use use_duration_signed_ser;

/// The following macros are used to implement `SerializeAs` and `DeserializeAs` for the various `DurationSigned` types
#[cfg(any(feature = "chrono_0_4", feature = "jiff_0_2", feature = "time_0_3"))]
macro_rules! use_duration_signed_de {
    (
        $main_trait:ident $internal_trait:ident
        $(
            => {
                $ty:ty; $converter:ident =>
                $($(#[$attr:meta])? {
                    $format:ty, $strictness:ty =>
                    $($tbound:ident: $bound:ident)*
                })*
            }
        )+
    ) =>{
        $($(
            $(#[$attr])?
            impl<'de, $($tbound,)*> DeserializeAs<'de, $ty> for $main_trait<$format, $strictness>
            where
                $($tbound: $bound,)*
            {
                fn deserialize_as<D>(deserializer: D) -> Result<$ty, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    let dur: DurationSigned = $internal_trait::<$format, $strictness>::deserialize_as(deserializer)?;
                    $converter::<D>(dur)
                }
            }
        )*)+
    };
    (
        $main_traitA:ident $internal_traitA:ident,
        $main_traitB:ident $internal_traitB:ident,
        $main_traitC:ident $internal_traitC:ident,
        $main_traitD:ident $internal_traitD:ident,
        $(=> $rest:tt)+
    ) => {
        use_duration_signed_de!($main_traitA $internal_traitA $(=> $rest)+);
        use_duration_signed_de!($main_traitB $internal_traitB $(=> $rest)+);
        use_duration_signed_de!($main_traitC $internal_traitC $(=> $rest)+);
        use_duration_signed_de!($main_traitD $internal_traitD $(=> $rest)+);
    };
}
// Make the macros available to the rest of the crate
#[cfg(any(feature = "chrono_0_4", feature = "jiff_0_2", feature = "time_0_3"))]
pub(crate) use use_duration_signed_de;

#[test]
fn test_parse_float_into_time_parts() {
    // Test normal behavior
    assert_eq!(
        Ok((Sign::Positive, 123, 456_000_000)),
        parse_float_into_time_parts("+123.456")
    );
    assert_eq!(
        Ok((Sign::Negative, 123, 987_000)),
        parse_float_into_time_parts("-123.000987")
    );
    assert_eq!(
        Ok((Sign::Positive, 18446744073709551615, 123_456_789)),
        parse_float_into_time_parts("18446744073709551615.123456789")
    );

    // Test behavior around 0
    assert_eq!(
        Ok((Sign::Positive, 0, 456_000_000)),
        parse_float_into_time_parts("+0.456")
    );
    assert_eq!(
        Ok((Sign::Negative, 0, 987_000)),
        parse_float_into_time_parts("-0.000987")
    );
    assert_eq!(
        Ok((Sign::Positive, 0, 123_456_789)),
        parse_float_into_time_parts("0.123456789")
    );
}
