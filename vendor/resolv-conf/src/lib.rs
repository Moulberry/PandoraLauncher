//! The crate simply parses `/etc/resolv.conf` file and creates a config object
//!
//! # Examples
//!
//! ## Parsing a config from a string
//! ```rust
//! extern crate resolv_conf;
//!
//! use std::net::{Ipv4Addr, Ipv6Addr};
//! use resolv_conf::{ScopedIp, Config, Network};
//!
//! fn main() {
//!     let config_str = "
//! options ndots:8 timeout:8 attempts:8
//!
//! domain example.com
//! search example.com sub.example.com
//!
//! nameserver 2001:4860:4860::8888
//! nameserver 2001:4860:4860::8844
//! nameserver 8.8.8.8
//! nameserver 8.8.4.4
//!
//! options rotate
//! options inet6 no-tld-query
//!
//! sortlist 130.155.160.0/255.255.240.0 130.155.0.0";
//!
//!     // Parse the config.
//!     let parsed_config = Config::parse(&config_str).expect("Failed to parse config");
//!
//!     // We can build configs manually as well, either directly or with Config::new()
//!     let mut expected_config = Config::new();
//!     expected_config.nameservers = vec![
//!         ScopedIp::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888), None),
//!         ScopedIp::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8844), None),
//!         ScopedIp::V4(Ipv4Addr::new(8, 8, 8, 8)),
//!         ScopedIp::V4(Ipv4Addr::new(8, 8, 4, 4)),
//!     ];
//!     expected_config.sortlist = vec![
//!         Network::V4(Ipv4Addr::new(130, 155, 160, 0), Ipv4Addr::new(255, 255, 240, 0)),
//!         Network::V4(Ipv4Addr::new(130, 155, 0, 0), Ipv4Addr::new(255, 255, 0, 0)),
//!     ];
//!     expected_config.debug = false;
//!     expected_config.ndots = 8;
//!     expected_config.timeout = 8;
//!     expected_config.attempts = 8;
//!     expected_config.rotate = true;
//!     expected_config.no_check_names = false;
//!     expected_config.inet6 = true;
//!     expected_config.ip6_bytestring = false;
//!     expected_config.ip6_dotint = false;
//!     expected_config.edns0 = false;
//!     expected_config.single_request = false;
//!     expected_config.single_request_reopen = false;
//!     expected_config.no_tld_query = true;
//!     expected_config.use_vc = false;
//!     expected_config.set_domain(String::from("example.com"));
//!     expected_config.set_search(vec![
//!         String::from("example.com"),
//!         String::from("sub.example.com")
//!     ]);
//!
//!     // We can compare configurations, since resolv_conf::Config implements Eq
//!     assert_eq!(parsed_config, expected_config);
//! }
//! ```
//!
//! ## Parsing a file
//!
//! ```rust
//! use std::io::Read;
//! use std::fs::File;
//!
//! extern crate resolv_conf;
//!
//! fn main() {
//!     // Read the file
//!     let mut buf = Vec::with_capacity(4096);
//!     let mut f = File::open("/etc/resolv.conf").unwrap();
//!     f.read_to_end(&mut buf).unwrap();
//!
//!     // Parse the buffer
//!     let cfg = resolv_conf::Config::parse(&buf).unwrap();
//!
//!     // Print the config
//!     println!("---- Parsed /etc/resolv.conf -----\n{:#?}\n", cfg);
//! }
//! ```

#![warn(missing_debug_implementations, missing_docs, unreachable_pub)]
#![warn(clippy::use_self)]

use std::fmt;
use std::iter::Iterator;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::slice::Iter;
use std::str::{self, from_utf8, FromStr, Utf8Error};

mod ip;
pub use ip::{AddrParseError, Network, ScopedIp};

