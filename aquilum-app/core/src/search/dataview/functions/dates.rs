use super::first;
use super::super::dateformat;
use super::super::duration;
use super::super::value::Value;
use crate::search::note_date;
pub fn date(value: &Value) -> Value {
    match value {
        Value::Date(nanos) => Value::Date(*nanos),
        Value::Text(text) => match text.trim().to_lowercase().as_str() {
            "today" => Value::Date(note_date::start_of_day(note_date::now())),
            "now" => Value::Date(note_date::now()),
            trimmed => match note_date::parse(trimmed) {
                Some(nanos) => Value::Date(nanos),
                None => Value::Null,
            },
        },
        _ => Value::Null,
    }
}

pub fn duration_value(value: &Value) -> Value {
    match value {
        Value::Duration(length) => Value::Duration(*length),
        other => match duration::parse(&other.text()) {
            Some(length) => Value::Duration(length),
            None => Value::Null,
        },
    }
}

pub fn date_format(arguments: &[Value]) -> Value {
    let pattern = arguments.get(1).map(Value::text).unwrap_or_default();
    match date(first(arguments)) {
        Value::Date(nanos) => Value::Text(dateformat::format(nanos, &pattern)),
        _ => Value::Null,
    }
}

pub fn striptime(value: &Value) -> Value {
    match date(value) {
        Value::Date(nanos) => Value::Date(note_date::start_of_day(nanos)),
        _ => Value::Null,
    }
}

pub fn as_date(value: &Value) -> Option<i64> {
    match date(value) {
        Value::Date(nanos) => Some(nanos),
        _ => None,
    }
}
