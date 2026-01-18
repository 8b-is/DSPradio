//! Priority levels for SDR device access
//!
//! The priority system enables preemption - higher priority clients can
//! take devices from lower priority ones with a graceful handoff period.

use serde::{Deserialize, Serialize};

/// Priority levels for device access
///
/// Higher priority clients can preempt lower priority ones. The handoff
/// timeout gives the current user time to checkpoint their state.
///
/// # Priority Levels
///
/// - **Low**: Background tasks, experiments, testing
/// - **Normal**: Default for general applications
/// - **High**: Time-sensitive tasks, live decoding
/// - **Critical**: Safety-critical (ADS-B, emergency systems, security alerts)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// Background tasks - lowest priority, longest handoff time
    Low = 0,
    /// Default priority for general applications
    Normal = 1,
    /// Time-sensitive tasks requiring faster access
    High = 2,
    /// Safety-critical systems - immediate or near-immediate preemption
    Critical = 3,
}

impl Priority {
    /// Get the handoff timeout in milliseconds for this priority level
    ///
    /// When a higher-priority client requests a device, the current holder
    /// gets this much time to save state before being preempted.
    ///
    /// # Returns
    ///
    /// Timeout in milliseconds. Lower priority = more time to checkpoint.
    pub fn handoff_timeout_ms(&self) -> u64 {
        match self {
            Priority::Critical => 0,      // Immediate for security alerts
            Priority::High => 2000,       // 2 seconds
            Priority::Normal => 5000,     // 5 seconds
            Priority::Low => 10000,       // 10 seconds for background tasks
        }
    }

    /// Check if this priority can preempt another
    ///
    /// A priority can preempt another only if it's strictly higher.
    pub fn can_preempt(&self, other: &Priority) -> bool {
        self > other
    }

    /// Get a human-readable description of this priority level
    pub fn description(&self) -> &'static str {
        match self {
            Priority::Low => "Background tasks, experiments",
            Priority::Normal => "General applications",
            Priority::High => "Time-sensitive, live decoding",
            Priority::Critical => "Safety-critical, security alerts",
        }
    }
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::Low => write!(f, "low"),
            Priority::Normal => write!(f, "normal"),
            Priority::High => write!(f, "high"),
            Priority::Critical => write!(f, "critical"),
        }
    }
}

impl std::str::FromStr for Priority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" | "0" => Ok(Priority::Low),
            "normal" | "1" => Ok(Priority::Normal),
            "high" | "2" => Ok(Priority::High),
            "critical" | "3" => Ok(Priority::Critical),
            _ => Err(format!("Invalid priority: '{}'. Use: low, normal, high, critical", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
    }

    #[test]
    fn test_can_preempt() {
        assert!(Priority::Critical.can_preempt(&Priority::High));
        assert!(Priority::High.can_preempt(&Priority::Normal));
        assert!(!Priority::Normal.can_preempt(&Priority::High));
        assert!(!Priority::Normal.can_preempt(&Priority::Normal)); // Same priority can't preempt
    }

    #[test]
    fn test_priority_serialization() {
        assert_eq!(serde_json::to_string(&Priority::Low).unwrap(), r#""low""#);
        assert_eq!(serde_json::to_string(&Priority::Critical).unwrap(), r#""critical""#);
    }

    #[test]
    fn test_priority_from_str() {
        assert_eq!("low".parse::<Priority>().unwrap(), Priority::Low);
        assert_eq!("HIGH".parse::<Priority>().unwrap(), Priority::High);
        assert_eq!("2".parse::<Priority>().unwrap(), Priority::High);
    }
}
