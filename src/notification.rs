//! Notification System
//!
//! Manages notifications for GCE operations with toast messages,
//! operation polling, and history tracking.

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Level of detail for notifications
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailLevel {
    /// Minimal: action + resource + status icon
    Minimal,
    /// Detailed: action + resource + timestamp + duration
    #[default]
    Detailed,
    /// Verbose: all info including error details
    Verbose,
}

impl DetailLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "minimal" => Self::Minimal,
            "verbose" => Self::Verbose,
            _ => Self::Detailed,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Detailed => "detailed",
            Self::Verbose => "verbose",
        }
    }
}

/// Sound configuration for notifications
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoundConfig {
    /// No sounds
    #[default]
    Off,
    /// Beep on errors only
    ErrorsOnly,
    /// Beep on all completions
    All,
}

impl SoundConfig {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "errors_only" | "errors" => Self::ErrorsOnly,
            "all" => Self::All,
            _ => Self::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::ErrorsOnly => "errors_only",
            Self::All => "all",
        }
    }
}

/// Type of operation being performed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationType {
    Start,
    Stop,
    Reset,
    Delete,
    Other(String),
}

impl OperationType {
    pub fn from_method(method: &str) -> Self {
        match method {
            "start_instance" => Self::Start,
            "stop_instance" => Self::Stop,
            "reset_instance" => Self::Reset,
            m if m.starts_with("delete_") => Self::Delete,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Reset => "Reset",
            Self::Delete => "Delete",
            Self::Other(name) => name,
        }
    }

    pub fn past_tense(&self) -> &str {
        match self {
            Self::Start => "Started",
            Self::Stop => "Stopped",
            Self::Reset => "Reset",
            Self::Delete => "Deleted",
            Self::Other(_) => "Completed",
        }
    }

    pub fn present_participle(&self) -> &str {
        match self {
            Self::Start => "Starting",
            Self::Stop => "Stopping",
            Self::Reset => "Resetting",
            Self::Delete => "Deleting",
            Self::Other(_) => "Processing",
        }
    }
}

/// Status of a notification/operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationStatus {
    /// Operation has been submitted, waiting for GCP
    Pending,
    /// Operation is in progress (polling)
    InProgress,
    /// Operation completed successfully
    Success,
    /// Operation failed with error message
    Error(String),
}

impl NotificationStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Success | Self::Error(_))
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Pending => "◯",
            Self::InProgress => "↻",
            Self::Success => "✓",
            Self::Error(_) => "✗",
        }
    }
}

/// A single notification
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: Uuid,
    pub operation_type: OperationType,
    pub resource_type: String,
    pub resource_id: String,
    pub status: NotificationStatus,
    pub message: Option<String>,
    pub gcp_operation_url: Option<String>,
    pub created_at: Instant,
    pub completed_at: Option<Instant>,
}

impl Notification {
    pub fn new(
        operation_type: OperationType,
        resource_type: String,
        resource_id: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            operation_type,
            resource_type,
            resource_id,
            status: NotificationStatus::Pending,
            message: None,
            gcp_operation_url: None,
            created_at: Instant::now(),
            completed_at: None,
        }
    }

    /// Mark operation as in progress with GCP operation URL
    pub fn set_in_progress(&mut self, operation_url: Option<String>) {
        self.status = NotificationStatus::InProgress;
        self.gcp_operation_url = operation_url;
    }

    /// Mark operation as successful
    pub fn set_success(&mut self) {
        self.status = NotificationStatus::Success;
        self.completed_at = Some(Instant::now());
    }

    /// Mark operation as failed
    pub fn set_error(&mut self, error: String) {
        self.status = NotificationStatus::Error(error);
        self.completed_at = Some(Instant::now());
    }

    /// Get duration of operation (or elapsed time if still running)
    pub fn duration(&self) -> Duration {
        self.completed_at
            .unwrap_or_else(Instant::now)
            .duration_since(self.created_at)
    }

    /// Format duration for display
    pub fn duration_display(&self) -> String {
        let d = self.duration();
        if d.as_secs() < 1 {
            format!("{}ms", d.as_millis())
        } else if d.as_secs() < 60 {
            format!("{}s", d.as_secs())
        } else {
            format!("{}m{}s", d.as_secs() / 60, d.as_secs() % 60)
        }
    }

    /// Format notification for toast display (short form)
    pub fn toast_message(&self, detail_level: DetailLevel) -> String {
        let icon = self.status.icon();
        let verb = match &self.status {
            NotificationStatus::Pending | NotificationStatus::InProgress => {
                self.operation_type.present_participle()
            }
            NotificationStatus::Success => self.operation_type.past_tense(),
            NotificationStatus::Error(_) => "Failed",
        };

        match detail_level {
            DetailLevel::Minimal => {
                format!("{} {} {}", icon, verb, self.resource_id)
            }
            DetailLevel::Detailed => {
                if self.status.is_terminal() {
                    format!(
                        "{} {} {} ({})",
                        icon,
                        verb,
                        self.resource_id,
                        self.duration_display()
                    )
                } else {
                    format!("{} {} {}...", icon, verb, self.resource_id)
                }
            }
            DetailLevel::Verbose => {
                let base = format!(
                    "{} {} {} [{}]",
                    icon,
                    verb,
                    self.resource_id,
                    self.resource_type
                );
                if let NotificationStatus::Error(ref err) = self.status {
                    format!("{} - {}", base, err)
                } else if self.status.is_terminal() {
                    format!("{} ({})", base, self.duration_display())
                } else {
                    format!("{}...", base)
                }
            }
        }
    }
}

