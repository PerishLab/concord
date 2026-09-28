use concord_core::Error;
use concord_core::activity::IssueActivity;
use serde_json::json;

pub fn issue_activity(activity: &IssueActivity, output: bool) {
    if activity.recent.is_empty() {
        return;
    }
    let warning = json!({
        "warning": {
            "code": "concord.activity.concurrent_session",
            "message": "another session touched this Issue within the recent window; take care",
            "issue": activity.issue,
            "node": activity.node,
            "window_seconds": activity.window,
            "current": activity.current,
            "sessions": activity.recent,
        }
    });
    show(&warning, output);
    if !output {
        detail(&activity.issue.identity(), &activity.recent);
    }
}

pub fn unavailable(subject: &str, error: &Error, output: bool) {
    let warning = json!({
        "warning": {
            "code": "concord.activity.unavailable",
            "message": "activity is unavailable; the primary command remains unaffected",
            "subject": subject,
            "details": {
                "code": error.code(),
                "message": error.message(),
            },
        }
    });
    if output {
        show(&warning, true);
        return;
    }
    eprintln!(
        "concord: warning: activity is unavailable for {subject}: {}",
        error.message()
    );
}

fn show(warning: &serde_json::Value, output: bool) {
    if output {
        eprintln!(
            "{}",
            serde_json::to_string(warning).expect("activity warning JSON should encode")
        );
    }
}

fn detail(subject: &str, touches: &[concord_core::activity::Touch]) {
    eprintln!("concord: warning: {subject} has recent activity from another session; take care");
    for touch in touches {
        if let (Some(agent), Some(session)) = (touch.agent, touch.session.as_deref()) {
            eprintln!(
                "  {} {} {} at {}",
                agent.name(),
                session,
                touch.operation,
                touch.time
            );
        }
    }
}
