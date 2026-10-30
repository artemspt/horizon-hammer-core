//! Resource/texture/content change manager.

#[derive(Clone, Debug, Default)]
pub struct ResourceChangeManager {
    pending: Vec<String>,
}

impl ResourceChangeManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn mark(&mut self, path: impl Into<String>) {
        self.pending.push(path.into());
    }
    pub fn pending(&self) -> &[String] {
        &self.pending
    }
    pub fn drain(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending)
    }
    pub fn count(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_change_drain() {
        let mut m = ResourceChangeManager::new();
        m.mark("textures/block/foo.png");
        assert_eq!(m.count(), 1);
        let out = m.drain();
        assert_eq!(out.len(), 1);
        assert_eq!(m.count(), 0);
    }
}
