//! mDNS service discovery for DSPradio broker and devices
//!
//! This module handles automatic discovery of brokers and SDR devices
//! on the local network using mDNS/DNS-SD (Bonjour-compatible).
//!
//! # Service Types
//!
//! - `_dspradio._tcp.local.` - Main service type for brokers and devices
//!
//! # TXT Records
//!
//! Brokers advertise:
//! - `version=1` - Protocol version
//! - `role=broker` - Identifies as broker
//! - `api_version=v1` - REST API version
//!
//! Devices advertise:
//! - `version=1` - Protocol version
//! - `role=device` - Identifies as device host
//! - `serial=<serial>` - Device serial number
//! - `type=rtlsdr|plutosdr|soapy` - Device type
//! - `broker=<hostname>` - Associated broker (if any)

use std::collections::HashMap;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use thiserror::Error;

use crate::{DeviceSerial, DeviceType, MDNS_SERVICE_TYPE, PROTOCOL_VERSION};

/// Discovery errors
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("mDNS daemon error: {0}")]
    Daemon(String),
    #[error("Service registration failed: {0}")]
    Registration(String),
    #[error("Invalid service info: {0}")]
    InvalidService(String),
}

/// Role in the DSPradio network
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryRole {
    /// This node is a broker
    Broker { port: u16 },
    /// This node hosts SDR devices
    DeviceHost { broker_url: Option<String> },
    /// This node is a client (browse only)
    Client,
}

/// Event when a broker is discovered or lost
#[derive(Debug, Clone)]
pub enum BrokerEvent {
    /// A broker was found
    Found {
        name: String,
        host: String,
        port: u16,
        api_version: String,
    },
    /// A broker was lost
    Lost { name: String },
}

/// Event when a device is discovered or lost
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    /// A device was found
    Found {
        serial: DeviceSerial,
        host: String,
        device_type: DeviceType,
        rtl_tcp_port: Option<u16>,
    },
    /// A device was lost
    Lost { serial: DeviceSerial },
    /// A device's info was updated
    Updated {
        serial: DeviceSerial,
        host: String,
        device_type: DeviceType,
    },
}

/// Advertisement info for a device
#[derive(Debug, Clone)]
pub struct DeviceAdvertisement {
    /// Device serial number
    pub serial: DeviceSerial,
    /// Device type
    pub device_type: DeviceType,
    /// rtl_tcp port if running
    pub rtl_tcp_port: Option<u16>,
    /// Frequency range (Hz)
    pub freq_range: Option<(u64, u64)>,
}

/// mDNS discovery service for DSPradio
pub struct DspRadioDiscovery {
    daemon: ServiceDaemon,
    role: DiscoveryRole,
    hostname: String,
}

impl DspRadioDiscovery {
    /// Create a new discovery service
    ///
    /// # Arguments
    ///
    /// * `role` - The role of this node in the network
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dsp_radio_protocol::discovery::{DspRadioDiscovery, DiscoveryRole};
    ///
    /// // Create a client that only browses
    /// let discovery = DspRadioDiscovery::new(DiscoveryRole::Client)?;
    ///
    /// // Create a broker
    /// let discovery = DspRadioDiscovery::new(DiscoveryRole::Broker { port: 8420 })?;
    /// # Ok::<(), dsp_radio_protocol::discovery::DiscoveryError>(())
    /// ```
    pub fn new(role: DiscoveryRole) -> Result<Self, DiscoveryError> {
        let daemon = ServiceDaemon::new().map_err(|e| DiscoveryError::Daemon(e.to_string()))?;

        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        Ok(Self {
            daemon,
            role,
            hostname,
        })
    }

    /// Advertise this node as a broker
    pub fn advertise_broker(&self, port: u16) -> Result<(), DiscoveryError> {
        let instance_name = format!("DSPradio Broker on {}", self.hostname);

        let mut properties = HashMap::new();
        properties.insert("version".to_string(), PROTOCOL_VERSION.to_string());
        properties.insert("role".to_string(), "broker".to_string());
        properties.insert("api_version".to_string(), "v1".to_string());

        let service = ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            &instance_name,
            &self.hostname,
            (),
            port,
            properties,
        )
        .map_err(|e| DiscoveryError::Registration(e.to_string()))?;

        self.daemon
            .register(service)
            .map_err(|e| DiscoveryError::Registration(e.to_string()))?;

