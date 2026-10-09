use super::super::value::{Link, Value};

pub fn link(arguments: Vec<Value>) -> Result<Value, String> {
    let mut arguments = arguments.into_iter();
    let Some(target) = arguments.next() else {
        return Err("«link» ждёт хотя бы путь или ссылку".to_owned());
    };
    let display = arguments.next().unwrap_or(Value::Null);
    let display = match display {
        Value::Null => None,
        value => {
            let text = value.text();
            (!text.trim().is_empty()).then_some(text)
        }
    };
    Ok(match target {
        Value::Link(existing) => Value::Link(Link {
            target: existing.target,
            display: display.or(existing.display),
        }),
        Value::Null => Value::Null,
        value => Value::Link(Link {
            target: value.text(),
            display,
        }),
    })
}

pub fn default(arguments: Vec<Value>) -> Value {
    let mut arguments = arguments.into_iter();
    let value = arguments.next().unwrap_or(Value::Null);
    let fallback = arguments.next().unwrap_or(Value::Null);
    if value.truthy() || matches!(value, Value::Number(_) | Value::Bool(false)) {
        value
    } else {
        fallback
    }
}

pub fn choice(arguments: Vec<Value>) -> Value {
    let mut arguments = arguments.into_iter();
    let condition = arguments.next().unwrap_or(Value::Null);
    let yes = arguments.next().unwrap_or(Value::Null);
    let no = arguments.next().unwrap_or(Value::Null);
    if condition.truthy() {
        yes
    } else {
        no
    }
}

pub fn object(arguments: Vec<Value>) -> Value {
    let mut fields = Vec::with_capacity(arguments.len() / 2);
    let mut arguments = arguments.into_iter();
    while let Some(key) = arguments.next() {
        let value = arguments.next().unwrap_or(Value::Null);
        fields.push((key.text(), value));
    }
    Value::Object(fields)
}

pub fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::Progress(_) => "progress",
        Value::Date(_) => "date",
        Value::Duration(_) => "duration",
        Value::Text(_) => "string",
        Value::Link(_) => "link",
        Value::List(_) => "array",
        Value::Object(_) => "object",
    }
}
