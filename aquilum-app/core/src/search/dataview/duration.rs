use super::constants::{
    DURATION_FINISHED, DURATION_UNITS, DURATION_ZERO_SECONDS, NANOS_PER_DAY, NANOS_PER_HOUR,
    NANOS_PER_MINUTE, NANOS_PER_MONTH, NANOS_PER_SECOND,
};
use crate::search::note_date;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Duration {
    pub months: i64,
    pub nanos: i64,
}

impl Duration {
    fn zero() -> Self {
        Self::default()
    }

    pub fn is_zero(&self) -> bool {
        self.months == 0 && self.nanos == 0
    }

    pub fn negate(self) -> Self {
        Self {
            months: -self.months,
            nanos: -self.nanos,
        }
    }

    pub fn add(self, other: Self) -> Self {
        Self {
            months: self.months + other.months,
            nanos: self.nanos + other.nanos,
        }
    }

    pub fn scale(self, factor: f64) -> Self {
        Self {
            months: (self.months as f64 * factor) as i64,
            nanos: (self.nanos as f64 * factor) as i64,
        }
    }

    pub fn approximate_nanos(&self) -> i64 {
        self.months * NANOS_PER_MONTH + self.nanos
    }

    pub fn text(&self) -> String {
        let total = self.approximate_nanos();
        if total == 0 {
            return DURATION_FINISHED.to_owned();
        }
        let sign = if total < 0 { "-" } else { "" };
        let total = total.abs();
        for (size, forms) in DURATION_UNITS {
            let amount = total / size;
            if amount > 0 {
                return format!("{sign}{amount} {}", plural(amount, forms));
            }
        }
        format!("{sign}{DURATION_ZERO_SECONDS}")
    }
}

fn plural(amount: i64, forms: [&str; 3]) -> &str {
    let tens = amount % 100;
    if (11..=14).contains(&tens) {
        return forms[2];
    }
    match amount % 10 {
        1 => forms[0],
        2..=4 => forms[1],
        _ => forms[2],
    }
}

pub fn parse(text: &str) -> Option<Duration> {
    let lowered = text.trim().to_lowercase();
    if lowered.is_empty() {
        return None;
    }
    let mut total = Duration::zero();
    let mut found = false;
    let mut rest = lowered.as_str();

    while !rest.is_empty() {
        rest = rest.trim_start_matches([' ', ',', '\t', '\n']);
        if rest.is_empty() {
            break;
        }
        let digits = rest
            .find(|symbol: char| !symbol.is_ascii_digit() && symbol != '.' && symbol != '-')
            .unwrap_or(rest.len());
        let amount: f64 = rest[..digits].parse().ok()?;
        rest = rest[digits..].trim_start();
        let letters = rest
            .find(|symbol: char| !symbol.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        if letters == 0 {
            return None;
        }
        total = total.add(unit(&rest[..letters], amount)?);
        rest = &rest[letters..];
        found = true;
    }

    found.then_some(total)
}

fn unit(name: &str, amount: f64) -> Option<Duration> {
    let name = if name.len() > 1 { name.trim_end_matches('s') } else { name };
    let months = |count: f64| Duration {
        months: count as i64,
        nanos: 0,
    };
    let nanos = |size: i64| Duration {
        months: 0,
        nanos: (amount * size as f64) as i64,
    };
    Some(match name {
        "year" | "yr" | "y" => months(amount * 12.0),
        "month" | "mo" | "mon" => months(amount),
        "week" | "wk" | "w" => nanos(7 * NANOS_PER_DAY),
        "day" | "d" => nanos(NANOS_PER_DAY),
        "hour" | "hr" | "h" => nanos(NANOS_PER_HOUR),
        "minute" | "min" | "m" => nanos(NANOS_PER_MINUTE),
        "second" | "sec" | "s" => nanos(NANOS_PER_SECOND),
        _ => return None,
    })
}

pub fn shift(date: i64, duration: Duration) -> i64 {
    let shifted = if duration.months == 0 {
        date
    } else {
        let (year, month, day, seconds) = note_date::to_civil(date);
        let total = year * 12 + (month - 1) + duration.months;
        let (year, month) = (total.div_euclid(12), total.rem_euclid(12) + 1);
        let day = day.min(note_date::days_in_month(year, month));
        note_date::from_civil(year, month, day, seconds)
    };
    shifted + duration.nanos
}

pub fn between(later: i64, earlier: i64) -> Duration {
    Duration {
        months: 0,
        nanos: later - earlier,
    }
}

#[cfg(test)]
mod tests {
    use super::{between, parse, shift, Duration};
    use crate::search::note_date;

    fn date(text: &str) -> i64 {
        note_date::parse(text).expect("дата разобрана")
    }

    #[test]
    fn a_single_letter_unit_is_not_a_plural() {
        assert_eq!(parse("30s"), parse("30 seconds"));
        assert_eq!(parse("2h"), parse("2 hours"));
    }

    #[test]
    fn reads_the_forms_dataview_writes() {
        assert_eq!(parse("1 week"), parse("7 days"));
        assert_eq!(parse("2 weeks"), parse("14d"));
        assert_eq!(parse("1 year"), Some(Duration { months: 12, nanos: 0 }));
        assert_eq!(parse("1y 6mo"), Some(Duration { months: 18, nanos: 0 }));
        assert!(parse("1 фунт").is_none());
        assert!(parse("неделя").is_none());
    }

    #[test]
    fn a_month_shift_stays_inside_the_month() {
        assert_eq!(
            shift(date("2024-01-31"), parse("1 month").unwrap()),
            date("2024-02-29")
        );
        assert_eq!(
            shift(date("2024-03-31"), parse("-1 month").unwrap()),
            date("2024-02-29")
        );
    }

    #[test]
    fn a_week_back_is_seven_days_back() {
        assert_eq!(
            shift(date("2024-10-07"), parse("1 week").unwrap().negate()),
            date("2024-09-30")
        );
    }

    #[test]
    fn the_difference_of_two_dates_is_days() {
        let difference = between(date("2024-10-08"), date("2024-10-07"));
        assert_eq!(difference.text(), "1 день");
    }

    #[test]
    fn only_the_largest_unit_is_printed() {
        assert_eq!(parse("1y 2mo").unwrap().text(), "1 год");
        assert_eq!(parse("57 years").unwrap().text(), "57 лет");
        assert_eq!(parse("90 minutes").unwrap().text(), "1 час");
        assert_eq!(parse("3 days 5 hours").unwrap().text(), "3 дня");
        assert_eq!(parse("45 seconds").unwrap().text(), "45 сек.");
        assert_eq!(Duration { months: 0, nanos: 500_000_000 }.text(), "0 сек.");
        assert_eq!(Duration::zero().text(), "конец");
    }

    #[test]
    fn the_russian_ending_follows_the_number() {
        assert_eq!(parse("21 years").unwrap().text(), "21 год");
        assert_eq!(parse("22 years").unwrap().text(), "22 года");
        assert_eq!(parse("11 years").unwrap().text(), "11 лет");
        assert_eq!(parse("2 days").unwrap().text(), "2 дня");
    }
}
