use serde::{Deserialize, Serialize};

use super::summary::GroupSummary;

/// Pushover limits (characters). We truncate defensively; byte-length is used as
/// a conservative proxy so we never exceed the character limit.
const MESSAGE_LIMIT: usize = 1024;
const TITLE_LIMIT: usize = 250;

/// Emergency priority (2) requires retry + expire or the API rejects the call.
const EMERGENCY_RETRY_SECS: u32 = 60;
const EMERGENCY_EXPIRE_SECS: u32 = 3600;

/// Instance-level Pushover credentials/options, sourced from env + secrets.
/// Not serialized: it never crosses the wire, only the rendered message does.
#[derive(Clone)]
pub struct PushoverConfig {
    pub token: String,
    pub user: String,
    pub priority: Option<i8>,
    pub sound: Option<String>,
}

/// A fully-rendered Pushover message, ready to be form-encoded to the API.
#[derive(Serialize, Deserialize, Clone)]
pub struct PushoverMessage {
    pub token: String,
    pub user: String,
    pub title: String,
    pub message: String,
    pub html: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire: Option<u32>,
}

// Manual Debug so credentials never end up in logs (send.rs logs the payload).
impl std::fmt::Debug for PushoverMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PushoverMessage")
            .field("token", &"<redacted>")
            .field("user", &"<redacted>")
            .field("title", &self.title)
            .field("message", &self.message)
            .field("html", &self.html)
            .field("priority", &self.priority)
            .field("sound", &self.sound)
            .field("retry", &self.retry)
            .field("expire", &self.expire)
            .finish()
    }
}

impl PushoverMessage {
    /// Render a group summary into a Pushover message. The Pushover notification
    /// title is the series (mirroring the Discord embed title); the body is the
    /// bolded headline followed by the same episode list all targets share.
    pub fn render(summary: &GroupSummary, config: &PushoverConfig) -> Self {
        let title = truncate(&summary.series_title, TITLE_LIMIT);

        // html=1: escape dynamic text, then bold the (short, leading) headline.
        // The `</b>` sits well within the limit, so truncation only ever trims
        // the tail of the episode list, never a tag.
        let body = format!(
            "<b>{}</b>\n{}",
            escape_html(&summary.headline),
            escape_html(&summary.description())
        );
        let message = truncate(&body, MESSAGE_LIMIT);

        let (retry, expire) = match config.priority {
            Some(2) => (Some(EMERGENCY_RETRY_SECS), Some(EMERGENCY_EXPIRE_SECS)),
            _ => (None, None),
        };

        PushoverMessage {
            token: config.token.clone(),
            user: config.user.clone(),
            title,
            message,
            html: 1,
            priority: config.priority,
            sound: config.sound.clone(),
            retry,
            expire,
        }
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Truncate to at most `max` bytes on a char boundary, appending an ellipsis
/// when anything was removed.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max.saturating_sub('…'.len_utf8());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structs::sonarr::SonarrEventType;
    use crate::structs::summary::{EpisodeLine, GroupSummary};

    fn summary() -> GroupSummary {
        GroupSummary {
            event_type: SonarrEventType::Grab,
            series_title: "Fake Series".into(),
            season_number: 1,
            headline: "Grabbed: Fake Series Season 01".into(),
            episodes: vec![EpisodeLine {
                season_number: 1,
                episode_number: 1,
                title: "Ep 1".into(),
                quality: "HDTV".into(),
                count: 1,
            }],
        }
    }

    fn config() -> PushoverConfig {
        PushoverConfig {
            token: "app-token".into(),
            user: "user-key".into(),
            priority: None,
            sound: None,
        }
    }

    #[test]
    fn basic_render() {
        let m = PushoverMessage::render(&summary(), &config());
        assert_eq!(m.title, "Fake Series");
        assert_eq!(m.html, 1);
        assert!(m
            .message
            .starts_with("<b>Grabbed: Fake Series Season 01</b>"));
        assert!(m.message.contains("01x01 - Ep 1 [HDTV]"));
        assert_eq!(m.token, "app-token");
        assert_eq!(m.user, "user-key");
        assert_eq!(m.priority, None);
        assert_eq!(m.retry, None);
    }

    #[test]
    fn passthrough_priority_and_sound() {
        let mut c = config();
        c.priority = Some(1);
        c.sound = Some("magic".into());
        let m = PushoverMessage::render(&summary(), &c);
        assert_eq!(m.priority, Some(1));
        assert_eq!(m.sound.as_deref(), Some("magic"));
        assert_eq!(m.retry, None);
        assert_eq!(m.expire, None);
    }

    #[test]
    fn emergency_priority_sets_retry_expire() {
        let mut c = config();
        c.priority = Some(2);
        let m = PushoverMessage::render(&summary(), &c);
        assert_eq!(m.priority, Some(2));
        assert_eq!(m.retry, Some(60));
        assert_eq!(m.expire, Some(3600));
    }

    #[test]
    fn html_is_escaped() {
        let mut s = summary();
        s.headline = "A & B <x>".into();
        let m = PushoverMessage::render(&s, &config());
        assert!(m.message.contains("<b>A &amp; B &lt;x&gt;</b>"));
    }

    #[test]
    fn debug_redacts_secrets() {
        let m = PushoverMessage::render(&summary(), &config());
        let dbg = format!("{m:?}");
        assert!(!dbg.contains("app-token"));
        assert!(!dbg.contains("user-key"));
        assert!(dbg.contains("redacted"));
    }

    #[test]
    fn long_message_is_truncated() {
        let mut s = summary();
        s.episodes = (1..=200)
            .map(|n| EpisodeLine {
                season_number: 1,
                episode_number: n,
                title: "A fairly long episode title here".into(),
                quality: "WEBDL-1080p".into(),
                count: 1,
            })
            .collect();
        let m = PushoverMessage::render(&s, &config());
        assert!(m.message.len() <= MESSAGE_LIMIT);
        assert!(m.message.ends_with('…'));
    }
}
