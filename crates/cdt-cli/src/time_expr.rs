use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Utc};

#[derive(Debug, PartialEq, Eq)]
pub enum TimeExprError {
    InvalidFormat(String),
}

impl std::fmt::Display for TimeExprError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat(input) => write!(
                f,
                "invalid time expression '{input}'. Valid formats: \
                 relative duration (7d, 24h, 30m), \
                 named period (today, yesterday, week), \
                 absolute date (2026-06-06, ISO 8601)"
            ),
        }
    }
}

impl std::error::Error for TimeExprError {}

pub fn parse_time_expr(
    expr: &str,
    now: DateTime<Utc>,
    local_tz: &impl TimeZone,
) -> Result<i64, TimeExprError> {
    let s = expr.trim();
    if s.is_empty() {
        return Err(TimeExprError::InvalidFormat(s.to_string()));
    }

    if let Some(ms) = try_named_period(s, now, local_tz) {
        return Ok(ms);
    }

    if let Some(ms) = try_relative_duration(s, now) {
        return Ok(ms);
    }

    if let Some(ms) = try_absolute_date(s, local_tz) {
        return Ok(ms);
    }

    Err(TimeExprError::InvalidFormat(s.to_string()))
}

fn try_named_period(s: &str, now: DateTime<Utc>, local_tz: &impl TimeZone) -> Option<i64> {
    let local_now = now.with_timezone(local_tz);
    let local_date = local_now.date_naive();

    match s {
        "today" => {
            let start = local_tz
                .from_local_datetime(&local_date.and_hms_opt(0, 0, 0)?)
                .earliest()?;
            Some(start.with_timezone(&Utc).timestamp_millis())
        }
        "yesterday" => {
            let yesterday = local_date.pred_opt()?;
            let start = local_tz
                .from_local_datetime(&yesterday.and_hms_opt(0, 0, 0)?)
                .earliest()?;
            Some(start.with_timezone(&Utc).timestamp_millis())
        }
        "week" => {
            let days_since_monday = local_date.weekday().num_days_from_monday();
            let monday = local_date - chrono::Duration::days(i64::from(days_since_monday));
            let start = local_tz
                .from_local_datetime(&monday.and_hms_opt(0, 0, 0)?)
                .earliest()?;
            Some(start.with_timezone(&Utc).timestamp_millis())
        }
        _ => None,
    }
}

fn try_relative_duration(s: &str, now: DateTime<Utc>) -> Option<i64> {
    if s.len() < 2 {
        return None;
    }
    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str.parse().ok()?;
    if num <= 0 {
        return None;
    }
    let ms = match unit {
        "m" => num.checked_mul(60 * 1000)?,
        "h" => num.checked_mul(3600 * 1000)?,
        "d" => num.checked_mul(24 * 3600 * 1000)?,
        _ => return None,
    };
    let now_ms = now.timestamp_millis();
    now_ms.checked_sub(ms)
}

fn try_absolute_date(s: &str, local_tz: &impl TimeZone) -> Option<i64> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc).timestamp_millis());
    }

    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        let local_dt = local_tz.from_local_datetime(&dt).earliest()?;
        return Some(local_dt.with_timezone(&Utc).timestamp_millis());
    }

    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let start = local_tz
            .from_local_datetime(&date.and_hms_opt(0, 0, 0)?)
            .earliest()?;
        return Some(start.with_timezone(&Utc).timestamp_millis());
    }

    None
}

