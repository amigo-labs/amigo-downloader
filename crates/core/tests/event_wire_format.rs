//! Pins the JSON the WebSocket sends for a `DownloadEvent`.
//!
//! The web UI parses exactly these shapes (`web-ui/src/lib/wsEvents.ts`,
//! `web-ui/test/wsEvents.test.ts`): fields sit next to `type`, without an
//! envelope. Change both sides together.

use krake_core::coordinator::DownloadEvent;
use krake_core::protocol::http::DownloadProgress;

fn wire(event: &DownloadEvent) -> String {
    serde_json::to_string(event).expect("DownloadEvent serializes")
}

#[test]
fn progress_event_is_flat_with_a_progress_object() {
    let event = DownloadEvent::Progress {
        id: "d1".into(),
        progress: DownloadProgress {
            bytes_downloaded: 341_400_000,
            total_bytes: Some(900_000_000),
            speed_bytes_per_sec: 4_896_147,
        },
    };
    assert_eq!(
        wire(&event),
        r#"{"type":"progress","id":"d1","progress":{"bytes_downloaded":341400000,"total_bytes":900000000,"speed_bytes_per_sec":4896147}}"#
    );
}

#[test]
fn failed_event_carries_the_error_next_to_the_type() {
    let event = DownloadEvent::Failed {
        id: "d1".into(),
        error: "HTTP 404 Not Found".into(),
    };
    assert_eq!(
        wire(&event),
        r#"{"type":"failed","id":"d1","error":"HTTP 404 Not Found"}"#
    );
}

#[test]
fn captcha_and_notification_events_are_flat() {
    let captcha = DownloadEvent::CaptchaChallenge {
        id: "c1".into(),
        plugin_id: "hoster".into(),
        download_id: "d1".into(),
        image_url: "https://example.com/c.png".into(),
        captcha_type: "image".into(),
    };
    assert_eq!(
        wire(&captcha),
        r#"{"type":"captcha_challenge","id":"c1","plugin_id":"hoster","download_id":"d1","image_url":"https://example.com/c.png","captcha_type":"image"}"#
    );
    let note = DownloadEvent::PluginNotification {
        plugin_id: "hoster".into(),
        title: "Login".into(),
        message: "Session renewed".into(),
    };
    assert_eq!(
        wire(&note),
        r#"{"type":"plugin_notification","plugin_id":"hoster","title":"Login","message":"Session renewed"}"#
    );
    assert_eq!(
        wire(&DownloadEvent::QueueEmpty),
        r#"{"type":"queue_empty"}"#
    );
}
