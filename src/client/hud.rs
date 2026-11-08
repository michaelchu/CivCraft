//! Heads-up display composition and its map, overview, and status panels.

use super::*;
mod context_panel;
mod minimap;
mod top_bar;

impl Game {
    pub(super) fn draw_hud(&mut self, assets: &Assets) -> Vec<(Rect, UiAction)> {
        let mut actions = Vec::new();
        self.draw_minimap();
        self.hud_elements_drawn += 1;

        let context = self.context_rect();
        draw_hud_card(context);
        let end = Rect::new(
            context.x + 10.0,
            context.bottom() - 38.0,
            context.w - 20.0,
            28.0,
        );
        self.draw_context_panel(assets, context, end.y);
        draw_ui_button(end, "", true, true);
        let shortcut = "Shift+Enter";
        let shortcut_width = measure_text(shortcut, None, 11, 1.0).width;
        draw_text("End Turn", end.x + 8.0, end.y + 18.0, 13.0, WHITE);
        draw_text(
            shortcut,
            end.right() - 8.0 - shortcut_width,
            end.y + 18.0,
            11.0,
            Color::from_rgba(241, 223, 189, 255),
        );
        actions.push((end, UiAction::EndTurn));
        actions.push((context, UiAction::BlockMapClick));
        self.hud_elements_drawn += 1;

        let action_width = screen_width() - 2.0 * (Self::corner_width() + 2.0 * HUD_MARGIN);
        self.draw_action_strip(&mut actions, action_width);
        let portraits_left = self.draw_leader_portraits(assets, &mut actions);
        self.draw_messages(&mut actions, portraits_left - 2.0 * HUD_MARGIN);
        self.draw_top_bar(&mut actions);
        self.hud_elements_drawn += 1;
        self.hud_elements_drawn += 1;
        actions
    }

    pub(super) fn draw_intro(&self) -> Vec<(Rect, UiAction)> {
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::from_rgba(4, 9, 12, 178),
        );
        let width = (screen_width() - 2.0 * HUD_MARGIN).min(470.0);
        let height = 254.0;
        let panel = Rect::new(
            (screen_width() - width) * 0.5,
            (screen_height() - height) * 0.5,
            width,
            height,
        );
        panel_frame(panel);
        text_in(
            "Your Chronicle",
            panel.x + 22.0,
            panel.y + 41.0,
            27.0,
            width - 44.0,
            GOLD,
        );
        for (index, line) in [
            "Found a city with your Settlers.",
            "Explore the fog and improve the land.",
            "Research stronger units.",
            "Unite the rival nations.",
            "Click a unit or city for details.",
        ]
        .into_iter()
        .enumerate()
        {
            text_in(
                line,
                panel.x + 22.0,
                panel.y + 75.0 + index as f32 * 25.0,
                17.0,
                width - 44.0,
                WHITE,
            );
        }
        let button = Rect::new(panel.right() - 172.0, panel.bottom() - 52.0, 150.0, 34.0);
        draw_ui_button(button, "Begin [Enter]", true, true);
        vec![(button, UiAction::DismissIntro)]
    }

    fn draw_messages(&self, actions: &mut Vec<(Rect, UiAction)>, available: f32) {
        let width = (screen_width() * 0.65).min(620.0).min(available);
        let latest = self.sim.events().iter().rev().take(2);
        for (i, message) in latest.enumerate() {
            let color = if i == 0 {
                Color::from_rgba(230, 228, 211, 255)
            } else {
                Color::from_rgba(174, 177, 171, 255)
            };
            let clipped = fit_text(message, 15.0, width - 16.0);
            let rect = Rect::new(
                HUD_MARGIN,
                TOP_H + 10.0 + (1 - i) as f32 * 21.0,
                measure_text(&clipped, None, 15, 1.0).width + 16.0,
                21.0,
            );
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                Color::from_rgba(16, 20, 20, 218),
            );
            draw_text(&clipped, rect.x + 8.0, rect.y + 15.0, 15.0, color);
            actions.push((rect, UiAction::BlockMapClick));
        }
    }
}

pub(super) fn draw_hud_card(card: Rect) {
    draw_rectangle(
        card.x,
        card.y,
        card.w,
        card.h,
        Color::from_rgba(26, 29, 26, 237),
    );
    draw_rectangle_lines(
        card.x,
        card.y,
        card.w,
        card.h,
        2.0,
        Color::from_rgba(160, 126, 61, 255),
    );
}

pub(super) enum HudIcon {
    Government,
    Diplomacy,
    Research,
    Science,
    Tax,
    Luxury,
    Spaceship,
    City,
    Road,
    Water,
    Pickaxe,
    Wheat,
    Tree,
    Cycle,
    Broom,
    Shield,
    Burst,
    Home,
    Gear,
    Pause,
    Upgrade,
    Search,
    Target,
    Parachute,
    Plane,
    Boat,
    Crate,
    Wonder,
    Cross,
    More,
}

