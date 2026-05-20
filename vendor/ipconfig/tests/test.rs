use ipconfig;

#[test]
fn no_error_adapters() {
    println!("Adapters: {:#?}", ipconfig::get_adapters().unwrap());
}

#[cfg(feature = "computer")]
#[test]
fn no_error_computer() {
    println!(
        "Search list: {:#?}",
        ipconfig::computer::get_search_list().unwrap()
    );
    println!("Domain: {:#?}", ipconfig::computer::get_domain().unwrap());
    println!(
        "Is round robin enabled: {:#?}",
        ipconfig::computer::is_round_robin_enabled().unwrap()
    );
}