/// Pending operation that needs polling
#[derive(Debug, Clone)]
pub struct PendingOperation {
    pub notification_id: Uuid,
    pub operation_url: String,
    pub last_poll: Instant,
    pub poll_count: u32,
}

impl PendingOperation {
    pub fn new(notification_id: Uuid, operation_url: String) -> Self {
        Self {
            notification_id,
            operation_url,
            last_poll: Instant::now(),
            poll_count: 0,
        }
    }

    pub fn should_poll(&self, interval: Duration) -> bool {
        self.last_poll.elapsed() >= interval
    }

    pub fn mark_polled(&mut self) {
        self.last_poll = Instant::now();
        self.poll_count += 1;
    }
}

/// Notification manager
pub struct NotificationManager {
    /// All notifications (recent first)
    pub notifications: VecDeque<Notification>,
    /// Operations currently being polled
    pub pending_operations: Vec<PendingOperation>,
    /// Maximum notifications to keep in history
    pub max_history: usize,
    /// Toast display duration
    pub toast_duration: Duration,
    /// Polling interval for pending operations
    pub poll_interval: Duration,
    /// Detail level for display
    pub detail_level: DetailLevel,
    /// Sound configuration
    pub sound_config: SoundConfig,
    /// Whether auto-polling is enabled
    pub auto_poll: bool,
    /// Last toast notification (for display)
    last_toast_time: Option<Instant>,
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            notifications: VecDeque::new(),
            pending_operations: Vec::new(),
            max_history: 50,
            toast_duration: Duration::from_secs(5),
            poll_interval: Duration::from_millis(2000),
            detail_level: DetailLevel::Detailed,
            sound_config: SoundConfig::Off,
            auto_poll: true,
            last_toast_time: None,
        }
    }

    /// Create a new notification for an operation
    pub fn create_notification(
        &mut self,
        operation_type: OperationType,
        resource_type: String,
        resource_id: String,
    ) -> Uuid {
        let notification = Notification::new(operation_type, resource_type, resource_id);
        let id = notification.id;
        self.notifications.push_front(notification);
        self.last_toast_time = Some(Instant::now());
        self.trim_history();
        id
    }

    /// Mark a notification as in progress and optionally start polling
    pub fn mark_in_progress(&mut self, id: Uuid, operation_url: Option<String>) {
        if let Some(notif) = self.notifications.iter_mut().find(|n| n.id == id) {
            notif.set_in_progress(operation_url.clone());

            // If we have an operation URL and auto-poll is enabled, start polling
            if let Some(url) = operation_url {
                if self.auto_poll {
                    self.pending_operations.push(PendingOperation::new(id, url));
                }
            }
            self.last_toast_time = Some(Instant::now());
        }
    }

    /// Mark a notification as successful
    pub fn mark_success(&mut self, id: Uuid) {
        if let Some(notif) = self.notifications.iter_mut().find(|n| n.id == id) {
            notif.set_success();
            self.last_toast_time = Some(Instant::now());

            // Remove from pending operations
            self.pending_operations.retain(|p| p.notification_id != id);

            // Play sound if configured
            if self.sound_config == SoundConfig::All {
                self.play_beep();
            }
        }
    }

    /// Mark a notification as failed
    pub fn mark_error(&mut self, id: Uuid, error: String) {
        if let Some(notif) = self.notifications.iter_mut().find(|n| n.id == id) {
            notif.set_error(error);
            self.last_toast_time = Some(Instant::now());

            // Remove from pending operations
            self.pending_operations.retain(|p| p.notification_id != id);

            // Play sound if configured
            if matches!(self.sound_config, SoundConfig::ErrorsOnly | SoundConfig::All) {
                self.play_beep();
            }
        }
    }

    /// Get notification by ID
    pub fn get(&self, id: Uuid) -> Option<&Notification> {
        self.notifications.iter().find(|n| n.id == id)
    }

    /// Get the most recent active notification (for toast display)
    pub fn current_toast(&self) -> Option<&Notification> {
        // Check if toast should still be visible
        if let Some(last_time) = self.last_toast_time {
            if last_time.elapsed() > self.toast_duration {
                return None;
            }
        } else {
            return None;
        }

        // Return most recent notification
        self.notifications.front()
    }

    /// Get count of in-progress operations
    pub fn in_progress_count(&self) -> usize {
        self.notifications
            .iter()
            .filter(|n| matches!(n.status, NotificationStatus::InProgress | NotificationStatus::Pending))
            .count()
    }

    /// Get pending operations that need polling
    pub fn operations_to_poll(&mut self) -> Vec<(Uuid, String)> {
        let interval = self.poll_interval;
        self.pending_operations
            .iter_mut()
            .filter(|p| p.should_poll(interval))
            .map(|p| {
                p.mark_polled();
                (p.notification_id, p.operation_url.clone())
            })
            .collect()
    }

    /// Clear all notifications
    pub fn clear(&mut self) {
        self.notifications.clear();
        self.pending_operations.clear();
        self.last_toast_time = None;
    }

    /// Trim history to max size
    fn trim_history(&mut self) {
        while self.notifications.len() > self.max_history {
            // Remove oldest completed notification
            if let Some(pos) = self.notifications.iter().rposition(|n| n.status.is_terminal()) {
                self.notifications.remove(pos);
            } else {
                // If all are active, remove from back anyway
                self.notifications.pop_back();
            }
        }
    }

    /// Play terminal beep
    fn play_beep(&self) {
        print!("\x07");
    }

    /// Check if there are any notifications to show
    pub fn has_notifications(&self) -> bool {
        !self.notifications.is_empty()
    }

    /// Get count of unread/recent notifications (last 5 minutes)
    pub fn recent_count(&self) -> usize {
        let cutoff = Duration::from_secs(300); // 5 minutes
        self.notifications
            .iter()
            .filter(|n| n.created_at.elapsed() < cutoff)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_lifecycle() {
        let mut manager = NotificationManager::new();

        // Create notification
        let id = manager.create_notification(
            OperationType::Start,
            "compute-instances".to_string(),
            "my-vm".to_string(),
        );

        assert_eq!(manager.notifications.len(), 1);
        assert!(matches!(
            manager.get(id).unwrap().status,
            NotificationStatus::Pending
        ));

        // Mark in progress
        manager.mark_in_progress(id, Some("https://example.com/op/123".to_string()));
        assert!(matches!(
            manager.get(id).unwrap().status,
            NotificationStatus::InProgress
        ));

        // Mark success
        manager.mark_success(id);
        assert!(matches!(
            manager.get(id).unwrap().status,
            NotificationStatus::Success
        ));
    }

    #[test]
    fn test_operation_type_from_method() {
        assert!(matches!(
            OperationType::from_method("start_instance"),
            OperationType::Start
        ));
        assert!(matches!(
            OperationType::from_method("delete_disk"),
            OperationType::Delete
        ));
        assert!(matches!(
            OperationType::from_method("custom_action"),
            OperationType::Other(_)
        ));
    }

    #[test]
    fn test_toast_message_formats() {
        let mut notif = Notification::new(
            OperationType::Start,
            "compute-instances".to_string(),
            "my-vm".to_string(),
        );

        // Pending state
        let msg = notif.toast_message(DetailLevel::Minimal);
        assert!(msg.contains("Starting"));
        assert!(msg.contains("my-vm"));

        // Success state
        notif.set_success();
        let msg = notif.toast_message(DetailLevel::Minimal);
        assert!(msg.contains("Started"));
        assert!(msg.contains("✓"));
    }
}
