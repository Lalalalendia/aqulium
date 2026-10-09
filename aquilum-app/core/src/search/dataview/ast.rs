use super::value::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    And,
    Or,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Not,
    Negate,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Literal(Value),
    Variable(String),
    Property {
        base: Box<Expr>,
        name: String,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Call {
        name: String,
        arguments: Vec<Expr>,
    },
    Unary {
        operator: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        operator: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Lambda {
        parameters: Vec<String>,
        body: Box<Expr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Once,
    Folder(String),
    Tag(String),
    LinksTo(String),
    LinksFrom(String),
    And(Box<Source>, Box<Source>),
    Or(Box<Source>, Box<Source>),
    Not(Box<Source>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub title: String,
    pub value: Expr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Ascending,
    Descending,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SortKey {
    pub value: Expr,
    pub direction: Direction,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Clause {
    Where(Expr),
    Sort(Vec<SortKey>),
    Limit(usize),
    GroupBy { value: Expr, name: String },
    Flatten { value: Expr, name: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Table {
        columns: Vec<Column>,
        show_path: bool,
    },
    List {
        value: Option<Expr>,
        show_path: bool,
    },
    Tasks {
        show_path: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub shape: Shape,
    pub source: Option<Source>,
    pub clauses: Vec<Clause>,
    pub title: Option<String>,
    pub refresh: Option<u32>,
}