/// Represent a resolver configuration, as described in `man 5 resolv.conf`.
/// The options and defaults match those in the linux `man` page.
///
/// Note: while most fields in the structure are public the `search` and
/// `domain` fields must be accessed via methods. This is because there are
/// few different ways to treat `domain` field. In GNU libc `search` and
/// `domain` replace each other ([`get_last_search_or_domain`]).
/// In MacOS `/etc/resolve/*` files `domain` is treated in entirely different
/// way.
///
/// Also consider using [`glibc_normalize`] and [`get_system_domain`] to match
/// behavior of GNU libc.
///
/// ```rust
/// extern crate resolv_conf;
///
/// use std::net::Ipv4Addr;
/// use resolv_conf::{Config, ScopedIp};
///
/// fn main() {
///     // Create a new config
///     let mut config = Config::new();
///     config.nameservers.push(ScopedIp::V4(Ipv4Addr::new(8, 8, 8, 8)));
///     config.set_search(vec!["example.com".into()]);
///
///     // Parse a config
///     let parsed = Config::parse("nameserver 8.8.8.8\nsearch example.com").unwrap();
///     assert_eq!(parsed, config);
/// }
/// ```
///
/// [`glibc_normalize`]: #method.glibc_normalize
/// [`get_last_search_or_domain`]: #method.get_last_search_or_domain
/// [`get_system_domain`]: #method.get_system_domain
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// List of nameservers
    pub nameservers: Vec<ScopedIp>,
    /// Indicated whether the last line that has been parsed is a "domain" directive or a "search"
    /// directive. This is important for compatibility with glibc, since in glibc's implementation,
    /// "search" and "domain" are mutually exclusive, and only the last directive is taken into
    /// consideration.
    last_search: LastSearch,
    /// Domain to append to name when it doesn't contain ndots
    domain: Option<String>,
    /// List of suffixes to append to name when it doesn't contain ndots
    search: Option<Vec<String>>,
    /// List of preferred addresses
    pub sortlist: Vec<Network>,
    /// Enable DNS resolve debugging
    pub debug: bool,
    /// Number of dots in name to try absolute resolving first (default 1)
    pub ndots: u32,
    /// Dns query timeout (default 5 [sec])
    pub timeout: u32,
    /// Number of attempts to resolve name if server is inaccesible (default 2)
    pub attempts: u32,
    /// Round-robin selection of servers (default false)
    pub rotate: bool,
    /// Don't check names for validity (default false)
    pub no_check_names: bool,
    /// Try AAAA query before A
    pub inet6: bool,
    /// Use reverse lookup of ipv6 using bit-label format described instead
    /// of nibble format
    pub ip6_bytestring: bool,
    /// Do ipv6 reverse lookups in ip6.int zone instead of ip6.arpa
    /// (default false)
    pub ip6_dotint: bool,
    /// Enable dns extensions described in RFC 2671
    pub edns0: bool,
    /// Don't make ipv4 and ipv6 requests simultaneously
    pub single_request: bool,
    /// Use same socket for the A and AAAA requests
    pub single_request_reopen: bool,
    /// Don't resolve unqualified name as top level domain
    pub no_tld_query: bool,
    /// Force using TCP for DNS resolution
    pub use_vc: bool,
    /// Disable the automatic reloading of a changed configuration file
    pub no_reload: bool,
    /// Optionally send the AD (authenticated data) bit in queries
    pub trust_ad: bool,
    /// The order in which databases should be searched during a lookup
    /// **(openbsd-only)**
    pub lookup: Vec<Lookup>,
    /// The order in which internet protocol families should be prefered
    /// **(openbsd-only)**
    pub family: Vec<Family>,
    /// Suppress AAAA queries made by the stub resolver
    pub no_aaaa: bool,
}

