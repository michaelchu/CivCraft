mod assets;
pub(crate) use assets::{Assets, window_conf};
use assets::{
    OCEAN_ATLAS_H, PLAYER_COLORS, TILE_SPRITE_H, TILE_SPRITE_W, bordered_sprite_source,
    ocean_cell_source, unit_sprite,
};
mod hud;
mod input;
mod research_tree;
mod research_view;
mod world;

use crate::rules::{Building, Genus, Requirement, Tech};
use crate::sim::{topology::*, *};
use macroquad::prelude::*;
use research_tree::{NODE_SIZE, RESEARCH_AGES, ResearchTree};

const TILE_W: f32 = 96.0;
const TILE_H: f32 = 48.0;
const PANEL_W: f32 = 316.0;
const TOP_H: f32 = 46.0;
const BOTTOM_H: f32 = 92.0;
const START_ZOOM: f32 = 0.82;
const MIN_ZOOM: f32 = 0.55;
const MAX_ZOOM: f32 = 1.35;

#[derive(Clone, Copy)]
pub(crate) enum UiAction {
    EndTurn,
    RaiseRate(usize),
    CloseCity,
    SelectUnit(u32),
    FoundCity,
    Road,
    Irrigate,
    Mine,
    Fortify,
    Wait,
    Automate,
    Produce(Production),
    ProductionPage(usize),
    OpenResearch,
    CloseResearch,
    CenterResearch,
    FitResearch,
    ResearchAge(usize),
    InspectResearch(Tech),
    ResearchGoal(Tech),
}

pub(crate) struct Game {
    pub(crate) sim: Simulation,
    pub(crate) selected_unit: Option<u32>,
    selected_city: Option<u32>,
    production_page: usize,
    camera: Vec2,
    zoom: f32,
    goto_mode: bool,
    research_open: bool,
    research_age: usize,
    research_inspected: Option<Tech>,
    research_tree: ResearchTree,
    research_center: Vec2,
    research_zoom: f32,
    research_drag: Option<Vec2>,
    pub(crate) hud_elements_drawn: usize,
    dragging: bool,
    drag_last: Vec2,
}

impl Game {
    pub(crate) fn new(map: crate::mapgen::GeneratedMap) -> Self {
        let sim = Simulation::new(map);
        let settler = sim
            .units()
            .iter()
            .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settlers)
            .unwrap();
        let selected_unit = Some(settler.id);
        let (x, y) = native_to_map(settler.x, settler.y);
        Self {
            sim,
            selected_unit,
            selected_city: None,
            production_page: 0,
            camera: vec2(x as f32, y as f32),
            zoom: START_ZOOM,
            goto_mode: false,
            research_open: false,
            research_age: 0,
            research_inspected: None,
            research_tree: ResearchTree::new(),
            research_center: Vec2::ZERO,
            research_zoom: 1.0,
            research_drag: None,
            hud_elements_drawn: 0,
            dragging: false,
            drag_last: vec2(0.0, 0.0),
        }
    }

    fn found_city(&mut self, id: u32) -> bool {
        if self.sim.apply(Command::FoundCity(id)) {
            self.selected_unit = None;
            self.production_page = 0;
            self.selected_city = self.sim.cities().last().map(|city| city.id);
            true
        } else {
            false
        }
    }

    fn move_selected(&mut self, command: Command, id: u32) -> bool {
        let previous = self
            .sim
            .units()
            .iter()
            .find(|u| u.id == id)
            .map(|u| (u.x, u.y));
        if !self.sim.apply(command) {
            return false;
        }
        if let Some((x, y)) = self
            .sim
            .units()
            .iter()
            .find(|u| u.id == id)
            .map(|u| (u.x, u.y))
        {
            if Some((x, y)) != previous {
                self.focus_camera_on(x, y);
            }
        } else if self.selected_unit == Some(id) {
            self.selected_unit = None;
        }
        true
    }

    fn selected_unit_index(&self) -> Option<usize> {
        let id = self.selected_unit?;
        self.sim.units().iter().position(|u| u.id == id)
    }

    fn selected_city_index(&self) -> Option<usize> {
        let id = self.selected_city?;
        self.sim.cities().iter().position(|c| c.id == id)
    }

    fn select_next_unit(&mut self) {
        let current = self.selected_unit.unwrap_or(0);
        let mut candidates: Vec<u32> = self
            .sim
            .units()
            .iter()
            .filter(|u| {
                u.owner == HUMAN
                    && u.moves_left > 0
                    && u.work_remaining == 0
                    && u.state != UnitState::Sentry
            })
            .map(|u| u.id)
            .collect();
        candidates.sort_unstable();
        let next = candidates
            .iter()
            .copied()
            .find(|id| *id > current)
            .or_else(|| candidates.first().copied());
        self.selected_unit = next;
        self.selected_city = None;
        if let Some(id) = next {
            if let Some((x, y)) = self
                .sim
                .units()
                .iter()
                .find(|u| u.id == id)
                .map(|unit| (unit.x, unit.y))
            {
                self.focus_camera_on(x, y);
            }
        }
    }
}

