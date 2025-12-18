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