        Ok(())
    }

    /// Advertise a device on this host
    pub fn advertise_device(&self, info: &DeviceAdvertisement) -> Result<(), DiscoveryError> {
        let device_type_str = match &info.device_type {
            DeviceType::RtlSdr => "rtlsdr",
            DeviceType::RtlTcp => "rtltcp",
            DeviceType::PlutoSdr => "plutosdr",
            DeviceType::SoapySdr { driver } => driver,
        };

        let instance_name = format!("{} {} on {}", device_type_str, info.serial, self.hostname);

        let mut properties = HashMap::new();
        properties.insert("version".to_string(), PROTOCOL_VERSION.to_string());
        properties.insert("role".to_string(), "device".to_string());
        properties.insert("serial".to_string(), info.serial.to_string());
        properties.insert("type".to_string(), device_type_str.to_string());

        if let Some(port) = info.rtl_tcp_port {
            properties.insert("rtl_tcp_port".to_string(), port.to_string());
        }

        if let Some((min, max)) = info.freq_range {
            properties.insert("freq_min".to_string(), min.to_string());
            properties.insert("freq_max".to_string(), max.to_string());
        }

        if let DiscoveryRole::DeviceHost {
            broker_url: Some(ref url),
        } = self.role
        {
            properties.insert("broker".to_string(), url.clone());
        }

        let port = info.rtl_tcp_port.unwrap_or(0);

        let service = ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            &instance_name,
            &self.hostname,
            (),
            port,
            properties,
        )
        .map_err(|e| DiscoveryError::Registration(e.to_string()))?;

        self.daemon
            .register(service)
            .map_err(|e| DiscoveryError::Registration(e.to_string()))?;

        Ok(())
    }

    /// Remove a device advertisement
    pub fn unadvertise_device(&self, serial: &DeviceSerial) -> Result<(), DiscoveryError> {
        let instance_name = format!("* {} on {}", serial, self.hostname);

        self.daemon
            .unregister(&instance_name)
            .map_err(|e| DiscoveryError::Registration(e.to_string()))?;

        Ok(())
    }

    /// Browse for brokers on the network
    ///
    /// Returns a receiver that yields broker events.
    pub fn browse_brokers(
        &self,
    ) -> Result<mdns_sd::Receiver<ServiceEvent>, DiscoveryError> {
        self.daemon
            .browse(MDNS_SERVICE_TYPE)
            .map_err(|e| DiscoveryError::Daemon(e.to_string()))
    }

    /// Find the first available broker (blocking with timeout)
    pub fn find_first_broker(&self, timeout: Duration) -> Result<(String, u16), DiscoveryError> {
        let receiver = self.browse_brokers()?;

        let deadline = std::time::Instant::now() + timeout;

        while std::time::Instant::now() < deadline {
            if let Ok(event) = receiver.recv_timeout(Duration::from_millis(100)) {
                if let ServiceEvent::ServiceResolved(info) = event {
                    // Check if it's a broker
                    if let Some(role) = info.get_property_val_str("role") {
                        if role == "broker" {
                            let host = info.get_hostname().to_string();
                            let port = info.get_port();
                            return Ok((host, port));
                        }
                    }
                }
            }
        }

        Err(DiscoveryError::Daemon("No broker found within timeout".to_string()))
    }

    /// Parse a service event into a device event
    pub fn parse_device_event(event: &ServiceEvent) -> Option<DeviceEvent> {
        match event {
            ServiceEvent::ServiceResolved(info) => {
                let role = info.get_property_val_str("role")?;
                if role != "device" {
                    return None;
                }

                let serial = DeviceSerial::new(info.get_property_val_str("serial")?);
                let host = info.get_hostname().to_string();
                let type_str = info.get_property_val_str("type")?;

                let device_type = match type_str {
                    "rtlsdr" => DeviceType::RtlSdr,
                    "rtltcp" => DeviceType::RtlTcp,
                    "plutosdr" => DeviceType::PlutoSdr,
                    other => DeviceType::SoapySdr {
                        driver: other.to_string(),
                    },
                };

                let rtl_tcp_port = info
                    .get_property_val_str("rtl_tcp_port")
                    .and_then(|s| s.parse().ok());

                Some(DeviceEvent::Found {
                    serial,
                    host,
                    device_type,
                    rtl_tcp_port,
                })
            }
            ServiceEvent::ServiceRemoved(_, name) => {
                // Try to extract serial from instance name
                // Format: "type serial on hostname"
                let parts: Vec<&str> = name.split_whitespace().collect();
                if parts.len() >= 2 {
                    let serial = DeviceSerial::new(parts[1]);
                    Some(DeviceEvent::Lost { serial })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Parse a service event into a broker event
    pub fn parse_broker_event(event: &ServiceEvent) -> Option<BrokerEvent> {
        match event {
            ServiceEvent::ServiceResolved(info) => {
                let role = info.get_property_val_str("role")?;
                if role != "broker" {
                    return None;
                }

                let name = info.get_fullname().to_string();
                let host = info.get_hostname().to_string();
                let port = info.get_port();
                let api_version = info
                    .get_property_val_str("api_version")
                    .unwrap_or("v1")
                    .to_string();

                Some(BrokerEvent::Found {
                    name,
                    host,
                    port,
                    api_version,
                })
            }
            ServiceEvent::ServiceRemoved(_, name) => Some(BrokerEvent::Lost {
                name: name.to_string(),
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_advertisement() {
        let advert = DeviceAdvertisement {
            serial: DeviceSerial::new("00000001"),
            device_type: DeviceType::RtlSdr,
            rtl_tcp_port: Some(1234),
            freq_range: Some((24_000_000, 1_766_000_000)),
        };

        assert_eq!(advert.serial.as_str(), "00000001");
        assert_eq!(advert.rtl_tcp_port, Some(1234));
    }
}
