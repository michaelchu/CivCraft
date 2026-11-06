//! Reference Amplio2 status artwork, shared by the map's ordinary/focus layers.
use super::*;

pub(super) fn hp_column(hp: i32, maximum: i32) -> i32 {
    10 - (10 * hp.max(0) / maximum.max(1)).clamp(0, 10)
}

pub(super) fn activity_cell(unit: &Unit) -> Option<(i32, i32)> {
    if let Some(work) = unit.work_kind.filter(|_| unit.work_left > 0) {
        return Some(match work {
            WorkKind::Road => (0, 0),
            WorkKind::Railroad => (0, 1),
            WorkKind::BuildBase(Base::Fort) => (1, 0),
            WorkKind::BuildBase(Base::Airbase) => (1, 3),
            WorkKind::BuildBase(Base::Airstrip) => (1, 1),
            WorkKind::BuildBase(Base::Fortress) => (1, 2),
            WorkKind::Cultivate => (3, 0),
            WorkKind::Plant => (3, 1),
            WorkKind::Transform => (3, 2),
            WorkKind::Clean(Contamination::Pollution) => (3, 3),
            WorkKind::Clean(Contamination::Fallout) => (3, 4),
            WorkKind::Pillage(_) => (2, 4),
            WorkKind::Irrigation => (5, 0),
            WorkKind::Farmland => (5, 1),
            WorkKind::Mine => (5, 2),
            WorkKind::OilWell => (5, 3),
            WorkKind::OilPlatform => (5, 4),
        });
    }
    match unit.state {
        UnitState::Fortifying => Some((2, 1)),
        UnitState::Fortified => Some((2, 0)),
        UnitState::Sentry => Some((2, 2)),
        UnitState::Idle if unit.automated && !unit.kind.stats().can_work => Some((4, 2)),
        UnitState::Idle => None,
    }
}

impl Game {
    pub(super) fn draw_city_overlay(&self, assets: &Assets, center: Vec2, zoom: f32, col: i32) {
        draw_texture_ex(
            &assets.bases,
            center.x - 34.5 * zoom,
            center.y - 43.0 * zoom,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(69.0 * zoom, 52.0 * zoom)),
                source: Some(bordered_sprite_source(0, col, 96.0, 72.0)),
                ..Default::default()
            },
        );
    }

    pub(super) fn draw_unit_status(&self, assets: &Assets, unit: &Unit, center: Vec2) {
        let z = self.zoom;
        // The original sheets include transparent margins. Position the HP
        // bar above the unit and the activity/rank glyphs beside its shoulders.
        let col = hp_column(unit.hp, unit.kind.stats().hp);
        draw_texture_ex(
            &assets.status,
            center.x - 44.0 * z,
            center.y - 47.0 * z,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(96.0 * z, 48.0 * z)),
                source: Some(bordered_sprite_source(0, col, 96.0, 48.0)),
                ..Default::default()
            },
        );
        if let Some((row, col)) = activity_cell(unit) {
            draw_texture_ex(
                &assets.activities,
                center.x + 14.0 * z,
                center.y - 39.0 * z,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(21.0 * z, 18.0 * z)),
                    source: Some(bordered_sprite_source(row, col, 28.0, 24.0)),
                    ..Default::default()
                },
            );
        }
        if unit.automated && unit.kind.stats().can_work {
            draw_texture_ex(
                &assets.status,
                center.x - 48.0 * z,
                center.y - 35.0 * z,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(96.0 * z, 48.0 * z)),
                    source: Some(bordered_sprite_source(4, 10, 96.0, 48.0)),
                    ..Default::default()
                },
            );
        }
        if unit.veteran > 0 {
            draw_texture_ex(
                &assets.veterancy,
                center.x - 32.0 * z,
                center.y - 44.0 * z,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(64.0 * z, 48.0 * z)),
                    // Veterancy's grid starts at 0, unlike the other sheets.
                    source: Some(Rect::new(
                        (unit.veteran.min(9) - 1) as f32 * 65.0,
                        0.0,
                        64.0,
                        48.0,
                    )),
                    ..Default::default()
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn health_uses_reference_floor_deciles_including_zero() {
        assert_eq!(hp_column(20, 20), 0);
        assert_eq!(hp_column(19, 20), 1);
        assert_eq!(hp_column(1, 20), 10);
        assert_eq!(hp_column(0, 20), 10);
        assert_eq!(hp_column(5, 10), 5);
    }
    #[test]
    fn activity_glyphs_resolve_to_the_reference_semantic_tags() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        let mut unit = sim.units()[0].clone();
        unit.kind = UnitKind::Engineer;
        unit.work_left = 1;
        let spec = include_str!("../../assets/amplio2_activities.spec");
        for (work, tag) in [
            (WorkKind::Road, "unit.road"),
            (WorkKind::Railroad, "unit.rail"),
            (WorkKind::Irrigation, "unit.irrigation"),
            (WorkKind::Farmland, "unit.farmland"),
            (WorkKind::Mine, "unit.mine"),
            (WorkKind::OilWell, "unit.oil_mine"),
            (WorkKind::OilPlatform, "unit.oil_rig"),
            (WorkKind::Cultivate, "unit.cultivate"),
            (WorkKind::Plant, "unit.plant"),
            (WorkKind::Transform, "unit.transform"),
            (WorkKind::Clean(Contamination::Pollution), "unit.pollution"),
            (WorkKind::Clean(Contamination::Fallout), "unit.fallout"),
            (WorkKind::BuildBase(Base::Fort), "unit.outpost"),
            (WorkKind::BuildBase(Base::Fortress), "unit.fortress"),
            (WorkKind::BuildBase(Base::Airstrip), "unit.airstrip"),
            (WorkKind::BuildBase(Base::Airbase), "unit.airbase"),
            (WorkKind::Pillage(PillageTarget::Road), "unit.pillage"),
        ] {
            unit.work_kind = Some(work);
            let line = spec
                .lines()
                .find(|line| line.contains(&format!("\"{tag}\"")))
                .unwrap();
            let fields: Vec<_> = line.split(',').map(str::trim).collect();
            let expected = (fields[0].parse().unwrap(), fields[1].parse().unwrap());
            assert_eq!(activity_cell(&unit), Some(expected), "{tag}");
        }
    }
}
