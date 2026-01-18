//! Core broker logic
//!
//! The Broker coordinates device registration, lease management, and
//! handles preemption when higher-priority clients need devices.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use dsp_radio_protocol::{
    discovery::{DeviceAdvertisement, DiscoveryRole, DspRadioDiscovery},
    DeviceInfo, DeviceSerial, DeviceStatus, LeaseId, Priority,
};

use crate::device_registry::DeviceRegistry;
use crate::lease::LeaseManager;

/// The central broker that manages SDR resources
pub struct Broker {
    /// Device registry
    pub devices: Arc<RwLock<DeviceRegistry>>,
    /// Lease manager
    pub leases: Arc<RwLock<LeaseManager>>,
    /// mDNS discovery service
    discovery: Option<DspRadioDiscovery>,
}

impl Broker {
    /// Create a new broker instance
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            devices: Arc::new(RwLock::new(DeviceRegistry::new())),
            leases: Arc::new(RwLock::new(LeaseManager::new())),
            discovery: None,
        })
    }

    /// Start mDNS advertisement
    pub fn start_mdns(&self, port: u16) -> Result<(), String> {
        let discovery = DspRadioDiscovery::new(DiscoveryRole::Broker { port })
            .map_err(|e| e.to_string())?;

        discovery.advertise_broker(port).map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Register a device with the broker
    pub async fn register_device(&self, info: DeviceInfo) -> Result<(), String> {
        let mut devices = self.devices.write().await;
        devices.register(info);
        Ok(())
    }

    /// Unregister a device
    pub async fn unregister_device(&self, serial: &DeviceSerial) -> Result<(), String> {
        let mut devices = self.devices.write().await;
        devices.unregister(serial);
        Ok(())
    }

    /// List all devices
    pub async fn list_devices(&self) -> Vec<DeviceInfo> {
        let devices = self.devices.read().await;
        devices.list_all()
    }

    /// Get a specific device
    pub async fn get_device(&self, serial: &DeviceSerial) -> Option<DeviceInfo> {
        let devices = self.devices.read().await;
        devices.get(serial)
    }

    /// Request a lease for a device
    pub async fn request_lease(
        &self,
        device_serial: DeviceSerial,
        client_id: uuid::Uuid,
        priority: Priority,
        config: dsp_radio_protocol::LeaseConfig,
        checkpoint_callback: Option<String>,
    ) -> Result<dsp_radio_protocol::LeaseResponse, String> {
        let devices = self.devices.read().await;
        let device = devices.get(&device_serial).ok_or("Device not found")?;

        match &device.status {
            DeviceStatus::Available => {
                drop(devices); // Release read lock

                let mut leases = self.leases.write().await;
                let lease = leases.create_lease(
                    device_serial.clone(),
                    client_id,
                    priority,
                    config,
                    checkpoint_callback,
                );

                // Update device status
                let mut devices = self.devices.write().await;
                devices.set_status(
                    &device_serial,
                    DeviceStatus::InUse {
                        lease_id: lease.lease_id,
                        client_id,
                        priority,
                    },
                );

                Ok(dsp_radio_protocol::LeaseResponse {
                    lease_id: lease.lease_id,
                    device_serial,
                    stream_url: format!("/api/v1/stream/{}", lease.lease_id),
                    state: dsp_radio_protocol::LeaseState::Active,
                    expires_at_ms: None,
                })
            }
            DeviceStatus::InUse {
                lease_id: current_lease,
                priority: current_priority,
                ..
            } => {
                // Check if we can preempt
                if priority.can_preempt(current_priority) {
                    // TODO: Initiate preemption
                    Err("Preemption not yet implemented".to_string())
                } else {
                    Err(format!(
                        "Device in use by higher/equal priority client ({})",
                        current_priority
                    ))
                }
            }
            DeviceStatus::Preempting { .. } => {
                Err("Device is being preempted, try again shortly".to_string())
            }
            DeviceStatus::Offline { reason, .. } => {
                Err(format!("Device offline: {}", reason.as_deref().unwrap_or("unknown")))
            }
        }
    }

    /// Release a lease
    pub async fn release_lease(&self, lease_id: LeaseId) -> Result<(), String> {
        let mut leases = self.leases.write().await;

        if let Some(lease) = leases.get(&lease_id) {
            let device_serial = lease.device_serial.clone();
            leases.release(lease_id);

            // Update device status
            let mut devices = self.devices.write().await;
            devices.set_status(&device_serial, DeviceStatus::Available);

            Ok(())
        } else {
            Err("Lease not found".to_string())
        }
    }
}

impl Default for Broker {
    fn default() -> Self {
        Self {
            devices: Arc::new(RwLock::new(DeviceRegistry::new())),
            leases: Arc::new(RwLock::new(LeaseManager::new())),
            discovery: None,
        }
    }
}
