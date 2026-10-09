use super::functions;
use super::parser;

pub fn syntax_help() -> String {
    format!(
        "{GRAMMAR}\n\nФункции: {}\n\n{EXAMPLES}",
        functions::available()
    )
}

pub fn check_query(text: &str) -> Result<(), String> {
    parser::parse(text).map(|_| ())
}

const GRAMMAR: &str = "\
Блок ```dataview в заметке считается внутри приложения. Строение запроса:

  TABLE [WITHOUT ID] [выражение [AS \"Колонка\"], …]
  LIST  [WITHOUT ID] [выражение]
  TASK  [WITHOUT ID]

Дальше в любом порядке:

  TITLE \"Подпись\"            подпись над выдачей
  EVERY 30s                   пересчитывать блок сам, каждые 30 секунд (5 minutes, 1 hour)
  FROM \"Папка\" | #тег | [[Заметка]] | outgoing([[Заметка]]) | once
  WHERE выражение             условие отбора
  SORT выражение [ASC|DESC]   порядок
  LIMIT число                 сколько строк оставить
  GROUP BY выражение [AS имя] схлопнуть строки в группы: ключ и список rows
  FLATTEN выражение [AS имя]  дать значению имя, список размножит строку

Части применяются в том порядке, в каком написаны: LIMIT перед SORT действительно обрежет
до сортировки. Источник once даёт одну пустую строку — для таблиц, которые не о заметках.

Поля файла: file.link, file.path, file.name, file.folder, file.ctime, file.mtime, file.size,
file.tags. Поля заметки — любое имя из frontmatter или из текста вида «Ключ:: значение».
Поля задачи (только в TASK): text, completed, line.

Даты: date(\"2024-10-07\"), today, now, dur(1 week), дата ± длительность, дата - дата.
Периоды: day, week, month, season, year, ever, span(начало, конец); по ним считают
bar(период), percent(период), remaining(период), count(даты, период), previous(период).";

const EXAMPLES: &str = "\
Примеры:

  TABLE WITHOUT ID link(file.link, title) AS \"Заметка\", rating AS \"Оценка\"
  FROM #книга
  WHERE rating >= 4
  SORT rating DESC
  LIMIT 10

  TASK TITLE \"Невыполненные задачи\"
  WHERE !completed
  SORT file.mtime DESC

  TABLE WITHOUT ID item.name AS \"Период\", bar(item.span) AS \"Прогресс\"
  FROM once
  EVERY 30s
  FLATTEN list(
      object(\"name\", \"День\", \"span\", day),
      object(\"name\", \"Год\", \"span\", year)
  ) AS item";

#[cfg(test)]
mod tests {
    use super::{check_query, syntax_help};

    #[test]
    fn help_lists_the_functions_the_build_actually_has() {
        let help = syntax_help();
        assert!(help.contains("dateformat"));
        assert!(help.contains("EVERY 30s"));
        assert!(help.contains("TASK"));
    }

    #[test]
    fn a_query_is_checked_without_touching_the_vault() {
        assert!(check_query("TASK TITLE \"Дела\" WHERE !completed").is_ok());
        let error = check_query("TASK AS TITLE \"Дела\"").expect_err("разбор не удался");
        assert!(!error.is_empty());
    }
}
