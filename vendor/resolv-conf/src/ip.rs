use std::error::Error;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

/// A network, that is an IP address and a mask
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Network {
    /// Represent an IPv4 network address
    V4(Ipv4Addr, Ipv4Addr),
    /// Represent an IPv6 network address
    V6(Ipv6Addr, Ipv6Addr),
}

impl FromStr for Network {
    type Err = AddrParseError;

    fn from_str(val: &str) -> Result<Self, Self::Err> {
        let (ip, mask) = match val.split_once('/') {
            Some((ip, mask)) => (ip, Some(mask)),
            None => (val, None),
        };

        match IpAddr::from_str(ip)? {
            IpAddr::V4(ip) => {
                if ip.is_unspecified() {
                    return Err(AddrParseError);
                }

                let mask = match mask {
                    Some(mask) => {
                        let mask = Ipv4Addr::from_str(mask)?;
                        // make sure this is a valid mask
                        let value = ip.octets().iter().fold(0, |acc, &x| acc + u32::from(x));
                        match value == 0 || (value & !value != 0) {
                            true => return Err(AddrParseError),
                            false => mask,
                        }
                    }
                    // We have to "guess" the mask.
                    //
                    // FIXME(@little-dude) right now, we look at the number or bytes that are 0,
                    // but maybe we should use the number of bits that are 0.
                    //
                    // In other words, with this implementation, the mask of `128.192.0.0` will be
                    // `255.255.0.0` (a.k.a `/16`). But we could also consider that the mask is
                    // `/10` (a.k.a `255.63.0.0`).
                    //
                    // My only source on topic is the "DNS and Bind" book which suggests using
                    // bytes, not bits.
                    None => {
                        let octets = ip.octets();
                        if octets[3] == 0 {
                            if octets[2] == 0 {
                                if octets[1] == 0 {
                                    Ipv4Addr::new(255, 0, 0, 0)
                                } else {
                                    Ipv4Addr::new(255, 255, 0, 0)
                                }
                            } else {
                                Ipv4Addr::new(255, 255, 255, 0)
                            }
                        } else {
                            Ipv4Addr::new(255, 255, 255, 255)
                        }
                    }
                };

                Ok(Self::V4(ip, mask))
            }
            IpAddr::V6(ip) => {
                let mask = match mask {
                    // FIXME: validate the mask
                    Some(mask) => Ipv6Addr::from_str(mask)?,
                    // FIXME: "guess" an appropriate mask for the IP
                    None => Ipv6Addr::new(
                        65_535, 65_535, 65_535, 65_535, 65_535, 65_535, 65_535, 65_535,
                    ),
                };

                Ok(Self::V6(ip, mask))
            }
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::V4(address, mask) => write!(fmt, "{address}/{mask}"),
            Self::V6(address, mask) => write!(fmt, "{address}/{mask}"),
        }
    }
}

/// Represent an IP address. This type is similar to `std::net::IpAddr` but it supports IPv6 scope
/// identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopedIp {
    /// Represent an IPv4 address
    V4(Ipv4Addr),
    /// Represent an IPv6 and its scope identifier, if any
    V6(Ipv6Addr, Option<String>),
}

impl From<ScopedIp> for IpAddr {
    fn from(val: ScopedIp) -> Self {
        match val {
            ScopedIp::V4(ip) => Self::from(ip),
            ScopedIp::V6(ip, _) => Self::from(ip),
        }
    }
}

impl From<&ScopedIp> for IpAddr {
    fn from(val: &ScopedIp) -> Self {
        match val {
            ScopedIp::V4(ip) => Self::from(*ip),
            ScopedIp::V6(ip, _) => Self::from(*ip),
        }
    }
}

impl From<Ipv6Addr> for ScopedIp {
    fn from(value: Ipv6Addr) -> Self {
        Self::V6(value, None)
    }
}

impl From<Ipv4Addr> for ScopedIp {
    fn from(value: Ipv4Addr) -> Self {
        Self::V4(value)
    }
}

impl From<IpAddr> for ScopedIp {
    fn from(value: IpAddr) -> Self {
        match value {
            IpAddr::V4(ip) => Self::from(ip),
            IpAddr::V6(ip) => Self::from(ip),
        }
    }
}

impl FromStr for ScopedIp {
    type Err = AddrParseError;
    /// Parse a string representing an IP address.
    fn from_str(s: &str) -> Result<Self, AddrParseError> {
        let mut parts = s.split('%');
        let addr = parts.next().unwrap();
        match IpAddr::from_str(addr) {
            Ok(IpAddr::V4(ip)) => {
                if parts.next().is_some() {
                    // It's not a valid IPv4 address if it contains a '%'
                    Err(AddrParseError)
                } else {
                    Ok(Self::from(ip))
                }
            }
            Ok(IpAddr::V6(ip)) => {
                if let Some(scope_id) = parts.next() {
                    if scope_id.is_empty() {
                        return Err(AddrParseError);
                    }
                    for c in scope_id.chars() {
                        if !c.is_alphanumeric() {
                            return Err(AddrParseError);
                        }
                    }
                    Ok(Self::V6(ip, Some(scope_id.to_string())))
                } else {
                    Ok(Self::V6(ip, None))
                }
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl fmt::Display for ScopedIp {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::V4(address) => address.fmt(fmt),
            Self::V6(address, None) => address.fmt(fmt),
            Self::V6(address, Some(scope)) => write!(fmt, "{address}%{scope}"),
        }
    }
}

/// An error which can be returned when parsing an IP address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddrParseError;

impl fmt::Display for AddrParseError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("invalid IP address syntax")
    }
}

impl Error for AddrParseError {}

impl From<::std::net::AddrParseError> for AddrParseError {
    fn from(_: ::std::net::AddrParseError) -> Self {
        Self
    }
}