pub fn parse_time_expr_local(expr: &str) -> Result<i64, TimeExprError> {
    parse_time_expr(expr, Utc::now(), &Local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn cst() -> FixedOffset {
        FixedOffset::east_opt(8 * 3600).unwrap()
    }

    fn fixed_now() -> DateTime<Utc> {
        "2026-06-07T02:00:00Z".parse().unwrap()
    }

    #[test]
    fn relative_7d() {
        let result = parse_time_expr("7d", fixed_now(), &cst()).unwrap();
        let expected = fixed_now().timestamp_millis() - 7 * 24 * 3600 * 1000;
        assert_eq!(result, expected);
    }

    #[test]
    fn relative_24h() {
        let result = parse_time_expr("24h", fixed_now(), &cst()).unwrap();
        let expected = fixed_now().timestamp_millis() - 24 * 3600 * 1000;
        assert_eq!(result, expected);
    }

    #[test]
    fn relative_30m() {
        let result = parse_time_expr("30m", fixed_now(), &cst()).unwrap();
        let expected = fixed_now().timestamp_millis() - 30 * 60 * 1000;
        assert_eq!(result, expected);
    }

    #[test]
    fn named_today_cst() {
        let result = parse_time_expr("today", fixed_now(), &cst()).unwrap();
        // 2026-06-07T02:00:00Z = 2026-06-07T10:00:00+08:00
        // today start in CST = 2026-06-07T00:00:00+08:00 = 2026-06-06T16:00:00Z
        let expected: DateTime<Utc> = "2026-06-06T16:00:00Z".parse().unwrap();
        assert_eq!(result, expected.timestamp_millis());
    }

    #[test]
    fn named_yesterday_cst() {
        let result = parse_time_expr("yesterday", fixed_now(), &cst()).unwrap();
        // yesterday in CST = 2026-06-06T00:00:00+08:00 = 2026-06-05T16:00:00Z
        let expected: DateTime<Utc> = "2026-06-05T16:00:00Z".parse().unwrap();
        assert_eq!(result, expected.timestamp_millis());
    }

    #[test]
    fn named_week_cst() {
        // 2026-06-07 is Sunday, week start (Monday) = 2026-06-01
        let result = parse_time_expr("week", fixed_now(), &cst()).unwrap();
        // Monday 2026-06-01T00:00:00+08:00 = 2026-05-31T16:00:00Z
        let expected: DateTime<Utc> = "2026-05-31T16:00:00Z".parse().unwrap();
        assert_eq!(result, expected.timestamp_millis());
    }

    #[test]
    fn absolute_date() {
        let result = parse_time_expr("2026-06-01", fixed_now(), &cst()).unwrap();
        // 2026-06-01T00:00:00+08:00 = 2026-05-31T16:00:00Z
        let expected: DateTime<Utc> = "2026-05-31T16:00:00Z".parse().unwrap();
        assert_eq!(result, expected.timestamp_millis());
    }

    #[test]
    fn absolute_iso8601_with_tz() {
        let result = parse_time_expr("2026-06-07T10:00:00+08:00", fixed_now(), &cst()).unwrap();
        let expected: DateTime<Utc> = "2026-06-07T02:00:00Z".parse().unwrap();
        assert_eq!(result, expected.timestamp_millis());
    }

    #[test]
    fn absolute_datetime_no_tz() {
        let result = parse_time_expr("2026-06-07T10:00:00", fixed_now(), &cst()).unwrap();
        // Interpreted as local (CST): 2026-06-07T10:00:00+08:00 = 2026-06-07T02:00:00Z
        let expected: DateTime<Utc> = "2026-06-07T02:00:00Z".parse().unwrap();
        assert_eq!(result, expected.timestamp_millis());
    }

    #[test]
    fn invalid_format() {
        let err = parse_time_expr("last month", fixed_now(), &cst()).unwrap_err();
        assert!(matches!(err, TimeExprError::InvalidFormat(_)));
        assert!(err.to_string().contains("Valid formats:"));
    }

    #[test]
    fn invalid_empty() {
        assert!(parse_time_expr("", fixed_now(), &cst()).is_err());
    }

    #[test]
    fn invalid_zero_duration() {
        assert!(parse_time_expr("0d", fixed_now(), &cst()).is_err());
    }

    #[test]
    fn invalid_negative_duration() {
        assert!(parse_time_expr("-5h", fixed_now(), &cst()).is_err());
    }
}
