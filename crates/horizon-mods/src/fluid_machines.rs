//! Fluid machine resolver for tank mixing, bucket conversions and limits.

use crate::FluidInteractionResolver;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FluidTank {
    pub id: String,
    pub capacity: u32,
    pub fluid: Option<(String, u32)>,
}

impl FluidTank {
    pub fn new(fluid: Option<(String, u32)>, capacity: u32) -> Self {
        Self {
            id: "tank".into(),
            fluid,
            capacity,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FluidMachineManager {
    pub interactions: FluidInteractionResolver,
}

impl FluidMachineManager {
    pub fn new() -> Self {
        Self {
            interactions: FluidInteractionResolver::with_defaults(),
        }
    }
    pub fn set_fluid_in_tank(&mut self, tank: &mut FluidTank, requested: &str, amount: u32) -> u32 {
        if let Some((old, _)) = &tank.fluid
            && old != requested
            && let Some(result) = self.interactions.resolve(old, requested)
        {
            let cap = tank.capacity.min(amount);
            tank.fluid = Some((result.to_owned(), cap));
            return cap;
        }
        let held = tank.fluid.as_ref().map(|(_, a)| *a).unwrap_or(0);
        let space = tank.capacity.saturating_sub(held);
        let n = space.min(amount);
        if n > 0 {
            let fluid = if held == 0 {
                requested.to_owned()
            } else {
                tank.fluid
                    .as_ref()
                    .map(|(f, _)| f.clone())
                    .unwrap_or_else(|| requested.to_owned())
            };
            tank.fluid = Some((fluid, held + n));
        }
        match tank.fluid.as_ref() {
            Some((f, _)) if f == requested => n,
            _ => 0,
        }
    }
    pub fn extract(&mut self, tank: &mut FluidTank, amount: u32) -> Option<(String, u32)> {
        let (fluid, held) = tank.fluid.clone()?;
        if held == 0 {
            return None;
        }
        let n = held.min(amount);
        if n == 0 {
            return None;
        }
        if held > n {
            tank.fluid = Some((fluid.clone(), held - n));
        } else {
            tank.fluid = None;
        }
        Some((fluid, n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tank_mix_and_extract() {
        let mut m = FluidMachineManager::new();
        let mut t = FluidTank::new(Some(("minecraft:lava".into(), 8)), 16);
        let r = m.set_fluid_in_tank(&mut t, "minecraft:water", 4);
        assert_eq!(
            t.fluid.as_ref().map(|(f, _)| f.as_str()),
            Some("minecraft:obsidian")
        );
        // conflict results in full controlled conversion response
        let _ = r;
        let e = m.extract(&mut t, 6);
        assert_eq!(e, Some(("minecraft:obsidian".into(), 4)));
    }
}
