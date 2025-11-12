use anyhow::Result;

/// Message format options for Hydra - now TOON-only
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageFormat {
    /// Token-Oriented Object Notation for efficient AI messaging
    Toon,
}

impl Default for MessageFormat {
    fn default() -> Self {
        MessageFormat::Toon
    }
}

impl std::fmt::Display for MessageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageFormat::Toon => write!(f, "toon"),
        }
    }
}

impl std::str::FromStr for MessageFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "toon" => Ok(MessageFormat::Toon),
            _ => anyhow::bail!("Invalid format: {}. Only toon format is supported", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_format_parsing() {
        assert_eq!("toon".parse::<MessageFormat>().unwrap(), MessageFormat::Toon);
        assert_eq!("TOON".parse::<MessageFormat>().unwrap(), MessageFormat::Toon); // Case insensitive

        assert!("json".parse::<MessageFormat>().is_err());
        assert!("invalid".parse::<MessageFormat>().is_err());
    }

    #[test]
    fn test_message_format_display() {
        assert_eq!(MessageFormat::Toon.to_string(), "toon");
    }

    #[test]
    fn test_message_format_default() {
        assert_eq!(MessageFormat::default(), MessageFormat::Toon);
    }
}