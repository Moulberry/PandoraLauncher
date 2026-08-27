mod de;
pub(crate) use de::*;
mod ser;
pub use ser::*;

use crate::{Error, Result, Signature};

/// Reject a signature that carries a GVariant maybe type before it reaches the D-Bus codec.
///
/// The maybe type has no D-Bus wire representation, so a signature containing one (whether given
/// statically or read from a variant on the wire) is invalid input rather than an internal
/// invariant. It becomes reachable when `zvariant_utils/gvariant` is enabled — including
/// transitively via a co-located `zgvariant` — so this must return an error, not panic.
pub(crate) fn reject_maybe(signature: &Signature) -> Result<()> {
    if signature.contains_maybe() {
        return Err(maybe_not_in_dbus());
    }

    Ok(())
}

/// Reject a maybe type in a signature carried as a `g` or `v` value's string form.
///
/// This is the hot path (every variant on the wire), so it avoids re-parsing the string into a
/// `Signature`: `m` is the maybe type constructor and appears nowhere else in the signature
/// grammar, so its presence is an exact test for a maybe type.
pub(crate) fn reject_maybe_in_signature_str(bytes: &[u8]) -> Result<()> {
    if bytes.contains(&b'm') {
        return Err(maybe_not_in_dbus());
    }

    Ok(())
}

fn maybe_not_in_dbus() -> Error {
    Error::Message(
        "GVariant `Maybe` types are not valid in the D-Bus format; use the `zgvariant` crate \
         for GVariant serialization"
            .to_owned(),
    )
}
