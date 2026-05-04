use crate::{Address, Result, process::run};

#[derive(Clone, Debug, PartialEq, Eq)]
/// The transport properties of an IBus D-Bus address.
///
/// This transport type queries the IBus daemon for its D-Bus address using the `ibus address`
/// command. IBus (Intelligent Input Bus) is an input method framework used primarily on Linux
/// systems for entering text in various languages.
///
/// # Platform Support
///
/// This transport is available on Unix-like systems where IBus is installed.
///
/// # Example
///
/// ```no_run
/// # use zbus::address::transport::{Transport, Ibus};
/// # use zbus::Address;
/// #
/// // Create an IBus transport
/// let ibus = Ibus::new();
/// let _addr = Transport::Ibus(ibus);
///
/// // Or use it directly as an address
/// let _addr = Address::from(Transport::Ibus(Ibus::new()));
/// ```
pub struct Ibus;

impl Ibus {
    /// Create a new IBus transport.
    ///
    /// This will query the IBus daemon for its D-Bus address when the connection is established.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Determine the actual transport details behind an IBus address.
    ///
    /// This method executes the `ibus address` command to retrieve the D-Bus address from the
    /// running IBus daemon, then parses and returns the underlying transport.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `ibus` command is not found or fails to execute
    /// - The IBus daemon is not running
    /// - The command output cannot be parsed as a valid D-Bus address
    /// - The command output is not valid UTF-8
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use zbus::connection::Builder;
    /// # use zbus::block_on;
    /// #
    /// # block_on(async {
    /// // This method is used internally by the connection builder
    /// let _conn = Builder::ibus()?.build().await?;
    /// # Ok::<(), zbus::Error>(())
    /// # }).unwrap();
    /// ```
    pub(super) async fn bus_address(&self) -> Result<Address> {
        let output = run("ibus", ["address"])
            .await
            .map_err(|e| crate::Error::Address(format!("Failed to execute ibus command: {e}")))?;

        if !output.status.success() {
            return Err(crate::Error::Address(format!(
                "ibus terminated with code: {}",
                output.status
            )));
        }

        let addr = String::from_utf8(output.stdout).map_err(|e| {
            crate::Error::Address(format!("Unable to parse ibus output as UTF-8: {e}"))
        })?;

        addr.trim().parse()
    }

    /// Parse IBus transport from D-Bus address options.
    ///
    /// The IBus transport type does not require any options, so this method will succeed
    /// as long as the transport type is specified as "ibus".
    ///
    /// # Errors
    ///
    /// This method does not return errors for the IBus transport, but the signature is kept
    /// consistent with other transport types.
    pub(super) fn from_options(_opts: std::collections::HashMap<&str, &str>) -> Result<Self> {
        Ok(Self)
    }
}

impl Default for Ibus {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Ibus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ibus:")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ibus_new() {
        let ibus = Ibus::new();
        assert_eq!(ibus.to_string(), "ibus:");
    }

    #[test]
    fn test_ibus_default() {
        let ibus = Ibus::default();
        assert_eq!(ibus.to_string(), "ibus:");
    }

    #[test]
    fn test_ibus_from_options() {
        let options = std::collections::HashMap::new();
        let ibus = Ibus::from_options(options).unwrap();
        assert_eq!(ibus, Ibus::new());
    }

    #[test]
    fn test_ibus_display() {
        let ibus = Ibus::new();
        assert_eq!(format!("{}", ibus), "ibus:");
    }
}
