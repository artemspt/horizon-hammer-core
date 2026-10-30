//! AI goal manager/resolver, a NeoForge-like ordered set of goals.

use crate::parity_models::AIGoalDef;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct AiGoalManager {
    goals: BTreeMap<String, Vec<AIGoalDef>>, // entity type -> goal defs
}

impl AiGoalManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, entity: impl Into<String>, goal: AIGoalDef) {
        self.goals.entry(entity.into()).or_default().push(goal);
    }
    pub fn goals_for(&self, entity: &str) -> Vec<AIGoalDef> {
        let mut g = self.goals.get(entity).cloned().unwrap_or_default();
        g.sort_by_key(|x| x.priority);
        g
    }
    pub fn top_goal(&self, entity: &str) -> Option<AIGoalDef> {
        self.goals_for(entity).into_iter().next()
    }
    pub fn remove_goal(&mut self, entity: &str, id: &str) {
        if let Some(goals) = self.goals.get_mut(entity) {
            goals.retain(|g| g.id.to_string() != id);
        }
    }
    pub fn remove_entity(&mut self, entity: &str) {
        self.goals.remove(entity);
    }
    pub fn entity_count(&self) -> usize {
        self.goals.len()
    }
    pub fn goal_count(&self) -> usize {
        self.goals.values().map(|v| v.len()).sum()
    }
    pub fn has_goal(&self, entity: &str, id: &str) -> bool {
        self.goals
            .get(entity)
            .map(|g| g.iter().any(|x| x.id.to_string() == id))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceLocation;
    #[test]
    fn ai_goals_sorted_and_selected() {
        let mut m = AiGoalManager::new();
        m.add(
            "zombie",
            AIGoalDef::new(ResourceLocation::parse("mod:attack").unwrap(), 2),
        );
        m.add(
            "zombie",
            AIGoalDef::new(ResourceLocation::parse("mod:wander").unwrap(), 5),
        );
        let first = m.top_goal("zombie").unwrap();
        assert_eq!(first.priority, 2);
        assert_eq!(m.goal_count(), 2);
        m.remove_goal(
            "zombie",
            &ResourceLocation::parse("mod:attack").unwrap().to_string(),
        );
        assert_eq!(m.goal_count(), 1);
        assert!(!m.has_goal(
            "zombie",
            &ResourceLocation::parse("mod:attack").unwrap().to_string()
        ));
    }
}
