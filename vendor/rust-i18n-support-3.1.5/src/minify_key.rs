use once_cell::sync::Lazy;
use siphasher::sip128::SipHasher13;
use std::borrow::Cow;

/// The default value of `minify_key` feature.
pub const DEFAULT_MINIFY_KEY: bool = false;

/// The length of auto-generated translation key
pub const DEFAULT_MINIFY_KEY_LEN: usize = 24;

/// The prefix of auto-generated translation key
pub const DEFAULT_MINIFY_KEY_PREFIX: &str = "";

/// The minimum length of the value to be generated the translation key
pub const DEFAULT_MINIFY_KEY_THRESH: usize = 127;

// The hasher for generate the literal translation key
static TR_KEY_HASHER: Lazy<SipHasher13> = Lazy::new(SipHasher13::new);

/// Calculate a 128-bit siphash of a value.
pub fn hash128<T: AsRef<[u8]> + ?Sized>(value: &T) -> u128 {
    TR_KEY_HASHER.hash(value.as_ref()).as_u128()
}

/// Generate a translation key from a value.
///
/// # Arguments
///
/// * `value` - The value to be generated.
/// * `len` - The length of the translation key.
/// * `prefix` - The prefix of the translation key.
/// * `threshold` - The minimum length of the value to be generated.
///
/// # Returns
///
/// * If `value.len() <= threshold` then returns the origin value.
/// * Otherwise, returns a base62 encoded 128 bits hashed translation key.
///
pub fn minify_key<'r>(value: &'r str, len: usize, prefix: &str, threshold: usize) -> Cow<'r, str> {
    if value.len() <= threshold {
        return Cow::Borrowed(value);
    }
    let encoded = base62::encode(hash128(value));
    let len = len.min(encoded.len());
    format!("{}{}", prefix, &encoded[..len]).into()
}

/// A trait for generating translation key from a value.
pub trait MinifyKey<'a> {
    /// Generate translation key from a value.
    fn minify_key(&'a self, len: usize, prefix: &str, threshold: usize) -> Cow<'a, str>;
}

impl<'a> MinifyKey<'a> for str {
    #[inline]
    fn minify_key(&'a self, len: usize, prefix: &str, threshold: usize) -> Cow<'a, str> {
        minify_key(self, len, prefix, threshold)
    }
}

impl<'a> MinifyKey<'a> for &str {
    #[inline]
    fn minify_key(&'a self, len: usize, prefix: &str, threshold: usize) -> Cow<'a, str> {
        minify_key(self, len, prefix, threshold)
    }
}

impl<'a> MinifyKey<'a> for String {
    #[inline]
    fn minify_key(&'a self, len: usize, prefix: &str, threshold: usize) -> Cow<'a, str> {
        if self.len() <= threshold {
            return Cow::Borrowed(self);
        }
        minify_key(self, len, prefix, threshold)
    }
}

impl<'a> MinifyKey<'a> for &String {
    #[inline]
    fn minify_key(&'a self, len: usize, prefix: &str, threshold: usize) -> Cow<'a, str> {
        if self.len() <= threshold {
            return Cow::from(*self);
        }
        minify_key(self, len, prefix, threshold)
    }
}

impl<'a> MinifyKey<'a> for Cow<'a, str> {
    #[inline]
    fn minify_key(&'a self, len: usize, prefix: &str, threshold: usize) -> Cow<'a, str> {
        if self.len() <= threshold {
            return Cow::Borrowed(self);
        }
        minify_key(self, len, prefix, threshold)
    }
}

impl<'a> MinifyKey<'a> for &Cow<'a, str> {
    #[inline]
    fn minify_key(&'a self, len: usize, prefix: &str, threshold: usize) -> Cow<'a, str> {
        if self.len() <= threshold {
            return Cow::Borrowed(*self);
        }
        minify_key(self, len, prefix, threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minify_key() {
        let msg = "Hello, world!";
        assert_eq!(
            minify_key(msg, 24, DEFAULT_MINIFY_KEY_PREFIX, 0),
            "1LokVzuiIrh1xByyZG4wjZ"
        );
        assert_eq!(
            msg.minify_key(24, DEFAULT_MINIFY_KEY_PREFIX, 0),
            "1LokVzuiIrh1xByyZG4wjZ"
        );
        let msg = "Hello, world!".to_string();
        assert_eq!(
            minify_key(&msg, 24, DEFAULT_MINIFY_KEY_PREFIX, 0),
            "1LokVzuiIrh1xByyZG4wjZ"
        );
        assert_eq!(
            msg.minify_key(24, DEFAULT_MINIFY_KEY_PREFIX, 0),
            "1LokVzuiIrh1xByyZG4wjZ"
        );
        assert_eq!(
            msg.minify_key(24, DEFAULT_MINIFY_KEY_PREFIX, 128),
            "Hello, world!"
        );
        let msg = &msg;
        assert_eq!(
            msg.minify_key(24, DEFAULT_MINIFY_KEY_PREFIX, 0),
            "1LokVzuiIrh1xByyZG4wjZ"
        );
        let msg = Cow::Owned("Hello, world!".to_owned());
        assert_eq!(
            minify_key(&msg, 24, DEFAULT_MINIFY_KEY_PREFIX, 0),
            "1LokVzuiIrh1xByyZG4wjZ"
        );
        assert_eq!(
            msg.minify_key(24, DEFAULT_MINIFY_KEY_PREFIX, 0),
            "1LokVzuiIrh1xByyZG4wjZ"
        );
        assert_eq!(
            msg.minify_key(24, DEFAULT_MINIFY_KEY_PREFIX, 128),
            "Hello, world!"
        );
        assert_eq!("".minify_key(24, DEFAULT_MINIFY_KEY_PREFIX, 0), "");
        assert_eq!(
            "1".minify_key(24, DEFAULT_MINIFY_KEY_PREFIX, 0),
            "knx7vOJBRfzgQvNfEkbEi"
        );
        assert_eq!("1".minify_key(24, "t_", 0), "t_knx7vOJBRfzgQvNfEkbEi");
    }
}
