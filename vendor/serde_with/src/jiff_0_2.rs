//! De/Serialization of [jiff v0.2][jiff] types
//!
//! This modules is only available if using the `jiff_0_2` feature of the crate.
//! No extra types are exposed. Instead it enables support for [`jiff_0_2::SignedDuration`] together with [`DurationSeconds`] and its variants.
//! The types [`jiff_0_2::Timestamp`], [`jiff_0_2::Zoned`], and [`jiff_0_2::civil::DateTime`][::jiff_0_2::civil::DateTime] are supported by [`TimestampSeconds`] and its variants.
//!
//! [jiff]: https://docs.rs/jiff/0.2/

// Serialization of large numbers can result in overflows
// The time calculations are prone to this, so lint here extra
// https://github.com/jonasbb/serde_with/issues/771
#![warn(clippy::as_conversions)]

use crate::{
    formats::{Flexible, Format, Strict, Strictness},
    prelude::*,
    utils::duration::{use_duration_signed_de, use_duration_signed_ser},
};
use ::jiff_0_2::{
    civil::DateTime as CivilDateTime, tz::TimeZone, SignedDuration, Timestamp, Zoned,
};

/// Create a [`CivilDateTime`] for the Unix Epoch
fn unix_epoch_civil() -> CivilDateTime {
    ::jiff_0_2::civil::datetime(1970, 1, 1, 0, 0, 0, 0)
}

/// Convert a [`SignedDuration`] into a [`DurationSigned`]
fn duration_into_duration_signed(dur: &SignedDuration) -> DurationSigned {
    // The seconds and nanoseconds of a SignedDuration always have the same sign.
    let std_dur = Duration::new(
        dur.as_secs().unsigned_abs(),
        dur.subsec_nanos().unsigned_abs(),
    );

    DurationSigned::with_duration(
        // A duration of 0 is not positive, so check for negative value.
        if dur.is_negative() {
            Sign::Negative
        } else {
            Sign::Positive
        },
        std_dur,
    )
}

fn timestamp_to_duration(source: &Timestamp) -> DurationSigned {
    duration_into_duration_signed(&source.as_duration())
}

fn zoned_to_duration(source: &Zoned) -> DurationSigned {
    timestamp_to_duration(&source.timestamp())
}

fn civil_datetime_to_duration(source: &CivilDateTime) -> DurationSigned {
    duration_into_duration_signed(&source.duration_since(unix_epoch_civil()))
}

// No sub-unit precision
use_duration_signed_ser!(
    DurationSeconds DurationSeconds,
    DurationMilliSeconds DurationMilliSeconds,
    DurationMicroSeconds DurationMicroSeconds,
    DurationNanoSeconds DurationNanoSeconds,
    => {
        SignedDuration; duration_into_duration_signed =>
        {i64, STRICTNESS => STRICTNESS: Strictness}
        {f64, STRICTNESS => STRICTNESS: Strictness}
        #[cfg(feature = "alloc")] {String, STRICTNESS => STRICTNESS: Strictness}
    }
);

use_duration_signed_ser!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        Timestamp; timestamp_to_duration =>
        {i64, STRICTNESS => STRICTNESS: Strictness}
        {f64, STRICTNESS => STRICTNESS: Strictness}
        #[cfg(feature = "alloc")] {String, STRICTNESS => STRICTNESS: Strictness}
    }
    => {
        Zoned; zoned_to_duration =>
        {i64, STRICTNESS => STRICTNESS: Strictness}
        {f64, STRICTNESS => STRICTNESS: Strictness}
        #[cfg(feature = "alloc")] {String, STRICTNESS => STRICTNESS: Strictness}
    }
    => {
        CivilDateTime; civil_datetime_to_duration =>
        {i64, STRICTNESS => STRICTNESS: Strictness}
        {f64, STRICTNESS => STRICTNESS: Strictness}
        #[cfg(feature = "alloc")] {String, STRICTNESS => STRICTNESS: Strictness}
    }
);

// Duration/Timestamp WITH FRACTIONS
use_duration_signed_ser!(
    DurationSecondsWithFrac DurationSecondsWithFrac,
    DurationMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    DurationMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    DurationNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        SignedDuration; duration_into_duration_signed =>
        {f64, STRICTNESS => STRICTNESS: Strictness}
        #[cfg(feature = "alloc")] {String, STRICTNESS => STRICTNESS: Strictness}
    }
);