impl Config {
    /// Create a new `Config` object with default values.
    ///
    /// ```rust
    /// # extern crate resolv_conf;
    /// use resolv_conf::Config;
    /// # fn main() {
    /// let config = Config::new();
    /// assert_eq!(config.nameservers, vec![]);
    /// assert!(config.get_domain().is_none());
    /// assert!(config.get_search().is_none());
    /// assert_eq!(config.sortlist, vec![]);
    /// assert_eq!(config.debug, false);
    /// assert_eq!(config.ndots, 1);
    /// assert_eq!(config.timeout, 5);
    /// assert_eq!(config.attempts, 2);
    /// assert_eq!(config.rotate, false);
    /// assert_eq!(config.no_check_names, false);
    /// assert_eq!(config.inet6, false);
    /// assert_eq!(config.ip6_bytestring, false);
    /// assert_eq!(config.ip6_dotint, false);
    /// assert_eq!(config.edns0, false);
    /// assert_eq!(config.single_request, false);
    /// assert_eq!(config.single_request_reopen, false);
    /// assert_eq!(config.no_tld_query, false);
    /// assert_eq!(config.use_vc, false);
    /// # }
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a buffer and return the corresponding `Config` object.
    ///
    /// ```rust
    /// # extern crate resolv_conf;
    /// use resolv_conf::{ScopedIp, Config};
    /// # fn main() {
    /// let config_str = "# /etc/resolv.conf
    /// nameserver  8.8.8.8
    /// nameserver  8.8.4.4
    /// search      example.com sub.example.com
    /// options     ndots:8 attempts:8";
    ///
    /// // Parse the config
    /// let parsed_config = Config::parse(&config_str).expect("Failed to parse config");
    ///
    /// // Print the config
    /// println!("{:?}", parsed_config);
    /// # }
    /// ```
    pub fn parse<T: AsRef<[u8]>>(buf: T) -> Result<Self, ParseError> {
        let (new, mut errors) = Self::parse_with_errors(buf.as_ref());
        let mut iter = errors.drain(..);
        match iter.next() {
            Some(err) => Err(err),
            None => Ok(new),
        }
    }

    /// Parse a buffer and return a best-effort parsed `Config` object along with any errors.
    pub fn parse_with_errors(bytes: &[u8]) -> (Self, Vec<ParseError>) {
        use ParseError::*;
        let mut cfg = Self::new();
        let mut errors = Vec::new();
        'lines: for (lineno, line) in bytes.split(|&x| x == b'\n').enumerate() {
            for &c in line.iter() {
                if c != b'\t' && c != b' ' {
                    if c == b';' || c == b'#' {
                        continue 'lines;
                    } else {
                        break;
                    }
                }
            }

            // All that dances above to allow invalid utf-8 inside the comments
            let str = match from_utf8(line) {
                Ok(str) => str,
                Err(e) => {
                    errors.push(InvalidUtf8(lineno, e));
                    continue;
                }
            };

            // ignore everything after ';' or '#'
            let text = match str.split([';', '#']).next() {
                Some(text) => text,
                None => {
                    errors.push(InvalidValue(lineno));
                    continue;
                }
            };

            let mut words = text.split_whitespace();
            let keyword = match words.next() {
                Some(x) => x,
                None => continue,
            };

            match keyword {
                "nameserver" => {
                    let srv = match words.next() {
                        Some(srv) => srv,
                        None => {
                            errors.push(InvalidValue(lineno));
                            continue;
                        }
                    };

                    match ScopedIp::from_str(srv) {
                        Ok(addr) => cfg.nameservers.push(addr),
                        Err(e) => errors.push(InvalidIp(lineno, e)),
                    }

                    if words.next().is_some() {
                        errors.push(ExtraData(lineno));
                    }
                }
                "domain" => {
                    let domain = match words.next() {
                        Some(domain) => domain,
                        None => {
                            errors.push(InvalidValue(lineno));
                            continue;
                        }
                    };

                    cfg.set_domain(domain.to_owned());
                    if words.next().is_some() {
                        errors.push(ExtraData(lineno));
                    }
                }
                "search" => {
                    let mut search = Vec::new();
                    for word in words {
                        if !word.contains(',') {
                            // Easy to get confused between spaces and commas here
                            // https://github.com/hickory-dns/resolv-conf/issues/58
                            search.push(word.to_owned());
                        }
                    }
                    cfg.set_search(search);
                }
                "sortlist" => {
                    cfg.sortlist.clear();
                    for pair in words {
                        match Network::from_str(pair) {
                            Ok(network) => cfg.sortlist.push(network),
                            Err(e) => errors.push(InvalidIp(lineno, e)),
                        }
                    }
                }
                "options" => {
                    for pair in words {
                        let mut iter = pair.splitn(2, ':');
                        let key = match iter.next() {
                            Some(key) => key,
                            None => {
                                errors.push(InvalidValue(lineno));
                                continue 'lines;
                            }
                        };

                        let value = iter.next();
                        if iter.next().is_some() {
                            errors.push(ExtraData(lineno));
                            continue 'lines;
                        }

                        match (key, value) {
                            // TODO(tailhook) ensure that values are None?
                            ("debug", _) => cfg.debug = true,
                            ("ndots", Some(x)) => match u32::from_str(x) {
                                Ok(ndots) => cfg.ndots = ndots,
                                Err(_) => errors.push(InvalidOptionValue(lineno)),
                            },
                            ("timeout", Some(x)) => match u32::from_str(x) {
                                Ok(timeout) => cfg.timeout = timeout,
                                Err(_) => errors.push(InvalidOptionValue(lineno)),
                            },
                            ("attempts", Some(x)) => match u32::from_str(x) {
                                Ok(attempts) => cfg.attempts = attempts,
                                Err(_) => errors.push(InvalidOptionValue(lineno)),
                            },
                            ("rotate", _) => cfg.rotate = true,
                            ("no-check-names", _) => cfg.no_check_names = true,
                            ("inet6", _) => cfg.inet6 = true,
                            ("ip6-bytestring", _) => cfg.ip6_bytestring = true,
                            ("ip6-dotint", _) => cfg.ip6_dotint = true,
                            ("no-ip6-dotint", _) => cfg.ip6_dotint = false,
                            ("edns0", _) => cfg.edns0 = true,
                            ("single-request", _) => cfg.single_request = true,
                            ("single-request-reopen", _) => cfg.single_request_reopen = true,
                            ("no-reload", _) => cfg.no_reload = true,
                            ("trust-ad", _) => cfg.trust_ad = true,
                            ("no-tld-query", _) => cfg.no_tld_query = true,
                            ("use-vc", _) => cfg.use_vc = true,
                            ("no-aaaa", _) => cfg.no_aaaa = true,
                            _ => errors.push(InvalidOption(lineno)),
                        }
                    }
                }
                "lookup" => {
                    for word in words {
                        match word {
                            "file" => cfg.lookup.push(Lookup::File),
                            "bind" => cfg.lookup.push(Lookup::Bind),
                            extra => cfg.lookup.push(Lookup::Extra(extra.to_string())),
                        }
                    }
                }
                "family" => {
                    for word in words {
                        match word {
                            "inet4" => cfg.family.push(Family::Inet4),
                            "inet6" => cfg.family.push(Family::Inet6),
                            _ => errors.push(InvalidValue(lineno)),
                        }
                    }
                }
                _ => errors.push(InvalidDirective(lineno)),
            }
        }

