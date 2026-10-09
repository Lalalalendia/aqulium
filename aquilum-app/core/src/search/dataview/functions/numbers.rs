use super::first;
use super::super::value::Value;

pub fn numeric(arguments: &[Value], apply: fn(f64) -> f64) -> Value {
    match first(arguments).as_number() {
        Some(number) => Value::Number(apply(number)),
        None => Value::Null,
    }
}

pub fn numbers(value: &Value) -> Vec<f64> {
    match value {
        Value::List(items) => items.iter().filter_map(Value::as_number).collect(),
        other => other.as_number().into_iter().collect(),
    }
}

pub fn extreme(value: &Value, smallest: bool) -> Value {
    let items = match value {
        Value::List(items) => items.clone(),
        Value::Null => Vec::new(),
        other => vec![other.clone()],
    };
    items
        .into_iter()
        .filter(|item| !matches!(item, Value::Null))
        .reduce(|left, right| {
            let take_right = if smallest {
                right.compare(&left).is_lt()
            } else {
                right.compare(&left).is_gt()
            };
            if take_right {
                right
            } else {
                left
            }
        })
        .unwrap_or(Value::Null)
}

pub fn average(value: &Value) -> Value {
    let found = numbers(value);
    if found.is_empty() {
        return Value::Null;
    }
    Value::Number(found.iter().sum::<f64>() / found.len() as f64)
}