use_duration_signed_ser!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        Timestamp; timestamp_to_duration =>
        {f64, STRICTNESS => STRICTNESS: Strictness}
        #[cfg(feature = "alloc")] {String, STRICTNESS => STRICTNESS: Strictness}
    }
    => {
        Zoned; zoned_to_duration =>
        {f64, STRICTNESS => STRICTNESS: Strictness}
        #[cfg(feature = "alloc")] {String, STRICTNESS => STRICTNESS: Strictness}
    }
    => {
        CivilDateTime; civil_datetime_to_duration =>
        {f64, STRICTNESS => STRICTNESS: Strictness}
        #[cfg(feature = "alloc")] {String, STRICTNESS => STRICTNESS: Strictness}
    }
);

/// Convert a [`DurationSigned`] into a [`SignedDuration`]
fn duration_from_duration_signed<'de, D>(sdur: DurationSigned) -> Result<SignedDuration, D::Error>
where
    D: Deserializer<'de>,
{
    let mut dur: SignedDuration = match sdur.duration.try_into() {
        Ok(dur) => dur,
        Err(msg) => {
            return Err(DeError::custom(format_args!(
                "Duration is outside of the representable range: {msg}"
            )))
        }
    };
    if sdur.sign.is_negative() {
        dur = -dur;
    }
    Ok(dur)
}

fn duration_to_timestamp<'de, D>(dur: DurationSigned) -> Result<Timestamp, D::Error>
where
    D: Deserializer<'de>,
{
    Timestamp::from_duration(duration_from_duration_signed::<D>(dur)?).map_err(|msg| {
        DeError::custom(format_args!(
            "Timestamp is outside of the representable range: {msg}"
        ))
    })
}

// #[cfg(feature = "std")]
fn duration_to_zoned<'de, D>(dur: DurationSigned) -> Result<Zoned, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(duration_to_timestamp::<D>(dur)?.to_zoned(TimeZone::system()))
}

fn duration_to_civil_datetime<'de, D>(dur: DurationSigned) -> Result<CivilDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    unix_epoch_civil()
        .checked_add(duration_from_duration_signed::<D>(dur)?)
        .map_err(|msg| {
            DeError::custom(format_args!(
                "DateTime is outside of the representable range: {msg}"
            ))
        })
}

// No sub-unit precision
use_duration_signed_de!(
    DurationSeconds DurationSeconds,
    DurationMilliSeconds DurationMilliSeconds,
    DurationMicroSeconds DurationMicroSeconds,
    DurationNanoSeconds DurationNanoSeconds,
    => {
        SignedDuration; duration_from_duration_signed =>
        {FORMAT, Flexible => FORMAT: Format}
        {i64, Strict =>}
        #[cfg(feature = "alloc")] {String, Strict =>}
        #[cfg(feature = "std")] {f64, Strict =>}
    }
);

use_duration_signed_de!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        Timestamp; duration_to_timestamp =>
        {FORMAT, Flexible => FORMAT: Format}
        {i64, Strict =>}
        #[cfg(feature = "alloc")] {String, Strict =>}
        #[cfg(feature = "std")] {f64, Strict =>}
    }
    => {
        Zoned; duration_to_zoned =>
        {FORMAT, Flexible => FORMAT: Format}
        {i64, Strict =>}
        #[cfg(feature = "alloc")] {String, Strict =>}
        #[cfg(feature = "std")] {f64, Strict =>}
    }
    => {
        CivilDateTime; duration_to_civil_datetime =>
        {FORMAT, Flexible => FORMAT: Format}
        {i64, Strict =>}
        #[cfg(feature = "alloc")] {String, Strict =>}
        #[cfg(feature = "std")] {f64, Strict =>}
    }
);

// Duration/Timestamp WITH FRACTIONS
use_duration_signed_de!(
    DurationSecondsWithFrac DurationSecondsWithFrac,
    DurationMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    DurationMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    DurationNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        SignedDuration; duration_from_duration_signed =>
        {FORMAT, Flexible => FORMAT: Format}
        {f64, Strict =>}
        #[cfg(feature = "alloc")] {String, Strict =>}
    }
);

use_duration_signed_de!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        Timestamp; duration_to_timestamp =>
        {FORMAT, Flexible => FORMAT: Format}
        {f64, Strict =>}
        #[cfg(feature = "alloc")] {String, Strict =>}
    }
    => {
        Zoned; duration_to_zoned =>
        {FORMAT, Flexible => FORMAT: Format}
        {f64, Strict =>}
        #[cfg(feature = "alloc")] {String, Strict =>}
    }
    => {
        CivilDateTime; duration_to_civil_datetime =>
        {FORMAT, Flexible => FORMAT: Format}
        {f64, Strict =>}
        #[cfg(feature = "alloc")] {String, Strict =>}
    }
);
