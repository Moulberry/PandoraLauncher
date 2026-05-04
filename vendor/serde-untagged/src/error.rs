use alloc::borrow::ToOwned;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::{self, Debug, Display};
use serde::de::Expected;

pub struct Error {
    imp: ErrorImpl,
}

pub(crate) fn erase<E: serde::de::Error>(err: E) -> Error {
    serde::de::Error::custom(err)
}

pub(crate) fn unerase<E: serde::de::Error>(err: Error) -> E {
    err.as_serde()
}

impl serde::de::StdError for Error {}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let error = self.as_serde::<serde::de::value::Error>();
        Display::fmt(&error, formatter)
    }
}

impl Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let error = self.as_serde::<serde::de::value::Error>();
        Debug::fmt(&error, formatter)
    }
}

enum ErrorImpl {
    Custom(String),
    InvalidType {
        unexpected: Unexpected,
        expected: String,
    },
    InvalidValue {
        unexpected: Unexpected,
        expected: String,
    },
    InvalidLength {
        len: usize,
        expected: String,
    },
    UnknownVariant {
        variant: String,
        expected: &'static [&'static str],
    },
    UnknownField {
        field: String,
        expected: &'static [&'static str],
    },
    MissingField {
        field: &'static str,
    },
    DuplicateField {
        field: &'static str,
    },
}

enum Unexpected {
    Bool(bool),
    Unsigned(u64),
    Signed(i64),
    Float(f64),
    Char(char),
    Str(String),
    Bytes(Vec<u8>),
    Unit,
    Option,
    NewtypeStruct,
    Seq,
    Map,
    Enum,
    UnitVariant,
    NewtypeVariant,
    TupleVariant,
    StructVariant,
    Other(String),
}

impl serde::de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        let imp = ErrorImpl::Custom(msg.to_string());
        Error { imp }
    }

    fn invalid_type(unexpected: serde::de::Unexpected, expected: &dyn Expected) -> Self {
        let imp = ErrorImpl::InvalidType {
            unexpected: Unexpected::from_serde(unexpected),
            expected: expected.to_string(),
        };
        Error { imp }
    }

    fn invalid_value(unexpected: serde::de::Unexpected, expected: &dyn Expected) -> Self {
        let imp = ErrorImpl::InvalidValue {
            unexpected: Unexpected::from_serde(unexpected),
            expected: expected.to_string(),
        };
        Error { imp }
    }

    fn invalid_length(len: usize, expected: &dyn Expected) -> Self {
        let imp = ErrorImpl::InvalidLength {
            len,
            expected: expected.to_string(),
        };
        Error { imp }
    }

    fn unknown_variant(variant: &str, expected: &'static [&'static str]) -> Self {
        let imp = ErrorImpl::UnknownVariant {
            variant: variant.to_owned(),
            expected,
        };
        Error { imp }
    }

    fn unknown_field(field: &str, expected: &'static [&'static str]) -> Self {
        let imp = ErrorImpl::UnknownField {
            field: field.to_owned(),
            expected,
        };
        Error { imp }
    }

    fn missing_field(field: &'static str) -> Self {
        let imp = ErrorImpl::MissingField { field };
        Error { imp }
    }

    fn duplicate_field(field: &'static str) -> Self {
        let imp = ErrorImpl::DuplicateField { field };
        Error { imp }
    }
}

impl Error {
    fn as_serde<E: serde::de::Error>(&self) -> E {
        match &self.imp {
            ErrorImpl::Custom(msg) => E::custom(msg),
            ErrorImpl::InvalidType {
                unexpected,
                expected,
            } => E::invalid_type(unexpected.as_serde(), &expected.as_str()),
            ErrorImpl::InvalidValue {
                unexpected,
                expected,
            } => E::invalid_value(unexpected.as_serde(), &expected.as_str()),
            ErrorImpl::InvalidLength { len, expected } => {
                E::invalid_length(*len, &expected.as_str())
            }
            ErrorImpl::UnknownVariant { variant, expected } => {
                E::unknown_variant(variant, expected)
            }
            ErrorImpl::UnknownField { field, expected } => E::unknown_field(field, expected),
            ErrorImpl::MissingField { field } => E::missing_field(field),
            ErrorImpl::DuplicateField { field } => E::duplicate_field(field),
        }
    }
}

impl Unexpected {
    fn from_serde(unexpected: serde::de::Unexpected) -> Self {
        match unexpected {
            serde::de::Unexpected::Bool(value) => Unexpected::Bool(value),
            serde::de::Unexpected::Unsigned(value) => Unexpected::Unsigned(value),
            serde::de::Unexpected::Signed(value) => Unexpected::Signed(value),
            serde::de::Unexpected::Float(value) => Unexpected::Float(value),
            serde::de::Unexpected::Char(value) => Unexpected::Char(value),
            serde::de::Unexpected::Str(value) => Unexpected::Str(value.to_owned()),
            serde::de::Unexpected::Bytes(value) => Unexpected::Bytes(value.to_owned()),
            serde::de::Unexpected::Unit => Unexpected::Unit,
            serde::de::Unexpected::Option => Unexpected::Option,
            serde::de::Unexpected::NewtypeStruct => Unexpected::NewtypeStruct,
            serde::de::Unexpected::Seq => Unexpected::Seq,
            serde::de::Unexpected::Map => Unexpected::Map,
            serde::de::Unexpected::Enum => Unexpected::Enum,
            serde::de::Unexpected::UnitVariant => Unexpected::UnitVariant,
            serde::de::Unexpected::NewtypeVariant => Unexpected::NewtypeVariant,
            serde::de::Unexpected::TupleVariant => Unexpected::TupleVariant,
            serde::de::Unexpected::StructVariant => Unexpected::StructVariant,
            serde::de::Unexpected::Other(msg) => Unexpected::Other(msg.to_owned()),
        }
    }

    fn as_serde(&self) -> serde::de::Unexpected {
        match self {
            Unexpected::Bool(value) => serde::de::Unexpected::Bool(*value),
            Unexpected::Unsigned(value) => serde::de::Unexpected::Unsigned(*value),
            Unexpected::Signed(value) => serde::de::Unexpected::Signed(*value),
            Unexpected::Float(value) => serde::de::Unexpected::Float(*value),
            Unexpected::Char(value) => serde::de::Unexpected::Char(*value),
            Unexpected::Str(value) => serde::de::Unexpected::Str(value),
            Unexpected::Bytes(value) => serde::de::Unexpected::Bytes(value),
            Unexpected::Unit => serde::de::Unexpected::Unit,
            Unexpected::Option => serde::de::Unexpected::Option,
            Unexpected::NewtypeStruct => serde::de::Unexpected::NewtypeStruct,
            Unexpected::Seq => serde::de::Unexpected::Seq,
            Unexpected::Map => serde::de::Unexpected::Map,
            Unexpected::Enum => serde::de::Unexpected::Enum,
            Unexpected::UnitVariant => serde::de::Unexpected::UnitVariant,
            Unexpected::NewtypeVariant => serde::de::Unexpected::NewtypeVariant,
            Unexpected::TupleVariant => serde::de::Unexpected::TupleVariant,
            Unexpected::StructVariant => serde::de::Unexpected::StructVariant,
            Unexpected::Other(msg) => serde::de::Unexpected::Other(msg),
        }
    }
}
