use std::time::SystemTime;

use fern::colors::{Color, ColoredLevelConfig};
use log::{debug, error, warn};

fn main() {
    let colors = ColoredLevelConfig::new().debug(Color::Magenta);

    fern::Dispatch::new()
        .chain(std::io::stdout())
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339_seconds(SystemTime::now()),
                // This will color the log level only, not the whole line. Just a touch.
                colors.color(record.level()),
                record.target(),
                message
            ))
        })
        .apply()
        .unwrap();

    error!("hi");
    debug!("sup");
    warn!("oh");
}
