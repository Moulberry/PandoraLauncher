//! Compare parse results with the `semver` crate.

#[test]
fn zeroes() {
    let vs = ["1.2.2-0a", "1.2.2-00a"];
    for v in vs {
        semver_parse_both(v);
        semver_parser_parse_both(v);
    }
}

fn semver_parse_both(v: &str) {
    let sv = semver::Version::parse(v).unwrap();
    assert_eq!(v, sv.to_string());
}

fn semver_parser_parse_both(v: &str) {
    let sv = semver_parser::version::parse(v).unwrap();
    assert_eq!(v, sv.to_string());
}
