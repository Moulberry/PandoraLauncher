extern crate udev;

use std::io;

fn main() -> io::Result<()> {
    let mut enumerator = udev::Enumerator::new()?;

    for device in enumerator.scan_devices()? {
        println!();
        println!("{:#?}", device);

        println!("  [properties]");
        for property in device.properties() {
            println!("    - {:?} {:?}", property.name(), property.value());
        }

        println!("  [attributes]");
        for attribute in device.attributes() {
            println!("    - {:?} {:?}", attribute.name(), attribute.value());
        }
    }

    Ok(())
}
