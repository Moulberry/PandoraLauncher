//! Percent-encoders for URI components.

use crate::enc::table::{self, Table};

/// A trait used by [`EString`] to specify the table used for encoding.
///
/// [`EString`]: super::EString
pub trait Encoder: Send + Sync + 'static {
    /// The table used for encoding.
    const TABLE: &'static Table;
}

/// An encoder for the path component.
#[derive(Clone, Copy, Debug)]
pub struct PathEncoder(());

/// An encoder for the query or the fragment component.
#[derive(Clone, Copy, Debug)]
pub struct QueryFragmentEncoder(());

impl Encoder for PathEncoder {
    const TABLE: &'static Table = table::PATH;
}

impl Encoder for QueryFragmentEncoder {
    const TABLE: &'static Table = table::QUERY_FRAGMENT;
}
