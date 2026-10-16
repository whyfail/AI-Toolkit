use anyhow::{bail, Context, Result};
use std::time::{Duration, Instant};

pub struct ResourceBudget {
    entries_left: u64,
    bytes_left: u64,
    max_depth: usize,
    deadline: Instant,
}

impl ResourceBudget {
    pub fn new() -> Result<Self> {
        fn limit(key: &str, default: u64) -> Result<u64> {
            let value = std::env::var(key)
                .ok()
                .map(|v| v.parse::<u64>())
                .transpose()
                .with_context(|| format!("资源限制 {key} 无效"))?
                .unwrap_or(default);
            if value == 0 {
                bail!("资源限制 {key} 必须大于 0");
            }
            Ok(value)
        }
        Ok(Self {
            entries_left: limit("AI_TOOLKIT_SKILL_MAX_ENTRIES", 100_000)?,
            bytes_left: limit("AI_TOOLKIT_SKILL_MAX_BYTES", 2 * 1024 * 1024 * 1024)?,
            max_depth: limit("AI_TOOLKIT_SKILL_MAX_DEPTH", 128)? as usize,
            deadline: Instant::now()
                + Duration::from_secs(limit("AI_TOOLKIT_SKILL_TIMEOUT_SECS", 300)?.min(86400)),
        })
    }

    pub fn enter(&mut self, depth: usize) -> Result<()> {
        self.check()?;
        if depth > self.max_depth || self.entries_left == 0 {
            bail!("技能目录深度或文件数量超过限制");
        }
        self.entries_left -= 1;
        Ok(())
    }

    pub fn bytes(&mut self, count: u64) -> Result<()> {
        self.check()?;
        self.bytes_left = self
            .bytes_left
            .checked_sub(count)
            .ok_or_else(|| anyhow::anyhow!("技能总字节数超过限制"))?;
        Ok(())
    }

    pub fn check(&self) -> Result<()> {
        if Instant::now() >= self.deadline {
            bail!("技能处理超过时间限制");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_aggregate_limits() {
        let mut budget = ResourceBudget {
            entries_left: 1,
            bytes_left: 4,
            max_depth: 2,
            deadline: Instant::now() + Duration::from_secs(1),
        };
        assert!(budget.enter(3).is_err());
        budget.enter(1).unwrap();
        assert!(budget.enter(1).is_err());
        budget.bytes(3).unwrap();
        assert!(budget.bytes(2).is_err());
    }
}
