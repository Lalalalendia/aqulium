use super::constants::{NANOS_PER_DAY, SPAN_END, SPAN_START};
use super::value::Value;
use crate::search::note_date;

pub fn of(start: i64, end: i64) -> Value {
    Value::Object(vec![
        (SPAN_START.to_owned(), Value::Date(start)),
        (SPAN_END.to_owned(), Value::Date(end)),
    ])
}

pub fn bounds(value: &Value) -> Option<(i64, i64)> {
    match value {
        Value::Object(fields) => {
            let find = |wanted: &str| {
                fields.iter().find_map(|(key, found)| match found {
                    Value::Date(nanos) if key.eq_ignore_ascii_case(wanted) => Some(*nanos),
                    _ => None,
                })
            };
            Some((find(SPAN_START)?, find(SPAN_END)?))
        }
        Value::Date(nanos) => Some((*nanos, *nanos)),
        _ => None,
    }
}

pub fn passed(value: &Value) -> Option<f64> {
    let (start, end) = bounds(value)?;
    if end <= start {
        return Some(100.0);
    }
    let now = note_date::now();
    let part = (now - start) as f64 / (end - start) as f64 * 100.0;
    Some(part.clamp(0.0, 100.0))
}

pub fn covers(value: &Value, moment: i64) -> Option<bool> {
    let (start, end) = bounds(value)?;
    Some(moment >= start && moment < end)
}

pub fn previous(value: &Value) -> Option<Value> {
    let (start, end) = bounds(value)?;
    if start <= 0 || end <= start {
        return None;
    }
    Some(of(start.saturating_sub(end - start), start))
}

pub fn left(value: &Value) -> Option<i64> {
    let (_, end) = bounds(value)?;
    Some((end - note_date::now()).max(0))
}

pub fn builtin(name: &str) -> Option<Value> {
    let now = note_date::now();
    let (year, month, day, _) = note_date::to_civil(now);
    Some(match name.to_lowercase().as_str() {
        "day" => {
            let start = note_date::from_civil(year, month, day, 0);
            of(start, start + NANOS_PER_DAY)
        }
        "week" => {
            let start = note_date::from_civil(year, month, day, 0)
                - (note_date::weekday(now) - 1) * NANOS_PER_DAY;
            of(start, start + 7 * NANOS_PER_DAY)
        }
        "month" => of(
            note_date::from_civil(year, month, 1, 0),
            month_start(year, month + 1),
        ),
        "season" => {
            let (first, length) = season_of(month);
            let first_year = if first == 12 && month < 3 { year - 1 } else { year };
            of(
                month_start(first_year, first),
                month_start(first_year, first + length),
            )
        }
        "year" => of(month_start(year, 1), month_start(year + 1, 1)),
        "ever" => of(0, now),
        "season_name" => Value::Text(
            match season_of(month).0 {
                3 => "Весна",
                6 => "Лето",
                9 => "Осень",
                _ => "Зима",
            }
            .to_owned(),
        ),
        _ => return None,
    })
}

fn season_of(month: i64) -> (i64, i64) {
    match month {
        3..=5 => (3, 3),
        6..=8 => (6, 3),
        9..=11 => (9, 3),
        _ => (12, 3),
    }
}

fn month_start(year: i64, month: i64) -> i64 {
    let shifted = year * 12 + (month - 1);
    note_date::from_civil(shifted.div_euclid(12), shifted.rem_euclid(12) + 1, 1, 0)
}

#[cfg(test)]
mod tests {
    use super::super::constants::NANOS_PER_DAY;
    use super::{bounds, builtin, month_start, of, passed, previous};
    use crate::search::note_date;

    #[test]
    fn a_span_knows_its_ends() {
        let span = of(100, 300);
        assert_eq!(bounds(&span), Some((100, 300)));
    }

    #[test]
    fn a_bare_date_is_a_span_of_no_length() {
        assert_eq!(bounds(&super::Value::Date(42)), Some((42, 42)));
    }

    #[test]
    fn the_progress_of_a_finished_span_is_full_and_of_a_future_one_is_empty() {
        let now = note_date::now();
        assert_eq!(passed(&of(now - 2 * NANOS_PER_DAY, now - NANOS_PER_DAY)), Some(100.0));
        assert_eq!(passed(&of(now + NANOS_PER_DAY, now + 2 * NANOS_PER_DAY)), Some(0.0));
    }

    #[test]
    fn a_month_starts_on_the_first_and_ends_on_the_next_first() {
        let (start, end) = bounds(&builtin("month").expect("месяц есть")).expect("границы");
        assert_eq!(note_date::to_civil(start).2, 1);
        assert_eq!(note_date::to_civil(end).2, 1);
        assert!(end > start);
    }

    #[test]
    fn a_week_starts_on_monday() {
        let (start, _) = bounds(&builtin("week").expect("неделя есть")).expect("границы");
        assert_eq!(note_date::weekday(start), 1);
    }

    #[test]
    fn a_month_after_december_is_january_of_the_next_year() {
        assert_eq!(month_start(2024, 13), month_start(2025, 1));
        assert_eq!(note_date::to_civil(month_start(2024, 13)).0, 2025);
    }

    #[test]
    fn every_builtin_period_covers_the_present_moment() {
        let now = note_date::now();
        for name in ["day", "week", "month", "season", "year"] {
            let (start, end) = bounds(&builtin(name).expect(name)).expect("границы");
            assert!(start <= now && now < end, "период «{name}» не содержит сейчас");
        }
    }

    #[test]
    fn ever_and_zero_spans_have_no_previous_period() {
        let ever = builtin("ever").expect("ever есть");
        assert_eq!(previous(&ever), None);
        assert_eq!(previous(&of(0, 100)), None);
        assert_eq!(previous(&of(200, 100)), None);
        assert_eq!(previous(&of(100, 100)), None);
    }
}
