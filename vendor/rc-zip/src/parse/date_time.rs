use chrono::{
    offset::{LocalResult, TimeZone, Utc},
    DateTime, Timelike,
};
use ownable::{IntoOwned, ToOwned};
use std::fmt;
use winnow::{
    binary::{le_u16, le_u64},
    seq, PResult, Parser, Partial,
};

/// A timestamp in MS-DOS format
///
/// Represents dates from year 1980 to 2180, with 2 second precision.
#[derive(Clone, Copy, Eq, PartialEq, IntoOwned, ToOwned)]
pub struct MsdosTimestamp {
    /// Time in 2-second intervals
    pub time: u16,

    /// Date in MS-DOS format, cf. <https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-dosdatetimetofiletime>
    pub date: u16,
}

impl fmt::Debug for MsdosTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.to_datetime() {
            Some(dt) => write!(f, "MsdosTimestamp({})", dt),
            None => write!(f, "MsdosTimestamp(?)"),
        }
    }
}

impl MsdosTimestamp {
    /// Parser for MS-DOS timestamps
    pub fn parser(i: &mut Partial<&'_ [u8]>) -> PResult<Self> {
        seq! {Self {
            time: le_u16,
            date: le_u16,
        }}
        .parse_next(i)
    }

    /// Attempts to convert to a chrono UTC date time
    pub fn to_datetime(&self) -> Option<DateTime<Utc>> {
        // see https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-dosdatetimetofiletime
        let res = {
            // bits 0-4: day of the month (1-31)
            let d = (self.date & 0b1_1111) as u32;
            // bits 5-8: month (1 = january, 2 = february and so on)
            let m = ((self.date >> 5) & 0b1111) as u32;
            // bits 9-15: year offset from 1980
            let y = ((self.date >> 9) + 1980) as i32;
            Utc.with_ymd_and_hms(y, m, d, 0, 0, 0)
        };

        let date = match res {
            LocalResult::Single(date) => date,
            _ => return None,
        };

        // bits 0-4: second divided by 2
        let s = (self.time & 0b1_1111) as u32 * 2;
        // bits 5-10: minute (0-59)
        let m = (self.time >> 5 & 0b11_1111) as u32;
        // bits 11-15: hour (0-23 on a 24-hour clock)
        let h = (self.time >> 11) as u32;
        date.with_hour(h)?.with_minute(m)?.with_second(s)
    }
}

/// A timestamp in NTFS format.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct NtfsTimestamp {
    /// Timestamp in 100ns intervals since 1601-01-01 00:00:00 UTC
    pub timestamp: u64,
}

impl fmt::Debug for NtfsTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.to_datetime() {
            Some(dt) => write!(f, "NtfsTimestamp({})", dt),
            None => write!(f, "NtfsTimestamp(?)"),
        }
    }
}

impl NtfsTimestamp {
    /// Parse an MS-DOS timestamp from a byte slice
    pub fn parser(i: &mut Partial<&'_ [u8]>) -> PResult<Self> {
        le_u64.map(|timestamp| Self { timestamp }).parse_next(i)
    }

    /// Attempts to convert to a chrono UTC date time
    pub fn to_datetime(&self) -> Option<DateTime<Utc>> {
        // windows timestamp resolution
        let ticks_per_second = 10_000_000;
        let secs = (self.timestamp / ticks_per_second) as i64;
        let nsecs = ((self.timestamp % ticks_per_second) * 100) as u32;
        let epoch = Utc.with_ymd_and_hms(1601, 1, 1, 0, 0, 0).single()?;
        match Utc.timestamp_opt(epoch.timestamp() + secs, nsecs) {
            LocalResult::Single(date) => Some(date),
            _ => None,
        }
    }
}

pub(crate) fn zero_datetime() -> chrono::DateTime<chrono::offset::Utc> {
    chrono::DateTime::<Utc>::from_timestamp_millis(0).unwrap()
}
