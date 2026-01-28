use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PortRegistry {
    #[serde(flatten)]
    pub allocations: HashMap<String, u16>,
}

impl PortRegistry {
    pub fn path() -> PathBuf {
        PathBuf::from(".hydra/wt-ports.json")
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let registry: PortRegistry = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(registry)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize port registry")?;
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        Ok(())
    }

    pub fn allocate(&mut self, branch: &str, range_start: u16, range_end: u16) -> Result<u16> {
        if let Some(&port) = self.allocations.get(branch) {
            bail!("Branch '{}' already has port {} allocated", branch, port);
        }

        let used_ports: std::collections::HashSet<u16> =
            self.allocations.values().copied().collect();

        for port in range_start..=range_end {
            if !used_ports.contains(&port) {
                self.allocations.insert(branch.to_string(), port);
                return Ok(port);
            }
        }

        bail!("No free ports in range {}-{}", range_start, range_end);
    }

    pub fn free(&mut self, branch: &str) -> Result<u16> {
        self.allocations
            .remove(branch)
            .ok_or_else(|| anyhow::anyhow!("No port allocated for branch '{}'", branch))
    }

    pub fn get(&self, branch: &str) -> Option<u16> {
        self.allocations.get(branch).copied()
    }

    pub fn list(&self) -> impl Iterator<Item = (&String, &u16)> {
        self.allocations.iter()
    }

    pub fn init() -> Result<()> {
        let path = Self::path();
        if path.exists() {
            return Ok(());
        }
        let registry = Self::default();
        registry.save()?;
        println!("Created {}", path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_port() {
        let mut registry = PortRegistry::default();
        let port = registry.allocate("feature-a", 3000, 3010).unwrap();
        assert_eq!(port, 3000);
        assert_eq!(registry.get("feature-a"), Some(3000));
    }

    #[test]
    fn test_allocate_multiple_ports() {
        let mut registry = PortRegistry::default();
        let port1 = registry.allocate("feature-a", 3000, 3010).unwrap();
        let port2 = registry.allocate("feature-b", 3000, 3010).unwrap();
        assert_eq!(port1, 3000);
        assert_eq!(port2, 3001);
        assert_ne!(port1, port2);
    }

    #[test]
    fn test_allocate_duplicate_branch_fails() {
        let mut registry = PortRegistry::default();
        registry.allocate("feature-a", 3000, 3010).unwrap();
        let result = registry.allocate("feature-a", 3000, 3010);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already has port"));
    }

    #[test]
    fn test_allocate_range_exhausted() {
        let mut registry = PortRegistry::default();
        registry.allocate("feature-a", 3000, 3001).unwrap();
        registry.allocate("feature-b", 3000, 3001).unwrap();
        let result = registry.allocate("feature-c", 3000, 3001);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No free ports"));
    }

    #[test]
    fn test_free_port() {
        let mut registry = PortRegistry::default();
        registry.allocate("feature-a", 3000, 3010).unwrap();
        let freed = registry.free("feature-a").unwrap();
        assert_eq!(freed, 3000);
        assert_eq!(registry.get("feature-a"), None);
    }

    #[test]
    fn test_free_nonexistent_branch_fails() {
        let mut registry = PortRegistry::default();
        let result = registry.free("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No port allocated"));
    }

    #[test]
    fn test_get_port() {
        let mut registry = PortRegistry::default();
        assert_eq!(registry.get("feature-a"), None);
        registry.allocate("feature-a", 3000, 3010).unwrap();
        assert_eq!(registry.get("feature-a"), Some(3000));
    }

    #[test]
    fn test_list_ports() {
        let mut registry = PortRegistry::default();
        registry.allocate("feature-a", 3000, 3010).unwrap();
        registry.allocate("feature-b", 3000, 3010).unwrap();

        let ports: HashMap<String, u16> = registry.list()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        assert_eq!(ports.len(), 2);
        assert!(ports.contains_key("feature-a"));
        assert!(ports.contains_key("feature-b"));
    }

    #[test]
    fn test_reuse_freed_port() {
        let mut registry = PortRegistry::default();
        registry.allocate("feature-a", 3000, 3010).unwrap();
        registry.free("feature-a").unwrap();
        let port = registry.allocate("feature-b", 3000, 3010).unwrap();
        assert_eq!(port, 3000);
    }

    #[test]
    fn test_invalid_port_range() {
        let mut registry = PortRegistry::default();
        // Range where start > end
        let result = registry.allocate("feature-a", 3010, 3000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No free ports"));
    }

    #[test]
    fn test_single_port_range() {
        let mut registry = PortRegistry::default();
        let port = registry.allocate("feature-a", 3000, 3000).unwrap();
        assert_eq!(port, 3000);

        // Second allocation should fail
        let result = registry.allocate("feature-b", 3000, 3000);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize_deserialize_registry() {
        let mut registry = PortRegistry::default();
        registry.allocate("feature-a", 3000, 3010).unwrap();
        registry.allocate("feature-b", 3000, 3010).unwrap();

        let json = serde_json::to_string(&registry).unwrap();
        let deserialized: PortRegistry = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.get("feature-a"), Some(3000));
        assert_eq!(deserialized.get("feature-b"), Some(3001));
    }

    #[test]
    fn test_deserialize_invalid_json() {
        let invalid_json = r#"{"feature-a": "not a number"}"#;
        let result: Result<PortRegistry, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_allocate_with_zero_port() {
        let mut registry = PortRegistry::default();
        // Port 0 is technically valid but unusual
        let port = registry.allocate("feature-a", 0, 10).unwrap();
        assert_eq!(port, 0);
    }

    #[test]
    fn test_free_twice_fails() {
        let mut registry = PortRegistry::default();
        registry.allocate("feature-a", 3000, 3010).unwrap();
        registry.free("feature-a").unwrap();

        // Second free should fail
        let result = registry.free("feature-a");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No port allocated"));
    }
}
