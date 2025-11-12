use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A message pulse transmitted through Hydra channels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pulse {
    /// Unique identifier for this pulse
    pub id: Uuid,
    /// Timestamp when the pulse was created
    pub timestamp: DateTime<Utc>,
    /// Type of the pulse (e.g., "delta", "status", "command")
    #[serde(rename = "type")]
    pub pulse_type: String,
    /// Channel this pulse was sent to
    pub channel: String,
    /// Payload data
    pub data: serde_json::Value,
    /// Optional metadata
    pub metadata: Option<serde_json::Value>,
}

impl Pulse {
    /// Create a new pulse with the given type, channel, and data
    pub fn new(
        pulse_type: impl Into<String>,
        channel: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            pulse_type: pulse_type.into(),
            channel: channel.into(),
            data,
            metadata: None,
        }
    }

    /// Create a new pulse with metadata
    pub fn with_metadata(
        pulse_type: impl Into<String>,
        channel: impl Into<String>,
        data: serde_json::Value,
        metadata: serde_json::Value,
    ) -> Self {
        let mut pulse = Self::new(pulse_type, channel, data);
        pulse.metadata = Some(metadata);
        pulse
    }

    /// Get the pulse type as a string
    pub fn pulse_type(&self) -> &str {
        &self.pulse_type
    }

    /// Validate pulse size for TOON encoding (<1KB payload limit)
    pub fn validate_size(&self) -> anyhow::Result<()> {
        // Serialize as JSON to estimate size
        let json_str = serde_json::to_string(self)?;
        if json_str.len() > 1024 {
            anyhow::bail!("Pulse too large: {} bytes (max 1024)", json_str.len());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_pulse_creation() {
        let pulse = Pulse::new("delta", "repo:stock", json!({"file": "test.rs"}));

        assert_eq!(pulse.pulse_type(), "delta");
        assert_eq!(pulse.channel, "repo:stock");
        assert_eq!(pulse.data, json!({"file": "test.rs"}));
        assert!(pulse.metadata.is_none());
    }

    #[test]
    fn test_pulse_with_metadata() {
        let metadata = json!({"source": "claude-code"});
        let pulse = Pulse::with_metadata(
            "status",
            "agent:heartbeat",
            json!({"status": "active"}),
            metadata.clone(),
        );

        assert_eq!(pulse.pulse_type(), "status");
        assert_eq!(pulse.metadata, Some(metadata));
    }

    #[test]
    fn test_pulse_validation() {
        let small_pulse = Pulse::new("test", "test", json!({"small": "data"}));
        assert!(small_pulse.validate_size().is_ok());

        // Create a large pulse that should fail validation
        let large_data = json!({"large": "x".repeat(2000)});
        let large_pulse = Pulse::new("test", "test", large_data);
        assert!(large_pulse.validate_size().is_err());
    }

    #[test]
    fn test_pulse_serialization() {
        let pulse = Pulse::new("delta", "test", json!({"key": "value"}));
        let serialized = serde_json::to_string(&pulse).unwrap();
        let deserialized: Pulse = serde_json::from_str(&serialized).unwrap();

        assert_eq!(pulse, deserialized);
    }
}