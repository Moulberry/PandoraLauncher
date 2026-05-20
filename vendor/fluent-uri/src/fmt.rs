use super::*;
use alloc::string::String;
use core::fmt;

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self.kind {
            ParseErrorKind::InvalidOctet => "invalid percent-encoded octet at index ",
            ParseErrorKind::UnexpectedChar => "unexpected character at index ",
            ParseErrorKind::InvalidIpLiteral => "invalid IP literal at index ",
        };
        write!(f, "{}{}", msg, self.index)
    }
}

impl fmt::Debug for Uri<&str> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Uri")
            .field("scheme", &self.scheme())
            .field("authority", &self.authority())
            .field("path", &self.path())
            .field("query", &self.query())
            .field("fragment", &self.fragment())
            .finish()
    }
}

impl fmt::Display for Uri<&str> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for Uri<&mut [u8]> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Uri").finish_non_exhaustive()
    }
}

impl fmt::Debug for Uri<String> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.borrow(), f)
    }
}

impl fmt::Display for Uri<String> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Display for Scheme {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for Scheme {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for Authority<&str> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Authority")
            .field("userinfo", &self.userinfo())
            .field("host", &self.host())
            .field("port", &self.port())
            .finish()
    }
}

impl fmt::Display for Authority<&str> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for Authority<&mut [u8]> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Authority").finish_non_exhaustive()
    }
}

impl fmt::Debug for Authority<String> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Authority")
            .field("userinfo", &self.userinfo())
            .field("host", &self.host())
            .field("port", &self.port())
            .finish()
    }
}

impl fmt::Display for Authority<String> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for Host<&str> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Host")
            .field("text", &self.as_str())
            .field("data", &self.data())
            .finish()
    }
}

impl fmt::Display for Host<&str> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for Host<String> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Host")
            .field("text", &self.as_str())
            .field("data", &self.data())
            .finish()
    }
}

impl fmt::Display for Host<String> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for Host<&mut [u8]> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Host")
            .field("text", &self.as_str())
            .field("data", &self.data())
            .finish()
    }
}

impl fmt::Display for Host<&mut [u8]> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Display for Path {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for Path {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl<'a, T: ?Sized + fmt::Display + Lens> fmt::Display for View<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_ref(), f)
    }
}

impl<'a, T: ?Sized + fmt::Debug + Lens> fmt::Debug for View<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("View").field(&&**self).finish()
    }
}