pub(super) fn draw_hud_icon(icon: HudIcon, center: Vec2, size: f32, color: Color) {
    // Coordinates use a shared 24 by 24 canvas so the icons scale together.
    let paths: &[&[(f32, f32)]] = match icon {
        HudIcon::Government => &[
            &[(2., 8.), (12., 2.), (22., 8.), (2., 8.)],
            &[(5., 11.), (5., 19.)],
            &[(12., 11.), (12., 19.)],
            &[(19., 11.), (19., 19.)],
            &[(2., 22.), (22., 22.)],
        ],
        HudIcon::Diplomacy => &[
            &[
                (2., 3.),
                (17., 3.),
                (17., 12.),
                (8., 12.),
                (4., 16.),
                (4., 12.),
                (2., 12.),
                (2., 3.),
            ],
            &[
                (20., 8.),
                (22., 8.),
                (22., 18.),
                (20., 18.),
                (20., 22.),
                (15., 18.),
                (10., 18.),
                (10., 15.),
            ],
        ],
        HudIcon::Research => &[&[
            (12., 6.),
            (8., 3.),
            (2., 3.),
            (2., 18.),
            (8., 18.),
            (12., 21.),
            (16., 18.),
            (22., 18.),
            (22., 3.),
            (16., 3.),
            (12., 6.),
            (12., 21.),
        ]],
        HudIcon::Science => &[
            &[(8., 2.), (16., 2.)],
            &[
                (9., 2.),
                (9., 10.),
                (3., 20.),
                (5., 22.),
                (19., 22.),
                (21., 20.),
                (15., 10.),
                (15., 2.),
            ],
            &[(6., 15.), (18., 15.)],
        ],
        HudIcon::Tax => &[
            &[
                (3., 6.),
                (7., 3.),
                (17., 3.),
                (21., 6.),
                (17., 9.),
                (7., 9.),
                (3., 6.),
                (3., 18.),
                (7., 21.),
                (17., 21.),
                (21., 18.),
                (21., 6.),
            ],
            &[(3., 12.), (7., 15.), (17., 15.), (21., 12.)],
        ],
        HudIcon::Luxury => &[
            &[
                (2., 8.),
                (6., 3.),
                (18., 3.),
                (22., 8.),
                (12., 22.),
                (2., 8.),
                (22., 8.),
            ],
            &[(9., 3.), (7., 8.), (12., 22.), (17., 8.), (15., 3.)],
        ],
        HudIcon::Spaceship => &[
            &[
                (12., 2.),
                (16., 7.),
                (17., 15.),
                (20., 19.),
                (15., 18.),
                (9., 18.),
                (4., 19.),
                (7., 15.),
                (8., 7.),
                (12., 2.),
            ],
            &[(10., 9.), (14., 9.)],
            &[(12., 19.), (12., 23.)],
        ],
        HudIcon::City => &[
            &[
                (2., 21.),
                (2., 9.),
                (7., 9.),
                (7., 14.),
                (10., 14.),
                (10., 4.),
                (15., 4.),
                (15., 11.),
                (21., 11.),
                (21., 21.),
                (2., 21.),
            ],
            &[(6., 17.), (6., 18.)],
            &[(16., 15.), (16., 18.)],
        ],
        HudIcon::Road => &[
            &[(3., 22.), (9., 2.)],
            &[(21., 22.), (15., 2.)],
            &[(12., 3.), (12., 7.)],
            &[(12., 10.), (12., 14.)],
            &[(12., 17.), (12., 22.)],
        ],
        HudIcon::Water => &[&[
            (12., 2.),
            (6., 10.),
            (5., 15.),
            (7., 20.),
            (12., 22.),
            (17., 20.),
            (19., 15.),
            (18., 10.),
            (12., 2.),
        ]],
        HudIcon::Pickaxe => &[
            &[(4., 21.), (16., 7.)],
            &[(5., 4.), (11., 3.), (17., 6.), (21., 12.)],
        ],
        HudIcon::Wheat => &[
            &[(12., 22.), (12., 3.)],
            &[(12., 9.), (7., 5.)],
            &[(12., 13.), (17., 8.)],
            &[(12., 17.), (6., 12.)],
            &[(12., 19.), (18., 14.)],
        ],
        HudIcon::Tree => &[
            &[
                (12., 2.),
                (5., 11.),
                (8., 11.),
                (3., 18.),
                (21., 18.),
                (16., 11.),
                (19., 11.),
                (12., 2.),
            ],
            &[(12., 18.), (12., 23.)],
        ],
        HudIcon::Cycle => &[
            &[(3., 10.), (6., 5.), (15., 4.), (20., 8.)],
            &[(15., 8.), (20., 8.), (20., 3.)],
            &[(21., 14.), (18., 19.), (9., 20.), (4., 16.)],
            &[(9., 16.), (4., 16.), (4., 21.)],
        ],
        HudIcon::Broom => &[
            &[(11., 13.), (20., 3.)],
            &[(8., 10.), (14., 15.), (10., 22.), (3., 16.), (8., 10.)],
            &[(6., 19.), (10., 14.)],
        ],
        HudIcon::Shield => &[
            &[
                (3., 4.),
                (21., 4.),
                (21., 13.),
                (18., 18.),
                (12., 22.),
                (6., 18.),
                (3., 13.),
                (3., 4.),
            ],
            &[(12., 7.), (12., 17.)],
        ],
        HudIcon::Burst => &[&[
            (12., 2.),
            (14., 8.),
            (21., 5.),
            (18., 11.),
            (23., 13.),
            (17., 15.),
            (20., 22.),
            (13., 18.),
            (9., 22.),
            (8., 16.),
            (2., 17.),
            (6., 12.),
            (2., 7.),
            (9., 9.),
            (12., 2.),
        ]],
        HudIcon::Home => &[
            &[(2., 11.), (12., 3.), (22., 11.)],
            &[(5., 9.), (5., 21.), (19., 21.), (19., 9.)],
            &[(10., 21.), (10., 15.), (14., 15.), (14., 21.)],
        ],
        HudIcon::Gear => &[
            &[
                (9., 2.),
                (15., 2.),
                (16., 6.),
                (21., 6.),
                (22., 11.),
                (19., 14.),
                (20., 19.),
                (15., 22.),
                (12., 19.),
                (8., 22.),
                (4., 19.),
                (5., 14.),
                (2., 11.),
                (3., 6.),
                (8., 6.),
                (9., 2.),
            ],
            &[(9., 12.), (12., 9.), (15., 12.), (12., 15.), (9., 12.)],
        ],
        HudIcon::Pause => &[&[(9., 5.), (9., 19.)], &[(15., 5.), (15., 19.)]],
        HudIcon::Upgrade => &[
            &[(4., 10.), (12., 3.), (20., 10.)],
            &[(4., 17.), (12., 10.), (20., 17.)],
            &[(12., 11.), (12., 22.)],
        ],
        HudIcon::Search => &[
            &[
                (4., 4.),
                (10., 2.),
                (16., 5.),
                (18., 11.),
                (15., 16.),
                (9., 18.),
                (3., 15.),
                (2., 9.),
                (4., 4.),
            ],
            &[(16., 16.), (22., 22.)],
        ],
        HudIcon::Target => &[
            &[
                (12., 4.),
                (18., 6.),
                (20., 12.),
                (18., 18.),
                (12., 20.),
                (6., 18.),
                (4., 12.),
                (6., 6.),
                (12., 4.),
            ],
            &[(12., 1.), (12., 8.)],
            &[(12., 16.), (12., 23.)],
            &[(1., 12.), (8., 12.)],
            &[(16., 12.), (23., 12.)],
        ],
        HudIcon::Parachute => &[
            &[
                (2., 10.),
                (4., 6.),
                (8., 3.),
                (16., 3.),
                (20., 6.),
                (22., 10.),
                (2., 10.),
                (12., 21.),
                (22., 10.),
            ],
            &[(8., 10.), (12., 21.), (16., 10.)],
        ],
        HudIcon::Plane => &[&[
            (12., 2.),
            (14., 4.),
            (14., 9.),
            (22., 15.),
            (22., 17.),
            (14., 14.),
            (14., 19.),
            (17., 22.),
            (7., 22.),
            (10., 19.),
            (10., 14.),
            (2., 17.),
            (2., 15.),
            (10., 9.),
            (10., 4.),
            (12., 2.),
        ]],
        HudIcon::Boat => &[
            &[(2., 15.), (22., 15.), (18., 21.), (6., 21.), (2., 15.)],
            &[(12., 15.), (12., 3.), (19., 12.), (12., 12.)],
        ],
        HudIcon::Crate => &[
            &[
                (3., 7.),
                (12., 3.),
                (21., 7.),
                (21., 17.),
                (12., 21.),
                (3., 17.),
                (3., 7.),
                (12., 11.),
                (21., 7.),
            ],
            &[(12., 11.), (12., 21.)],
            &[(7., 5.), (16., 9.)],
        ],
        HudIcon::Wonder => &[
            &[(2., 21.), (12., 3.), (22., 21.), (2., 21.)],
            &[(6., 14.), (18., 14.)],
            &[(4., 18.), (20., 18.)],
        ],
        HudIcon::Cross => &[&[(5., 5.), (19., 19.)], &[(19., 5.), (5., 19.)]],
        HudIcon::More => &[
            &[(5., 11.), (5., 13.)],
            &[(12., 11.), (12., 13.)],
            &[(19., 11.), (19., 13.)],
        ],
    };
    let point = |(x, y)| center + vec2(x - 12.0, y - 12.0) * (size / 24.0);
    for path in paths {
        for pair in path.windows(2) {
            let a = point(pair[0]);
            let b = point(pair[1]);
            draw_line(a.x, a.y, b.x, b.y, 1.6, color);
        }
    }
}
