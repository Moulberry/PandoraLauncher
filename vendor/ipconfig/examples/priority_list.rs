fn main() {
    let mut adapters = ipconfig::get_adapters().unwrap();
    adapters.sort_by(|ip1, ip2| ip1.ipv4_metric().cmp(&ip2.ipv4_metric()));
    for adapter in adapters {
        println!(
            "{}: IfType: {:?}  IPs: {:?} - IPv4 metric: {} IPv6 metric: {}",
            adapter.friendly_name(),
            adapter.if_type(),
            adapter.ip_addresses(),
            adapter.ipv4_metric(),
            adapter.ipv6_metric()
        )
    }
}
