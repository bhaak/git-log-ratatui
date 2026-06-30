use git2::Time;

const SECS_PER_DAY: i64 = 86400;
const SECS_PER_HOUR: i64 = 3600;
const SECS_PER_MIN: i64 = 60;
const DAYS_PER_YEAR: i64 = 365;
const DAYS_PER_LEAP_YEAR: i64 = 366;
const EPOCH_YEAR: i64 = 1970;

pub fn time_to_string(time: Time) -> String {
    let mut ts = time.seconds();
    let days = ts / SECS_PER_DAY;
    ts %= SECS_PER_DAY;
    let hour = ts / SECS_PER_HOUR;
    ts %= SECS_PER_HOUR;
    let minute = ts / SECS_PER_MIN;

    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        year, month, day, hour, minute
    )
}

pub fn time_to_string_with_seconds(time: Time) -> String {
    let second = time.seconds() % SECS_PER_MIN;
    format!("{}:{:02}", time_to_string(time), second)
}

fn days_to_ymd(mut days: i64) -> (i64, u32, u32) {
    let mut year = EPOCH_YEAR;
    loop {
        let days_in_year = if is_leap(year) {
            DAYS_PER_LEAP_YEAR
        } else {
            DAYS_PER_YEAR
        };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let month_days = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u32;
    for &md in month_days.iter() {
        if days < md as i64 {
            break;
        }
        days -= md as i64;
        month += 1;
    }
    (year, month, (days + 1) as u32)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Time;

    #[test]
    fn test_days_to_ymd_epoch() {
        let (year, month, day) = days_to_ymd(0);
        assert_eq!(year, 1970);
        assert_eq!(month, 1);
        assert_eq!(day, 1);
    }

    #[test]
    fn test_days_to_ymd_2024_01_01() {
        let (year, month, day) = days_to_ymd(19723);
        assert_eq!(year, 2024);
        assert_eq!(month, 1);
        assert_eq!(day, 1);
    }

    #[test]
    fn test_days_to_ymd_2024_12_31() {
        let (year, month, day) = days_to_ymd(19723 + 365);
        assert_eq!(year, 2024);
        assert_eq!(month, 12);
        assert_eq!(day, 31);
    }

    #[test]
    fn test_time_to_string_epoch() {
        let time = Time::new(0, 0);
        assert_eq!(time_to_string(time), "1970-01-01 00:00");
        assert_eq!(time_to_string_with_seconds(time), "1970-01-01 00:00:00");
    }

    #[test]
    fn test_is_leap() {
        assert!(is_leap(2024));
        assert!(!is_leap(2023));
        assert!(is_leap(2000));
        assert!(!is_leap(1900));
    }
}