        (cfg, errors)
    }

    /// Return the suffixes declared in the last "domain" or "search" directive.
    ///
    /// ```rust
    /// # extern crate resolv_conf;
    /// use resolv_conf::{ScopedIp, Config};
    /// # fn main() {
    /// let config_str = "search example.com sub.example.com\ndomain localdomain";
    /// let parsed_config = Config::parse(&config_str).expect("Failed to parse config");
    /// let domains = parsed_config.get_last_search_or_domain()
    ///                            .map(|domain| domain.clone())
    ///                            .collect::<Vec<String>>();
    /// assert_eq!(domains, vec![String::from("localdomain")]);
    ///
    /// let config_str = "domain localdomain\nsearch example.com sub.example.com";
    /// let parsed_config = Config::parse(&config_str).expect("Failed to parse config");
    /// let domains = parsed_config.get_last_search_or_domain()
    ///                            .map(|domain| domain.clone())
    ///                            .collect::<Vec<String>>();
    /// assert_eq!(domains, vec![String::from("example.com"), String::from("sub.example.com")]);
    /// # }
    pub fn get_last_search_or_domain(&self) -> DomainIter<'_> {
        let domain_iter = match self.last_search {
            LastSearch::Search => {
                DomainIterInternal::Search(self.get_search().map(|domains| domains.iter()))
            }
            LastSearch::Domain => DomainIterInternal::Domain(self.get_domain()),
            LastSearch::None => DomainIterInternal::None,
        };
        DomainIter(domain_iter)
    }

    /// Return the domain declared in the last "domain" directive.
    pub fn get_domain(&self) -> Option<&String> {
        self.domain.as_ref()
    }

    /// Return the domains declared in the last "search" directive.
    pub fn get_search(&self) -> Option<&Vec<String>> {
        self.search.as_ref()
    }

    /// Set the domain corresponding to the "domain" directive.
    pub fn set_domain(&mut self, domain: String) {
        self.domain = Some(domain);
        self.last_search = LastSearch::Domain;
    }

    /// Set the domains corresponding the "search" directive.
    pub fn set_search(&mut self, search: Vec<String>) {
        self.search = Some(search);
        self.last_search = LastSearch::Search;
    }

    /// Normalize config according to glibc rulees
    ///
    /// Currently this method does the following things:
    ///
    /// 1. Truncates list of nameservers to 3 at max
    /// 2. Truncates search list to 6 at max
    ///
    /// Other normalizations may be added in future as long as they hold true
    /// for a particular GNU libc implementation.
    ///
    /// Note: this method is not called after parsing, because we think it's
    /// not forward-compatible to rely on such small and ugly limits. Still,
    /// it's useful to keep implementation as close to glibc as possible.
    pub fn glibc_normalize(&mut self) {
        self.nameservers.truncate(NAMESERVER_LIMIT);
        self.search = self.search.take().map(|mut s| {
            s.truncate(SEARCH_LIMIT);
            s
        });
    }

    /// Get nameserver or on the local machine
    pub fn get_nameservers_or_local(&self) -> Vec<ScopedIp> {
        if self.nameservers.is_empty() {
            vec![
                ScopedIp::from(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
                ScopedIp::from(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1))),
            ]
        } else {
            self.nameservers.to_vec()
        }
    }

    /// Get domain from config or fallback to the suffix of a hostname
    ///
    /// This is how glibc finds out a hostname.
    pub fn get_system_domain(&self) -> Option<String> {
        if self.domain.is_some() {
            return self.domain.clone();
        }

        // This buffer is far larger than what most systems will ever allow, eg.
        // linux uses 64 via _SC_HOST_NAME_MAX even though POSIX says the size
        // must be _at least_ _POSIX_HOST_NAME_MAX (255), but other systems can
        // be larger, so we just use a sufficiently sized buffer so we can defer
        // a heap allocation until the last possible moment.
        let mut hostname = [0u8; 1024];

        #[cfg(all(target_os = "linux", target_feature = "crt-static"))]
        {
            use std::{fs::File, io::Read};
            let mut file = File::open("/proc/sys/kernel/hostname").ok()?;
            let read_bytes = file.read(&mut hostname).ok()?;

            // According to Linux kernel's proc_dostring handler, user-space reads
            // of /proc/sys entries which have a string value are terminated by
            // a newline character. While libc gethostname() terminates the hostname
            // with a null character. Hence, to match the behavior of gethostname()
            // it is necessary to replace the newline with a null character.
            if read_bytes == hostname.len() && hostname[read_bytes - 1] != b'\n' {
                // In this case the string read from /proc/sys/kernel/hostname is
                // truncated and cannot be terminated by a null character
                return None;
            }
            // Since any non-truncated string read from /proc/sys/kernel/hostname
            // ends with a newline character, read_bytes > 0.
            hostname[read_bytes - 1] = 0;
        }

        #[cfg(not(all(target_os = "linux", target_feature = "crt-static")))]
        {
            /*unsafe*/
            extern "C" {
                fn gethostname(hostname: *mut u8, size: usize) -> i32;
            }

            unsafe {
                if gethostname(hostname.as_mut_ptr(), hostname.len()) < 0 {
                    return None;
                }
            }
        }

        domain_from_host(&hostname).map(|s| s.to_owned())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            nameservers: Vec::new(),
            domain: None,
            search: None,
            last_search: LastSearch::None,
            sortlist: Vec::new(),
            debug: false,
            ndots: 1,
            timeout: 5,
            attempts: 2,
            rotate: false,
            no_check_names: false,
            inet6: false,
            ip6_bytestring: false,
            ip6_dotint: false,
            edns0: false,
            single_request: false,
            single_request_reopen: false,
            no_tld_query: false,
            use_vc: false,
            no_reload: false,
            trust_ad: false,
            lookup: Vec::new(),
            family: Vec::new(),
            no_aaaa: false,
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Self {
            nameservers,
            last_search,
            domain,
            search,
            sortlist,
            debug,
            ndots,
            timeout,
            attempts,
            rotate,
            no_check_names,
            inet6,
            ip6_bytestring,
            ip6_dotint,
            edns0,
            single_request,
            single_request_reopen,
            no_tld_query,
            use_vc,
            no_reload,
            trust_ad,
            lookup,
            family,
            no_aaaa,
        } = self;

        for nameserver in nameservers.iter() {
            writeln!(fmt, "nameserver {nameserver}")?;
        }

        if last_search != &LastSearch::Domain {
            if let Some(domain) = domain {
                writeln!(fmt, "domain {domain}")?;
            }
        }

        if let Some(search) = search {
            if !search.is_empty() {
                write!(fmt, "search")?;
                for suffix in search.iter() {
                    write!(fmt, " {suffix}")?;
                }
                writeln!(fmt)?;
            }
        }

        if last_search == &LastSearch::Domain {
            if let Some(domain) = &self.domain {
                writeln!(fmt, "domain {domain}")?;
            }
        }

        if !sortlist.is_empty() {
            write!(fmt, "sortlist")?;
            for network in sortlist.iter() {
                write!(fmt, " {network}")?;
            }
            writeln!(fmt)?;
        }

        if !lookup.is_empty() {
            write!(fmt, "lookup")?;
            for db in lookup.iter() {
                match db {
                    Lookup::File => write!(fmt, " file")?,
                    Lookup::Bind => write!(fmt, " bind")?,
                    Lookup::Extra(extra) => write!(fmt, " {extra}")?,
                }
            }
            writeln!(fmt)?;
        }

        if !family.is_empty() {
            write!(fmt, "family")?;
            for fam in family.iter() {
                match fam {
                    Family::Inet4 => write!(fmt, " inet4")?,
                    Family::Inet6 => write!(fmt, " inet6")?,
                }
            }
            writeln!(fmt)?;
        }

        if *debug {
            writeln!(fmt, "options debug")?;
        }
        if *ndots != 1 {
            writeln!(fmt, "options ndots:{}", self.ndots)?;
        }
        if *timeout != 5 {
            writeln!(fmt, "options timeout:{}", self.timeout)?;
        }
        if *attempts != 2 {
            writeln!(fmt, "options attempts:{}", self.attempts)?;
        }
        if *rotate {
            writeln!(fmt, "options rotate")?;
        }
        if *no_check_names {
            writeln!(fmt, "options no-check-names")?;
        }
        if *inet6 {
            writeln!(fmt, "options inet6")?;
        }
        if *ip6_bytestring {
            writeln!(fmt, "options ip6-bytestring")?;
        }
        if *ip6_dotint {
            writeln!(fmt, "options ip6-dotint")?;
        }
        if *edns0 {
            writeln!(fmt, "options edns0")?;
        }
        if *single_request {
            writeln!(fmt, "options single-request")?;
        }
        if *single_request_reopen {
            writeln!(fmt, "options single-request-reopen")?;
        }
        if *no_tld_query {
            writeln!(fmt, "options no-tld-query")?;
        }
        if *use_vc {
            writeln!(fmt, "options use-vc")?;
        }
        if *no_reload {
            writeln!(fmt, "options no-reload")?;
        }
        if *trust_ad {
            writeln!(fmt, "options trust-ad")?;
        }
        if *no_aaaa {
            writeln!(fmt, "options no-aaaa")?;
        }

        Ok(())
    }
}

