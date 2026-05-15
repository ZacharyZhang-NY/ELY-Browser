use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DayPhase {
    Morning,
    Afternoon,
    Evening,
}

impl DayPhase {
    fn as_str(self) -> &'static str {
        match self {
            DayPhase::Morning => "morning",
            DayPhase::Afternoon => "afternoon",
            DayPhase::Evening => "evening",
        }
    }
}

pub(crate) fn day_phase_for(now: SystemTime) -> DayPhase {
    let hour = match now.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let day_seconds = (duration.as_secs() as i64).rem_euclid(86_400);
            (day_seconds / 3_600) as u32
        }
        Err(_) => 12,
    };
    day_phase_from_hour(hour)
}

pub(crate) fn day_phase_from_hour(hour_utc: u32) -> DayPhase {
    match hour_utc {
        5..=11 => DayPhase::Morning,
        12..=17 => DayPhase::Afternoon,
        _ => DayPhase::Evening,
    }
}

pub(crate) fn greeting_for_now(now: SystemTime, name: &str) -> String {
    let phase = day_phase_for(now).as_str();

    if name.is_empty() { format!("Good {phase}") } else { format!("Good {phase}, {name}") }
}

pub(crate) fn relative_time_label(now: SystemTime, then: SystemTime) -> String {
    let elapsed = match now.duration_since(then) {
        Ok(duration) => duration,
        Err(_) => return "just now".to_string(),
    };

    let seconds = elapsed.as_secs();

    if seconds < 60 {
        return "just now".to_string();
    }
    if seconds < 3_600 {
        let minutes = seconds / 60;
        return format!("{minutes} min ago");
    }
    if seconds < 86_400 {
        let hours = seconds / 3_600;
        let unit = if hours == 1 { "hour" } else { "hours" };
        return format!("{hours} {unit} ago");
    }
    let days = seconds / 86_400;
    let unit = if days == 1 { "day" } else { "days" };
    format!("{days} {unit} ago")
}

#[cfg(test)]
mod tests {
    use super::{DayPhase, day_phase_from_hour, greeting_for_now, relative_time_label};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn day_phase_assigns_morning_to_seven_am() {
        assert_eq!(day_phase_from_hour(7), DayPhase::Morning);
    }

    #[test]
    fn day_phase_assigns_afternoon_to_one_pm() {
        assert_eq!(day_phase_from_hour(13), DayPhase::Afternoon);
    }

    #[test]
    fn day_phase_assigns_evening_to_eleven_pm() {
        assert_eq!(day_phase_from_hour(23), DayPhase::Evening);
    }

    #[test]
    fn greeting_includes_profile_name() {
        let two_pm = UNIX_EPOCH + Duration::from_secs(14 * 3_600);
        assert_eq!(greeting_for_now(two_pm, "Alex"), "Good afternoon, Alex");
    }

    #[test]
    fn greeting_omits_name_when_blank() {
        let nine_am = UNIX_EPOCH + Duration::from_secs(9 * 3_600);
        assert_eq!(greeting_for_now(nine_am, ""), "Good morning");
    }

    #[test]
    fn relative_time_label_uses_minutes_under_an_hour() {
        let now = UNIX_EPOCH + Duration::from_secs(180);
        let then = UNIX_EPOCH + Duration::from_secs(60);
        assert_eq!(relative_time_label(now, then), "2 min ago");
    }

    #[test]
    fn relative_time_label_uses_hours_within_a_day() {
        let now = UNIX_EPOCH + Duration::from_secs(7_200);
        let then = UNIX_EPOCH;
        assert_eq!(relative_time_label(now, then), "2 hours ago");
    }

    #[test]
    fn relative_time_label_singularizes_one_hour() {
        let now = UNIX_EPOCH + Duration::from_secs(3_600);
        let then = UNIX_EPOCH;
        assert_eq!(relative_time_label(now, then), "1 hour ago");
    }

    #[test]
    fn relative_time_label_uses_days_above_a_day() {
        let now = UNIX_EPOCH + Duration::from_secs(86_400 * 3);
        let then = UNIX_EPOCH;
        assert_eq!(relative_time_label(now, then), "3 days ago");
    }
}
