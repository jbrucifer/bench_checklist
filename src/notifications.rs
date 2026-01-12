use crate::checkers::{CheckResult, OverallStatus};
use winrt_notification::{Duration, Sound, Toast};

/// Send a toast notification for drift detection
pub fn notify_drift(failed_checks: &[&CheckResult]) {
    if failed_checks.is_empty() {
        return;
    }

    tracing::info!("Sending drift notification for {} checks", failed_checks.len());

    let title = if failed_checks.len() == 1 {
        "⚠ Setting Changed".to_string()
    } else {
        format!("⚠ {} Settings Changed", failed_checks.len())
    };

    let body: String = failed_checks
        .iter()
        .take(3) // Limit to 3 items in notification
        .map(|r| format!("• {}: {} → {}", r.name, r.expected_value, r.current_value))
        .collect::<Vec<_>>()
        .join("\n");

    let body = if failed_checks.len() > 3 {
        format!("{}\n... and {} more", body, failed_checks.len() - 3)
    } else {
        body
    };

    let result = Toast::new(Toast::POWERSHELL_APP_ID)
        .title(&title)
        .text1(&body)
        .sound(Some(Sound::Default))
        .duration(Duration::Long)
        .show();

    match result {
        Ok(_) => tracing::info!("Toast notification sent successfully"),
        Err(e) => tracing::error!("Failed to send toast notification: {:?}", e),
    }
}

/// Send a toast notification that all checks passed
#[allow(dead_code)]
pub fn notify_all_passed() {
    let _ = Toast::new(Toast::POWERSHELL_APP_ID)
        .title("All Checks Passed")
        .text1("Your system is configured for optimal performance.")
        .sound(Some(Sound::Default))
        .duration(Duration::Short)
        .show();
}

/// Send a status toast based on overall status
#[allow(dead_code)]
pub fn notify_status(status: OverallStatus, passed: usize, total: usize) {
    let (title, body) = match status {
        OverallStatus::AllPassed => (
            "All Checks Passed".to_string(),
            format!("{}/{} checks passed", passed, total),
        ),
        OverallStatus::SomeFailed => (
            "Some Checks Failed".to_string(),
            format!("{}/{} checks passed", passed, total),
        ),
        OverallStatus::AllFailed => (
            "All Checks Failed".to_string(),
            format!("0/{} checks passed - review your settings", total),
        ),
    };

    let _ = Toast::new(Toast::POWERSHELL_APP_ID)
        .title(&title)
        .text1(&body)
        .sound(Some(Sound::Default))
        .duration(Duration::Short)
        .show();
}
