//! Shared protocol types for DSPradio broker system
//!
//! This crate defines the types shared between the DSPradio library,
//! broker daemon, and client applications.

pub mod discovery;
pub mod messages;
pub mod priority;

pub use discovery::{BrokerEvent, DeviceAdvertisement, DeviceEvent, DspRadioDiscovery};
pub use messages::{
    DeviceCapabilities, DeviceInfo, DeviceStatus, DeviceType, LeaseConfig, LeaseRequest,
    LeaseResponse, LeaseState,
};
pub use priority::Priority;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a device based on its serial number
///
/// Device serials are stable across USB port changes, allowing the broker
/// to track devices even when they're moved between ports or hosts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceSerial(pub String);

impl DeviceSerial {
    /// Create a new device serial from a string
    pub fn new(serial: impl Into<String>) -> Self {
        Self(serial.into())
    }

    /// Get the serial as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceSerial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for DeviceSerial {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for DeviceSerial {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Unique identifier for a lease
pub type LeaseId = Uuid;

/// Unique identifier for a client
pub type ClientId = Uuid;

/// Unified gain configuration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Gain {
    /// Automatic gain control
    Auto,
    /// Manual gain in dB
    #[serde(rename = "manual")]
    Manual(f64),
}

impl Default for Gain {
    fn default() -> Self {
        Self::Auto
    }
}

/// Protocol version for compatibility checking
pub const PROTOCOL_VERSION: u32 = 1;

/// Default broker port (per project conventions)
pub const DEFAULT_BROKER_PORT: u16 = 8420;

/// mDNS service type for DSPradio
pub const MDNS_SERVICE_TYPE: &str = "_dspradio._tcp.local.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_serial_equality() {
        let s1 = DeviceSerial::new("ABC123");
        let s2 = DeviceSerial::new("ABC123");
        let s3 = DeviceSerial::new("DEF456");

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_device_serial_display() {
        let serial = DeviceSerial::new("RTL2838");
        assert_eq!(serial.to_string(), "RTL2838");
    }

    #[test]
    fn test_gain_serialization() {
        let auto = Gain::Auto;
        let manual = Gain::Manual(40.0);

        assert_eq!(serde_json::to_string(&auto).unwrap(), r#""auto""#);
        assert_eq!(serde_json::to_string(&manual).unwrap(), r#"{"manual":40.0}"#);
    }
}