impl Game {
    pub(crate) fn draw(&mut self, assets: &Assets) -> Vec<(Rect, UiAction)> {
        clear_background(Color::from_rgba(7, 20, 31, 255));
        self.hud_elements_drawn = 0;
        if self.research_open {
            return self.draw_research();
        }
        self.draw_world(assets);
        let mut actions = self.draw_hud();
        if self.selected_city_index().is_some() {
            self.draw_city_manager(assets, &mut actions);
        }
        if self.goto_mode {
            let viewport = Self::viewport_rect();
            let text = "GO-TO: choose a destination";
            let width = measure_text(text, None, 22, 1.0).width;
            draw_rectangle(
                viewport.x + viewport.w * 0.5 - width * 0.5 - 14.0,
                viewport.y + 14.0,
                width + 28.0,
                34.0,
                Color::from_rgba(25, 23, 18, 230),
            );
            draw_rectangle_lines(
                viewport.x + viewport.w * 0.5 - width * 0.5 - 14.0,
                viewport.y + 14.0,
                width + 28.0,
                34.0,
                2.0,
                Color::from_rgba(237, 203, 89, 255),
            );
            draw_text(
                text,
                viewport.x + viewport.w * 0.5 - width * 0.5,
                viewport.y + 38.0,
                22.0,
                Color::from_rgba(255, 237, 170, 255),
            );
        }
        if let Some(message) = self.sim.game_over() {
            let w = 540.0_f32.min(screen_width() - 80.0);
            let h = 174.0;
            let x = (screen_width() - w) * 0.5;
            let y = (screen_height() - h) * 0.5;
            draw_rectangle(x - 5.0, y - 5.0, w + 10.0, h + 10.0, BLACK);
            draw_rectangle(x, y, w, h, Color::from_rgba(42, 39, 31, 248));
            draw_rectangle_lines(x, y, w, h, 3.0, Color::from_rgba(224, 188, 82, 255));
            centered_text(
                "SKIRMISH COMPLETE",
                y + 42.0,
                28.0,
                Color::from_rgba(250, 222, 124, 255),
            );
            centered_text(
                message,
                y + 88.0,
                21.0,
                Color::from_rgba(238, 234, 218, 255),
            );
            centered_text(
                "The chronicle of this world is now complete.",
                y + 127.0,
                17.0,
                Color::from_rgba(176, 180, 169, 255),
            );
        }
        actions
    }
}

fn draw_diamond(center: Vec2, width: f32, height: f32, color: Color) {
    let top = vec2(center.x, center.y - height * 0.5);
    let right = vec2(center.x + width * 0.5, center.y);
    let bottom = vec2(center.x, center.y + height * 0.5);
    let left = vec2(center.x - width * 0.5, center.y);
    draw_triangle(top, right, bottom, color);
    draw_triangle(top, bottom, left, color);
}

