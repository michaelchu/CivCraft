use super::*;

impl Simulation {
    pub(in crate::sim) fn ai_ferry_goto_waypoint(
        &mut self,
        id: u32,
        waypoint: (i32, i32),
        destination: (i32, i32),
    ) {
        self.unit_goto_tiles.insert(id, destination);
        let Some(actor) = self.unit_by_id(id).cloned() else {
            return;
        };
        if let Some((path, _)) = self.ai_goto_path(&actor, waypoint, GotoTask::None) {
            self.follow_source_ai_path(id, path);
        }
    }

    pub(in crate::sim) fn wander_unit(&mut self, unit_id: u32) {
        let mut safety = 0;
        while self
            .units
            .iter()
            .find(|u| u.id == unit_id)
            .is_some_and(|u| u.moves_left > 0)
            && safety < 6
        {
            let start = self.rand_below(6) as usize;
            let mut moved = false;
            for offset in 0..6 {
                let (dx, dy) = ISOHEX_DIRS[(start + offset) % 6];
                if self.try_move_unit(unit_id, dx, dy) != MoveResult::Blocked {
                    moved = true;
                    break;
                }
            }
            if !moved {
                break;
            }
            safety += 1;
        }
    }

    pub(in crate::sim) fn run_automated_unit(&mut self, unit_id: u32) {
        let Some(unit) = self.unit_by_id(unit_id).cloned() else {
            return;
        };
        if unit.transport.is_some() {
            return;
        }
        if unit.kind.stats().can_work {
            self.run_auto_worker(unit_id);
            return;
        }
        if matches!(
            self.run_auto_explorer(unit_id),
            exploration::ExploreResult::Done | exploration::ExploreResult::Paused
        ) && let Some(unit) = self.units.iter_mut().find(|u| u.id == unit_id)
        {
            unit.automated = false;
        }
    }
}