/// Error while parsing resolv.conf file
#[derive(Debug)]
pub enum ParseError {
    /// Error that may be returned when the string to parse contains invalid UTF-8 sequences
    InvalidUtf8(usize, Utf8Error),
    /// Error returned a value for a given directive is invalid.
    /// This can also happen when the value is missing, if the directive requires a value.
    InvalidValue(usize),
    /// Error returned when a value for a given option is invalid.
    /// This can also happen when the value is missing, if the option requires a value.
    InvalidOptionValue(usize),
    /// Error returned when a invalid option is found.
    InvalidOption(usize),
    /// Error returned when a invalid directive is found.
    InvalidDirective(usize),
    /// Error returned when a value cannot be parsed an an IP address.
    InvalidIp(usize, AddrParseError),
    /// Error returned when there is extra data at the end of a line.
    ExtraData(usize),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InvalidUtf8(line, err) => write!(f, "bad unicode at line {line}: {err}"),
            Self::InvalidValue(line) => write!(
                f,
                "directive at line {line} is improperly formatted or contains invalid value",
            ),
            Self::InvalidOptionValue(line) => write!(
                f,
                "directive options at line {line} contains invalid value of some option",
            ),
            Self::InvalidOption(line) => {
                write!(f, "option at line {line} is not recognized")
            }
            Self::InvalidDirective(line) => {
                write!(f, "directive at line {line} is not recognized")
            }
            Self::InvalidIp(line, err) => {
                write!(f, "directive at line {line} contains invalid IP: {err}")
            }
            Self::ExtraData(line) => write!(f, "extra data at the end of line {line}"),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidUtf8(_, err) => Some(err),
            _ => None,
        }
    }
}

