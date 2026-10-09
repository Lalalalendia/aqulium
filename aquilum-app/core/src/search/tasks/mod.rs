pub mod parse;
pub mod repository;

pub use parse::Task;
pub use repository::{index_document, open_schema, read as read_tasks, remove_document};
