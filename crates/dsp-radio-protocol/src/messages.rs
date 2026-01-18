//! API message types for broker communication
//!
//! These types define the REST API request/response structures and
//! WebSocket message formats for the broker daemon.

use serde::{Deserialize, Serialize};

use crate::{ClientId, DeviceSerial, Gain, LeaseId, Priority};

/// Device type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    /// RTL-SDR (local USB)
    RtlSdr,
    /// Remote RTL-SDR via rtl_tcp
    RtlTcp,
    /// PlutoSDR
    PlutoSdr,
    /// SoapySDR-compatible device
    SoapySdr { driver: String },
}

/// Device capabilities and specifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// Frequency range in Hz (min, max)
    pub freq_range: (u64, u64),
    /// Available sample rates in Hz
    pub sample_rates: Vec<u32>,
    /// Gain range in dB (min, max)
    pub gain_range: (f64, f64),
    /// Whether bias tee is available
    pub has_bias_tee: bool,
    /// Tuner type (for RTL-SDR)
    pub tuner_type: Option<String>,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            freq_range: (24_000_000, 1_766_000_000), // Typical RTL-SDR range
            sample_rates: vec![240_000, 1_024_000, 2_048_000, 2_400_000],
            gain_range: (0.0, 49.6),
            has_bias_tee: false,
            tuner_type: None,
        }
    }
}

/// Current device status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DeviceStatus {
    /// Device is available for lease
    Available,
    /// Device is in use
    InUse {
        lease_id: LeaseId,
        client_id: ClientId,
        priority: Priority,
    },
    /// Device is being preempted (handoff in progress)
    Preempting {
        from_lease: LeaseId,
        to_lease: LeaseId,
        deadline_ms: u64,
    },
    /// Device is offline or unreachable
    Offline {
        since_epoch_ms: u64,
        reason: Option<String>,
    },
}

/// Device information returned by the broker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Unique device serial
    pub serial: DeviceSerial,
    /// Device type
    pub device_type: DeviceType,
    /// Current status
    pub status: DeviceStatus,
    /// Device capabilities
    pub capabilities: DeviceCapabilities,
    /// Host where device is connected (for remote devices)
    pub host: Option<String>,
    /// rtl_tcp port if available
    pub rtl_tcp_port: Option<u16>,
    /// Last seen timestamp (epoch ms)
    pub last_seen_ms: u64,
}

/// Configuration for a lease request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseConfig {
    /// Center frequency in Hz
    pub center_freq: u32,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Gain setting
    pub gain: Gain,
    /// Enable bias tee
    #[serde(default)]
    pub bias_tee: bool,
    /// PPM correction
    #[serde(default)]
    pub ppm_correction: i32,
}

/// Request to acquire a device lease
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseRequest {
    /// Device to lease (by serial)
    pub device_serial: DeviceSerial,
    /// Requested priority
    pub priority: Priority,
    /// Device configuration
    pub config: LeaseConfig,
    /// Optional callback URL for preemption notifications
    pub checkpoint_callback: Option<String>,
}

/// Current state of a lease
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LeaseState {
    /// Lease is active
    Active,
    /// Preemption warning issued
    PreemptionWarning { deadline_ms: u64, reason: String },
    /// Waiting for checkpoint acknowledgment
    CheckpointPending,
    /// Lease was released by client
    Released,
    /// Lease was revoked (preempted or error)
    Revoked { reason: String },
}

/// Response when a lease is granted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseResponse {
    /// Unique lease ID
    pub lease_id: LeaseId,
    /// Device serial
    pub device_serial: DeviceSerial,
    /// WebSocket URL for I/Q streaming
    pub stream_url: String,
    /// Current lease state
    pub state: LeaseState,
    /// Expiration time (if set)
    pub expires_at_ms: Option<u64>,
}

/// WebSocket control messages (client -> broker)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Set center frequency
    SetFrequency { freq_hz: u32 },
    /// Set sample rate
    SetSampleRate { rate_hz: u32 },
    /// Set gain
    SetGain { gain: Gain },
    /// Enable/disable bias tee
    SetBiasTee { enabled: bool },
    /// Acknowledge checkpoint complete
    CheckpointAck,
    /// Ping for keepalive
    Ping,
}

/// WebSocket control messages (broker -> client)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrokerMessage {
    /// Preemption warning - client should checkpoint
    PreemptWarning {
        deadline_ms: u64,
        reason: String,
        preemptor_priority: Priority,
    },
    /// Lease has been revoked
    LeaseRevoked { reason: String },
    /// Device parameters updated (confirmation)
    DeviceParams {
        center_freq: u32,
        sample_rate: u32,
        gain: Gain,
    },
    /// Error occurred
    Error { code: String, message: String },
    /// Pong response
    Pong,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lease_request_serialization() {
        let request = LeaseRequest {
            device_serial: DeviceSerial::new("ABC123"),
            priority: Priority::Normal,
            config: LeaseConfig {
                center_freq: 1_090_000_000,
                sample_rate: 2_400_000,
                gain: Gain::Auto,
                bias_tee: false,
                ppm_correction: 0,
            },
            checkpoint_callback: Some("http://localhost:8080/checkpoint".to_string()),
        };

        let json = serde_json::to_string_pretty(&request).unwrap();
        assert!(json.contains("ABC123"));
        assert!(json.contains("normal"));
    }

    #[test]
    fn test_device_status_serialization() {
        let status = DeviceStatus::Available;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#"{"status":"available"}"#);

        let in_use = DeviceStatus::InUse {
            lease_id: uuid::Uuid::nil(),
            client_id: uuid::Uuid::nil(),
            priority: Priority::High,
        };
        let json = serde_json::to_string(&in_use).unwrap();
        assert!(json.contains("in_use"));
        assert!(json.contains("high"));
    }

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage::SetFrequency { freq_hz: 1_090_000_000 };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("set_frequency"));
        assert!(json.contains("1090000000"));
    }
}