/// An iterator returned by [`Config.get_last_search_or_domain`](struct.Config.html#method.get_last_search_or_domain)
#[derive(Debug, Clone)]
pub struct DomainIter<'a>(DomainIterInternal<'a>);

impl<'a> Iterator for DomainIter<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

#[derive(Debug, Clone)]
enum DomainIterInternal<'a> {
    Search(Option<Iter<'a, String>>),
    Domain(Option<&'a String>),
    None,
}

impl<'a> Iterator for DomainIterInternal<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            DomainIterInternal::Search(Some(domains)) => domains.next(),
            DomainIterInternal::Domain(domain) => domain.take(),
            _ => None,
        }
    }
}

/// The databases that should be searched during a lookup.
/// This option is commonly found on openbsd.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
    /// Search for entries in /etc/hosts
    File,
    /// Query a domain name server
    Bind,
    /// A database we don't know yet
    Extra(String),
}

/// The internet protocol family that is prefered.
/// This option is commonly found on openbsd.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Family {
    /// A A lookup for an ipv4 address
    Inet4,
    /// A AAAA lookup for an ipv6 address
    Inet6,
}

/// Parses the domain name from a hostname, if available
fn domain_from_host(hostname: &[u8]) -> Option<&str> {
    let mut start = None;
    for (i, b) in hostname.iter().copied().enumerate() {
        if b == b'.' && start.is_none() {
            start = Some(i);
            continue;
        } else if b > 0 {
            continue;
        }

        return match start? {
            // Avoid empty domains
            start if i - start < 2 => None,
            start => str::from_utf8(&hostname[start + 1..i]).ok(),
        };
    }

    None
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum LastSearch {
    None,
    Domain,
    Search,
}

const NAMESERVER_LIMIT: usize = 3;
const SEARCH_LIMIT: usize = 6;

#[cfg(test)]
mod test {
    use super::domain_from_host;
    use super::Config;

    #[test]
    fn parses_domain_name() {
        assert!(domain_from_host(b"regular-hostname\0").is_none());

        assert_eq!(domain_from_host(b"with.domain-name\0"), Some("domain-name"));
        assert_eq!(
            domain_from_host(b"with.multiple.dots\0"),
            Some("multiple.dots")
        );

        assert!(domain_from_host(b"hostname.\0").is_none());
        assert_eq!(domain_from_host(b"host.a\0"), Some("a"));
        assert_eq!(domain_from_host(b"host.au\0"), Some("au"));
    }

    #[test]
    fn comma_search() {
        let config = Config::parse(COMMA_SEARCH).unwrap();
        assert_eq!(
            config.get_search().unwrap(),
            &["another.example.com".to_string(),]
        );
    }

    const COMMA_SEARCH: &str = r#"search example.com,sub.example.com another.example.com"#;
}
