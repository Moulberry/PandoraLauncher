use crate::error;
use crate::UntaggedEnumVisitor;
use serde::de::{Unexpected, Visitor};

pub(crate) enum IntKind {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
}

pub(crate) trait Integer:
    Copy
    + TryInto<i8>
    + TryInto<i16>
    + TryInto<i32>
    + TryInto<i64>
    + TryInto<i128>
    + TryInto<u8>
    + TryInto<u16>
    + TryInto<u32>
    + TryInto<u64>
    + TryInto<u128>
{
}

impl<T> Integer for T
where
    T: Copy,
    i8: TryFrom<T>,
    i16: TryFrom<T>,
    i32: TryFrom<T>,
    i64: TryFrom<T>,
    i128: TryFrom<T>,
    u8: TryFrom<T>,
    u16: TryFrom<T>,
    u32: TryFrom<T>,
    u64: TryFrom<T>,
    u128: TryFrom<T>,
{
}

impl<'closure, 'de, Value> UntaggedEnumVisitor<'closure, 'de, Value> {
    pub(crate) fn dispatch_integer<I, E>(
        self,
        value: I,
        precedence: [IntKind; 10],
    ) -> Result<Value, E>
    where
        I: Integer,
        E: serde::de::Error,
    {
        for kind in precedence {
            match kind {
                IntKind::I8 => {
                    if let Some(int) = i8::int_from(value) {
                        if let Some(visit_i8) = self.visit_i8 {
                            return visit_i8(int).map_err(error::unerase);
                        }
                    }
                }
                IntKind::I16 => {
                    if let Some(int) = i16::int_from(value) {
                        if let Some(visit_i16) = self.visit_i16 {
                            return visit_i16(int).map_err(error::unerase);
                        }
                    }
                }
                IntKind::I32 => {
                    if let Some(int) = i32::int_from(value) {
                        if let Some(visit_i32) = self.visit_i32 {
                            return visit_i32(int).map_err(error::unerase);
                        }
                    }
                }
                IntKind::I64 => {
                    if let Some(int) = i64::int_from(value) {
                        if let Some(visit_i64) = self.visit_i64 {
                            return visit_i64(int).map_err(error::unerase);
                        }
                    }
                }
                IntKind::I128 => {
                    if let Some(int) = i128::int_from(value) {
                        if let Some(visit_i128) = self.visit_i128 {
                            return visit_i128(int).map_err(error::unerase);
                        }
                    }
                }
                IntKind::U8 => {
                    if let Some(int) = u8::int_from(value) {
                        if let Some(visit_u8) = self.visit_u8 {
                            return visit_u8(int).map_err(error::unerase);
                        }
                    }
                }
                IntKind::U16 => {
                    if let Some(int) = u16::int_from(value) {
                        if let Some(visit_u16) = self.visit_u16 {
                            return visit_u16(int).map_err(error::unerase);
                        }
                    }
                }
                IntKind::U32 => {
                    if let Some(int) = u32::int_from(value) {
                        if let Some(visit_u32) = self.visit_u32 {
                            return visit_u32(int).map_err(error::unerase);
                        }
                    }
                }
                IntKind::U64 => {
                    if let Some(int) = u64::int_from(value) {
                        if let Some(visit_u64) = self.visit_u64 {
                            return visit_u64(int).map_err(error::unerase);
                        }
                    }
                }
                IntKind::U128 => {
                    if let Some(int) = u128::int_from(value) {
                        if let Some(visit_u128) = self.visit_u128 {
                            return visit_u128(int).map_err(error::unerase);
                        }
                    }
                }
            }
        }
        if let Some(int) = u64::int_from(value) {
            return Err(E::invalid_type(Unexpected::Unsigned(int), &self));
        }
        if let Some(int) = i64::int_from(value) {
            return Err(E::invalid_type(Unexpected::Signed(int), &self));
        }
        if let Some(int) = u128::int_from(value) {
            return crate::DefaultVisitor::new(&self).visit_u128(int);
        }
        if let Some(int) = i128::int_from(value) {
            return crate::DefaultVisitor::new(&self).visit_i128(int);
        }
        unreachable!()
    }
}

trait IntFrom<I>: Sized {
    fn int_from(int: I) -> Option<Self>;
}

impl<T, I> IntFrom<I> for T
where
    I: TryInto<Self>,
{
    fn int_from(int: I) -> Option<Self> {
        int.try_into().ok()
    }
}
