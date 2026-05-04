use std::net::{Ipv4Addr, Ipv6Addr};

use fluent_uri::{enc::EStr, ParseErrorKind::*, *};

#[test]
fn parse_absolute() {
    let u = Uri::parse("file:///etc/hosts").unwrap();
    assert_eq!(u.as_str(), "file:///etc/hosts");
    assert_eq!(u.scheme().unwrap().as_str(), "file");
    let a = u.authority().unwrap();
    assert_eq!(a.as_str(), "");
    assert_eq!(a.userinfo(), None);
    assert_eq!(a.host().as_str(), "");
    assert_eq!(a.host().data(), HostData::RegName(EStr::new("")));
    assert_eq!(a.port(), None);
    assert_eq!(u.path().as_str(), "/etc/hosts");
    assert!(u.path().segments().eq(["etc", "hosts"]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("ftp://ftp.is.co.za/rfc/rfc1808.txt").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "ftp");
    let a = u.authority().unwrap();
    assert_eq!(a.as_str(), "ftp.is.co.za");
    assert_eq!(a.userinfo(), None);
    assert_eq!(a.host().as_str(), "ftp.is.co.za");
    assert_eq!(
        a.host().data(),
        HostData::RegName(EStr::new("ftp.is.co.za"))
    );
    assert_eq!(a.port(), None);
    assert_eq!(u.path().as_str(), "/rfc/rfc1808.txt");
    assert!(u.path().segments().eq(["rfc", "rfc1808.txt"]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("http://www.ietf.org/rfc/rfc2396.txt").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "http");
    let a = u.authority().unwrap();
    assert_eq!(a.as_str(), "www.ietf.org");
    assert_eq!(a.userinfo(), None);
    assert_eq!(a.host().as_str(), "www.ietf.org");
    assert_eq!(
        a.host().data(),
        HostData::RegName(EStr::new("www.ietf.org"))
    );
    assert_eq!(a.port(), None);
    assert_eq!(u.path().as_str(), "/rfc/rfc2396.txt");
    assert!(u.path().segments().eq(["rfc", "rfc2396.txt"]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("ldap://[2001:db8::7]/c=GB?objectClass?one").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "ldap");
    let a = u.authority().unwrap();
    assert_eq!(a.as_str(), "[2001:db8::7]");
    assert_eq!(a.userinfo(), None);
    assert_eq!(a.host().as_str(), "[2001:db8::7]");
    assert_eq!(
        a.host().data(),
        HostData::Ipv6 {
            addr: Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0x7),
            #[cfg(feature = "rfc6874bis")]
            zone_id: None
        }
    );
    assert_eq!(a.port(), None);
    assert_eq!(u.path().as_str(), "/c=GB");
    assert!(u.path().segments().eq(["c=GB"]));
    assert_eq!(u.query(), Some(EStr::new("objectClass?one")));
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("mailto:John.Doe@example.com").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "mailto");
    assert!(u.authority().is_none());
    assert_eq!(u.path().as_str(), "John.Doe@example.com");
    assert!(u.path().segments().eq(["John.Doe@example.com"]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("news:comp.infosystems.www.servers.unix").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "news");
    assert!(u.authority().is_none());
    assert_eq!(u.path().as_str(), "comp.infosystems.www.servers.unix");
    assert!(u
        .path()
        .segments()
        .eq(["comp.infosystems.www.servers.unix"]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("tel:+1-816-555-1212").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "tel");
    assert!(u.authority().is_none());
    assert_eq!(u.path().as_str(), "+1-816-555-1212");
    assert!(u.path().segments().eq(["+1-816-555-1212"]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("telnet://192.0.2.16:80/").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "telnet");
    let a = u.authority().unwrap();
    assert_eq!(a.as_str(), "192.0.2.16:80");
    assert_eq!(a.userinfo(), None);
    assert_eq!(a.host().as_str(), "192.0.2.16");
    assert_eq!(
        a.host().data(),
        HostData::Ipv4(Ipv4Addr::new(192, 0, 2, 16))
    );
    assert_eq!(a.port(), Some("80"));
    assert_eq!(u.path().as_str(), "/");
    assert!(u.path().segments().eq([""]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("urn:oasis:names:specification:docbook:dtd:xml:4.1.2").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "urn");
    assert!(u.authority().is_none());
    assert_eq!(
        u.path().as_str(),
        "oasis:names:specification:docbook:dtd:xml:4.1.2"
    );
    assert!(u
        .path()
        .segments()
        .eq(["oasis:names:specification:docbook:dtd:xml:4.1.2"]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("foo://example.com:8042/over/there?name=ferret#nose").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "foo");
    let a = u.authority().unwrap();
    assert_eq!(a.as_str(), "example.com:8042");
    assert_eq!(a.userinfo(), None);
    assert_eq!(a.host().as_str(), "example.com");
    assert_eq!(a.host().data(), HostData::RegName(EStr::new("example.com")));
    assert_eq!(a.port(), Some("8042"));
    assert_eq!(u.path().as_str(), "/over/there");
    assert!(u.path().segments().eq(["over", "there"]));
    assert_eq!(u.query(), Some(EStr::new("name=ferret")));
    assert_eq!(u.fragment(), Some(EStr::new("nose")));

    let u = Uri::parse("ftp://cnn.example.com&story=breaking_news@10.0.0.1/top_story.htm").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "ftp");
    let a = u.authority().unwrap();
    assert_eq!(a.as_str(), "cnn.example.com&story=breaking_news@10.0.0.1");
    assert_eq!(
        a.userinfo(),
        Some(EStr::new("cnn.example.com&story=breaking_news"))
    );
    assert_eq!(a.host().as_str(), "10.0.0.1");
    assert_eq!(a.host().data(), HostData::Ipv4(Ipv4Addr::new(10, 0, 0, 1)));
    assert_eq!(a.port(), None);
    assert_eq!(u.path().as_str(), "/top_story.htm");
    assert!(u.path().segments().eq(["top_story.htm"]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    #[cfg(feature = "ipv_future")]
    {
        let u = Uri::parse("http://[vFe.foo.bar]").unwrap();
        assert_eq!(u.scheme().unwrap().as_str(), "http");
        let a = u.authority().unwrap();
        assert_eq!(a.as_str(), "[vFe.foo.bar]");
        assert_eq!(a.userinfo(), None);
        assert_eq!(a.host().as_str(), "[vFe.foo.bar]");
        assert_eq!(
            a.host().data(),
            HostData::IpvFuture {
                ver: "Fe",
                addr: "foo.bar",
            }
        );
        assert_eq!(a.port(), None);
        assert_eq!(u.path().as_str(), "");
        assert!(u.path().segments().eq(None::<&str>));
        assert_eq!(u.query(), None);
        assert_eq!(u.fragment(), None);
    }

    #[cfg(feature = "rfc6874bis")]
    {
        let u = Uri::parse("http://[fe80::520f:f5ff:fe51:cf0%17]").unwrap();
        assert_eq!(u.scheme().unwrap().as_str(), "http");
        let a = u.authority().unwrap();
        assert_eq!(a.as_str(), "[fe80::520f:f5ff:fe51:cf0%17]");
        assert_eq!(a.userinfo(), None);
        assert_eq!(a.host().as_str(), "[fe80::520f:f5ff:fe51:cf0%17]");
        assert_eq!(
            a.host().data(),
            HostData::Ipv6 {
                addr: Ipv6Addr::new(0xfe80, 0, 0, 0, 0x520f, 0xf5ff, 0xfe51, 0xcf0),
                zone_id: Some("17"),
            }
        );
        assert_eq!(a.port(), None);
        assert_eq!(u.path().as_str(), "");
        assert!(u.path().segments().eq(None::<&str>));
        assert_eq!(u.query(), None);
        assert_eq!(u.fragment(), None);
    }

    let u = Uri::parse("http://127.0.0.1:/").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "http");
    let a = u.authority().unwrap();
    assert_eq!(a.as_str(), "127.0.0.1:");
    assert_eq!(a.userinfo(), None);
    assert_eq!(a.host().as_str(), "127.0.0.1");
    assert_eq!(a.host().data(), HostData::Ipv4(Ipv4Addr::new(127, 0, 0, 1)));
    assert_eq!(a.port(), Some(""));
    // TODO: `u16` port parsing.
    assert_eq!(u.path().as_str(), "/");
    assert!(u.path().segments().eq([""]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("http://127.0.0.1:8080/").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "http");
    let a = u.authority().unwrap();
    assert_eq!(a.as_str(), "127.0.0.1:8080");
    assert_eq!(a.userinfo(), None);
    assert_eq!(a.host().as_str(), "127.0.0.1");
    assert_eq!(a.host().data(), HostData::Ipv4(Ipv4Addr::new(127, 0, 0, 1)));
    assert_eq!(a.port(), Some("8080"));
    assert_eq!(u.path().as_str(), "/");
    assert!(u.path().segments().eq([""]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("http://127.0.0.1:80808/").unwrap();
    assert_eq!(u.scheme().unwrap().as_str(), "http");
    let a = u.authority().unwrap();
    assert_eq!(a.as_str(), "127.0.0.1:80808");
    assert_eq!(a.userinfo(), None);
    assert_eq!(a.host().as_str(), "127.0.0.1");
    assert_eq!(a.host().data(), HostData::Ipv4(Ipv4Addr::new(127, 0, 0, 1)));
    assert_eq!(a.port(), Some("80808"));
    // TODO: `u16` port parsing.
    assert_eq!(u.path().as_str(), "/");
    assert!(u.path().segments().eq([""]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);
}

#[test]
fn parse_relative() {
    let u = Uri::parse("").unwrap();
    assert!(u.scheme().is_none());
    assert!(u.authority().is_none());
    assert_eq!(u.path().as_str(), "");
    assert!(u.path().segments().eq(None::<&str>));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("foo.txt").unwrap();
    assert!(u.scheme().is_none());
    assert!(u.authority().is_none());
    assert_eq!(u.path().as_str(), "foo.txt");
    assert!(u.path().segments().eq(["foo.txt"]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse(".").unwrap();
    assert!(u.scheme().is_none());
    assert!(u.authority().is_none());
    assert_eq!(u.path().as_str(), ".");
    assert!(u.path().segments().eq(["."]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("./this:that").unwrap();
    assert!(u.scheme().is_none());
    assert!(u.authority().is_none());
    assert_eq!(u.path().as_str(), "./this:that");
    assert!(u.path().segments().eq([".", "this:that"]));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("//example.com").unwrap();
    assert!(u.scheme().is_none());
    let a = u.authority().unwrap();
    assert_eq!(a.as_str(), "example.com");
    assert_eq!(a.userinfo(), None);
    assert_eq!(a.host().as_str(), "example.com");
    assert_eq!(a.host().data(), HostData::RegName(EStr::new("example.com")));
    assert_eq!(a.port(), None);
    assert_eq!(u.path().as_str(), "");
    assert!(u.path().segments().eq(None::<&str>));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("?query").unwrap();
    assert!(u.scheme().is_none());
    assert!(u.authority().is_none());
    assert_eq!(u.path().as_str(), "");
    assert!(u.path().segments().eq(None::<&str>));
    assert_eq!(u.query(), Some(EStr::new("query")));
    assert_eq!(u.fragment(), None);

    let u = Uri::parse("#fragment").unwrap();
    assert!(u.scheme().is_none());
    assert!(u.authority().is_none());
    assert_eq!(u.path().as_str(), "");
    assert!(u.path().segments().eq(None::<&str>));
    assert_eq!(u.query(), None);
    assert_eq!(u.fragment(), Some(EStr::new("fragment")));
}

#[test]
fn parse_error() {
    // Empty scheme
    let e = Uri::parse(":hello").unwrap_err();
    assert_eq!(e.index(), 0);
    assert_eq!(e.kind(), UnexpectedChar);

    // Scheme starts with non-letter
    let e = Uri::parse("3ttp://a.com").unwrap_err();
    assert_eq!(e.index(), 0);
    assert_eq!(e.kind(), UnexpectedChar);

    // After rewriting the parser, the following two cases are interpreted as
    // containing colon in the first path segment of a relative reference.

    // Unexpected char in scheme
    let e = Uri::parse("exam=ple:foo").unwrap_err();
    assert_eq!(e.index(), 8);
    assert_eq!(e.kind(), UnexpectedChar);

    let e = Uri::parse("(:").unwrap_err();
    assert_eq!(e.index(), 1);
    assert_eq!(e.kind(), UnexpectedChar);

    // Percent-encoded scheme
    let e = Uri::parse("a%20:foo").unwrap_err();
    assert_eq!(e.index(), 4);
    assert_eq!(e.kind(), UnexpectedChar);

    // Unexpected char in path
    let e = Uri::parse("foo\\bar").unwrap_err();
    assert_eq!(e.index(), 3);
    assert_eq!(e.kind(), UnexpectedChar);

    // Non-hexadecimal percent-encoded octet
    let e = Uri::parse("foo%xxd").unwrap_err();
    assert_eq!(e.index(), 3);
    assert_eq!(e.kind(), InvalidOctet);

    // Incomplete percent-encoded octet
    let e = Uri::parse("text%a").unwrap_err();
    assert_eq!(e.index(), 4);
    assert_eq!(e.kind(), InvalidOctet);

    // A single percent
    let e = Uri::parse("%").unwrap_err();
    assert_eq!(e.index(), 0);
    assert_eq!(e.kind(), InvalidOctet);

    // Non-decimal port
    // In this case the port is validated in reverse.
    let e = Uri::parse("http://example.com:80ab").unwrap_err();
    assert_eq!(e.index(), 22);
    assert_eq!(e.kind(), UnexpectedChar);

    let e = Uri::parse("http://user@example.com:80ab").unwrap_err();
    assert_eq!(e.index(), 26);
    assert_eq!(e.kind(), UnexpectedChar);

    // Multiple colons in authority
    let e = Uri::parse("http://user:pass:example.com/").unwrap_err();
    assert_eq!(e.index(), 11);
    assert_eq!(e.kind(), UnexpectedChar);

    // Unclosed bracket
    let e = Uri::parse("https://[::1/").unwrap_err();
    assert_eq!(e.index(), 8);
    assert_eq!(e.kind(), InvalidIpLiteral);

    // Not port after IP literal
    let e = Uri::parse("https://[::1]wrong").unwrap_err();
    assert_eq!(e.index(), 13);
    assert_eq!(e.kind(), UnexpectedChar);

    // IP literal too short
    let e = Uri::parse("http://[:]").unwrap_err();
    assert_eq!(e.index(), 7);
    assert_eq!(e.kind(), InvalidIpLiteral);
    let e = Uri::parse("http://[]").unwrap_err();
    assert_eq!(e.index(), 7);
    assert_eq!(e.kind(), InvalidIpLiteral);

    // Non-hexadecimal version in IPvFuture
    let e = Uri::parse("http://[vG.addr]").unwrap_err();
    assert_eq!(e.index(), 7);
    assert_eq!(e.kind(), InvalidIpLiteral);

    // Empty version in IPvFuture
    let e = Uri::parse("http://[v.addr]").unwrap_err();
    assert_eq!(e.index(), 7);
    assert_eq!(e.kind(), InvalidIpLiteral);

    // Empty address in IPvFuture
    let e = Uri::parse("ftp://[vF.]").unwrap_err();
    assert_eq!(e.index(), 6);
    assert_eq!(e.kind(), InvalidIpLiteral);

    // Percent-encoded address in IPvFuture
    let e = Uri::parse("ftp://[vF.%20]").unwrap_err();
    assert_eq!(e.index(), 6);
    assert_eq!(e.kind(), InvalidIpLiteral);

    // Empty Zone ID
    #[cfg(feature = "rfc6874bis")]
    {
        let e = Uri::parse("ftp://[fe80::abcd%]").unwrap_err();
        assert_eq!(e.index(), 6);
        assert_eq!(e.kind(), InvalidIpLiteral);
    }

    // Zone ID when the feature isn't enabled.
    #[cfg(not(feature = "rfc6874bis"))]
    {
        let e = Uri::parse("ftp://[fe80::abcd%eth0]").unwrap_err();
        assert_eq!(e.index(), 6);
        assert_eq!(e.kind(), InvalidIpLiteral);
    }

    // Invalid IPv6 address
    let e = Uri::parse("example://[44:55::66::77]").unwrap_err();
    assert_eq!(e.index(), 10);
    assert_eq!(e.kind(), InvalidIpLiteral);

    // IPvFuture when the feature isn't enabled.
    #[cfg(not(feature = "ipv_future"))]
    {
        let e = Uri::parse("http://[vFe.foo.bar]").unwrap_err();
        assert_eq!(e.index(), 7);
        assert_eq!(e.kind(), InvalidIpLiteral);
    }
}

#[test]
fn strict_ip_addr() {
    let u = Uri::parse("//127.0.0.001").unwrap();
    let a = u.authority().unwrap();
    assert!(matches!(a.host().data(), HostData::RegName(_)));

    let u = Uri::parse("//127.1").unwrap();
    let a = u.authority().unwrap();
    assert!(matches!(a.host().data(), HostData::RegName(_)));

    let u = Uri::parse("//127.00.00.1").unwrap();
    let a = u.authority().unwrap();
    assert!(matches!(a.host().data(), HostData::RegName(_)));

    assert!(Uri::parse("//[::1.1.1.1]").is_ok());
    assert!(Uri::parse("//[::ffff:1.1.1.1]").is_ok());
    assert!(Uri::parse("//[0000:0000:0000:0000:0000:0000:255.255.255.255]").is_ok());

    assert_eq!(
        Uri::parse("//[::01.1.1.1]").unwrap_err().kind(),
        InvalidIpLiteral
    );
    assert_eq!(
        Uri::parse("//[::00.1.1.1]").unwrap_err().kind(),
        InvalidIpLiteral
    );
}
