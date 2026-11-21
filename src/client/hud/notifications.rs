//! Campaign notification toasts and the browsable notification inbox.

use super::*;

const PAGE_SIZE: usize = 8;

fn page_bounds(item_count: usize, requested_page: usize) -> (usize, usize, usize, usize) {
    let pages = item_count.div_ceil(PAGE_SIZE).max(1);
    let page = requested_page.min(pages - 1);
    let start = item_count.saturating_sub((page + 1) * PAGE_SIZE);
    let end = item_count.saturating_sub(page * PAGE_SIZE);
    (start, end, page, pages)
}

impl Game {
    pub(in crate::client) fn draw_notification_panel(&self, actions: &mut Vec<(Rect, UiAction)>) {
        if !self.notifications_open {
            return;
        }
        let width = (screen_width() - 2.0 * HUD_MARGIN).clamp(300.0, 520.0);
        let height = (screen_height() - CONTROLS_H - 3.0 * HUD_MARGIN).clamp(220.0, 430.0);
        let panel = Rect::new(
            (screen_width() - width - HUD_MARGIN).max(HUD_MARGIN),
            CONTROLS_H + 8.0,
            width,
            height,
        );
        draw_hud_card(panel);
        actions.push((panel, UiAction::BlockMapClick));
        draw_text("Notifications", panel.x + 16.0, panel.y + 26.0, 20.0, GOLD);
        let close = Rect::new(panel.right() - 37.0, panel.y + 8.0, 27.0, 25.0);
        draw_ui_button(close, "×", true, false);
        actions.push((close, UiAction::ToggleNotifications));

        let items = self.sim.campaign_notifications_view();
        let (first, end, page, total_pages) = page_bounds(items.len(), self.notification_page);
        let rows = items[first..end].iter().rev();
        for (row, notification) in rows.enumerate() {
            let y = panel.y + 44.0 + row as f32 * 41.0;
            let color = if notification.read {
                Color::from_rgba(185, 188, 181, 255)
            } else {
                Color::from_rgba(255, 238, 180, 255)
            };
            let prefix = if notification.read { "" } else { "• " };
            let header = format!(
                "{}Turn {} · {:?}",
                prefix, notification.turn, notification.category
            );
            draw_text(&header, panel.x + 15.0, y + 12.0, 12.0, color);
            text_in(
                &notification.text,
                panel.x + 15.0,
                y + 31.0,
                14.0,
                panel.w - 30.0,
                WHITE,
            );
        }
        if items.is_empty() {
            draw_text(
                "No notifications yet.",
                panel.x + 16.0,
                panel.y + 70.0,
                15.0,
                WHITE,
            );
        }
        let page_label = format!("{} / {}", page + 1, total_pages);
        let label_width = measure_cached(&page_label, 13).width;
        draw_text(
            &page_label,
            panel.x + (panel.w - label_width) * 0.5,
            panel.bottom() - 20.0,
            13.0,
            WHITE,
        );
        for (enabled, x, label, target) in [
            (page + 1 < total_pages, panel.x + 12.0, "Older", page + 1),
            (
                page > 0,
                panel.right() - 82.0,
                "Newer",
                page.saturating_sub(1),
            ),
        ] {
            let button = Rect::new(x, panel.bottom() - 35.0, 70.0, 25.0);
            draw_ui_button(button, label, enabled, false);
            if enabled {
                actions.push((button, UiAction::NotificationsPage(target)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inbox_pages_show_newest_first_and_older_pages_backfill() {
        assert_eq!(page_bounds(19, 0), (11, 19, 0, 3));
        assert_eq!(page_bounds(19, 1), (3, 11, 1, 3));
        assert_eq!(page_bounds(19, 2), (0, 3, 2, 3));
        assert_eq!(page_bounds(19, 99), (0, 3, 2, 3));
        assert_eq!(page_bounds(0, 0), (0, 0, 0, 1));
    }
}