fn draw_diamond_lines(center: Vec2, width: f32, height: f32, thickness: f32, color: Color) {
    let points = [
        vec2(center.x, center.y - height * 0.5),
        vec2(center.x + width * 0.5, center.y),
        vec2(center.x, center.y + height * 0.5),
        vec2(center.x - width * 0.5, center.y),
    ];
    for i in 0..4 {
        draw_line(
            points[i].x,
            points[i].y,
            points[(i + 1) % 4].x,
            points[(i + 1) % 4].y,
            thickness,
            color,
        );
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::new(color.r, color.g, color.b, alpha)
}

fn darken(color: Color, factor: f32) -> Color {
    Color::new(
        color.r * factor,
        color.g * factor,
        color.b * factor,
        color.a,
    )
}

fn draw_ui_button(rect: Rect, label: &str, enabled: bool, accent: bool) {
    let mouse = vec2(mouse_position().0, mouse_position().1);
    let hovered = enabled && rect.contains(mouse);
    let fill = if !enabled {
        Color::from_rgba(51, 53, 50, 255)
    } else if accent && hovered {
        Color::from_rgba(183, 133, 44, 255)
    } else if accent {
        Color::from_rgba(142, 98, 34, 255)
    } else if hovered {
        Color::from_rgba(86, 100, 91, 255)
    } else {
        Color::from_rgba(61, 68, 63, 255)
    };
    let border = if accent {
        Color::from_rgba(236, 201, 105, 255)
    } else if enabled {
        Color::from_rgba(131, 137, 122, 255)
    } else {
        Color::from_rgba(75, 77, 72, 255)
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, border);
    let mut font_size = if rect.h > 38.0 { 19.0 } else { 15.0 };
    while font_size > 11.0 && measure_text(label, None, font_size as u16, 1.0).width > rect.w - 10.0
    {
        font_size -= 1.0;
    }
    let size = measure_text(label, None, font_size as u16, 1.0);
    draw_text(
        label,
        rect.x + (rect.w - size.width) * 0.5,
        rect.y + (rect.h + font_size * 0.68) * 0.5,
        font_size,
        if enabled {
            Color::from_rgba(241, 238, 218, 255)
        } else {
            Color::from_rgba(112, 115, 108, 255)
        },
    );
}

fn draw_meter(rect: Rect, value: i32, maximum: i32, color: Color) {
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        Color::from_rgba(24, 27, 25, 255),
    );
    let fraction = if maximum > 0 {
        value.max(0) as f32 / maximum as f32
    } else {
        0.0
    }
    .clamp(0.0, 1.0);
    draw_rectangle(
        rect.x + 2.0,
        rect.y + 2.0,
        (rect.w - 4.0) * fraction,
        rect.h - 4.0,
        color,
    );
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        1.0,
        Color::from_rgba(113, 116, 105, 255),
    );
}

fn centered_text(text: &str, y: f32, size: f32, color: Color) {
    let width = measure_text(text, None, size as u16, 1.0).width;
    draw_text(text, (screen_width() - width) * 0.5, y, size, color);
}

fn short_unit_name(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::Settlers => "Settler",
        UnitKind::Worker => "Worker",
        UnitKind::Diplomat => "Diplomat",
        UnitKind::Explorer => "Explorer",
        UnitKind::Warrior => "Warrior",
        UnitKind::Phalanx => "Phalanx",
        UnitKind::Archer => "Archer",
        UnitKind::Legion => "Legion",
        UnitKind::Horseman => "Horseman",
        UnitKind::Chariot => "Chariot",
        UnitKind::Catapult => "Catapult",
        UnitKind::Musketeers => "Musketeers",
        UnitKind::Knights => "Knights",
        UnitKind::Partisan => "Partisan",
        UnitKind::Cannon => "Cannon",
        UnitKind::Caravel => "Caravel",
        UnitKind::Galleon => "Galleon",
    }
}

fn clip_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        let mut clipped: String = text.chars().take(max_chars.saturating_sub(3)).collect();
        clipped.push_str("...");
        clipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clicking_a_garrisoned_city_opens_city_management() {
        let mut game = Game::new(crate::mapgen::generate(42));
        let settler = game
            .sim
            .units()
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
            .map(|unit| unit.id)
            .expect("human settler");
        assert!(game.found_city(settler));
        let city = game
            .sim
            .cities()
            .iter()
            .find(|city| city.owner == HUMAN)
            .cloned()
            .expect("human city");
        let garrison = game
            .sim
            .units()
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.x == city.x && unit.y == city.y)
            .map(|unit| unit.id)
            .expect("garrisoned unit");

        game.selected_city = None;
        game.selected_unit = Some(garrison);
        game.select_at(city.x, city.y);

        assert_eq!(game.selected_city, Some(city.id));
        assert_eq!(game.selected_unit, None);
    }
}
