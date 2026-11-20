//! Durable player-facing campaign notifications.

use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum NotificationPriority {
    #[default]
    Routine,
    Important,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum NotificationCategory {
    #[default]
    General,
    Diplomacy,
    City,
    Economy,
    Military,
    Research,
    World,
    Religion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Notification {
    pub(crate) id: u64,
    pub(crate) turn: i32,
    pub(crate) category: NotificationCategory,
    pub(crate) priority: NotificationPriority,
    pub(crate) text: String,
    pub(crate) related_city: Option<u32>,
    pub(crate) related_civilization: Option<usize>,
    pub(crate) read: bool,
}

impl Simulation {
    pub(crate) fn upgrade_legacy_notifications(&mut self) {
        self.next_notification_id = self.next_notification_id.max(1);
        if !self.legacy_events.is_empty() {
            let old_events = std::mem::take(&mut self.legacy_events);
            let start_id = self.next_notification_id;
            let migrated: Vec<_> = old_events
                .into_iter()
                .enumerate()
                .map(|(index, text)| Notification {
                    id: start_id.saturating_add(index as u64),
                    turn: self.turn,
                    category: NotificationCategory::General,
                    priority: NotificationPriority::Routine,
                    text,
                    related_city: None,
                    related_civilization: None,
                    // Old feed entries were already visible in the HUD.
                    read: true,
                })
                .collect();
            self.next_notification_id = self
                .next_notification_id
                .saturating_add(migrated.len() as u64);
            self.notifications.extend(migrated);
        }
        #[cfg(test)]
        {
            self.events = self
                .notifications
                .iter()
                .map(|item| item.text.clone())
                .collect();
        }
        if let Some(max_id) = self.notifications.iter().map(|item| item.id).max() {
            self.next_notification_id = self.next_notification_id.max(max_id.saturating_add(1));
        }
    }

    pub(crate) fn add_event(&mut self, text: impl Into<String>) {
        self.add_notification(
            text,
            NotificationCategory::General,
            NotificationPriority::Routine,
        );
    }

    pub(crate) fn add_important_event(&mut self, text: impl Into<String>) {
        self.add_notification(
            text,
            NotificationCategory::General,
            NotificationPriority::Important,
        );
    }

    pub(crate) fn add_notification(
        &mut self,
        text: impl Into<String>,
        category: NotificationCategory,
        priority: NotificationPriority,
    ) {
        self.add_contextual_notification(text, category, priority, None, None);
    }

    pub(crate) fn add_contextual_notification(
        &mut self,
        text: impl Into<String>,
        category: NotificationCategory,
        priority: NotificationPriority,
        related_city: Option<u32>,
        related_civilization: Option<usize>,
    ) {
        if self.next_notification_id == 0 {
            self.next_notification_id = 1;
        }
        let notification = Notification {
            id: self.next_notification_id,
            turn: self.turn,
            category,
            priority,
            text: text.into(),
            related_city,
            related_civilization,
            read: false,
        };
        self.next_notification_id = self.next_notification_id.saturating_add(1);
        #[cfg(test)]
        self.events.push(notification.text.clone());
        self.notifications.push(notification);
        // Keep saved campaign history bounded while retaining a useful chronicle.
        if self.notifications.len() > 500 {
            self.notifications.remove(0);
        }
    }

    #[cfg(test)]
    pub(crate) fn mark_notifications_read(&mut self) {
        for notification in &mut self.notifications {
            notification.read = true;
        }
    }

    #[cfg(test)]
    pub(crate) fn events(&self) -> &[String] {
        &self.events
    }

    pub(crate) fn clear_notifications(&mut self) {
        self.notifications.clear();
        self.legacy_events.clear();
        #[cfg(test)]
        self.events.clear();
        self.next_notification_id = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_get_distinct_ids_context_priority_and_unread_state() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.clear_notifications();
        game.turn = 7;

        game.add_contextual_notification(
            "A city changed hands.",
            NotificationCategory::City,
            NotificationPriority::Important,
            Some(23),
            Some(2),
        );
        game.add_event("Routine update.");

        assert_eq!(game.notifications.len(), 2);
        let important = &game.notifications[0];
        assert_eq!(important.id, 1);
        assert_eq!(important.turn, 7);
        assert_eq!(important.category, NotificationCategory::City);
        assert_eq!(important.priority, NotificationPriority::Important);
        assert_eq!(important.related_city, Some(23));
        assert_eq!(important.related_civilization, Some(2));
        assert!(!important.read);
        assert_eq!(game.notifications[1].id, 2);
        assert_eq!(game.unread_notifications_view(), 2);

        game.mark_notifications_read();
        assert_eq!(game.unread_notifications_view(), 0);
        assert!(game.notifications.iter().all(|item| item.read));
    }

    #[test]
    fn notification_history_is_bounded_and_ids_keep_advancing() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.clear_notifications();
        for index in 0..501 {
            game.add_event(format!("Notice {index}"));
        }
        assert_eq!(game.notifications.len(), 500);
        assert_eq!(game.notifications.first().unwrap().id, 2);
        assert_eq!(game.notifications.last().unwrap().id, 501);
        assert_eq!(game.next_notification_id, 502);
    }
}
