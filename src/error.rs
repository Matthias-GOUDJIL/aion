#[derive(Debug, Clone, thiserror::Error)]
pub enum CompileError {
    #[error("Type Error: {message}")]
    Type {
        message: String,
        line: usize,
        col: usize,
        snippet: Option<String>,
    },

    #[error("Unsafe Error: {message}")]
    Unsafe {
        message: String,
        line: usize,
        col: usize,
        snippet: Option<String>,
    },

    #[error("Not Found: {kind} '{name}' is not defined")]
    NotFound {
        kind: String,
        name: String,
        line: usize,
        col: usize,
        snippet: Option<String>,
    },

    #[error("'{name}' is not a function")]
    NotFunction {
        name: String,
        line: usize,
        col: usize,
        snippet: Option<String>,
    },

    #[error("Incompatible operator {op} for types {left} and {right}")]
    InvalidOperator {
        op: String,
        left: String,
        right: String,
        line: usize,
        col: usize,
        snippet: Option<String>,
    },

    #[error("LLVM error: {0}")]
    Inkwell(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Import error: {0}")]
    Import(String),

    #[error("{0}")]
    Internal(String),
}

impl CompileError {
    pub fn new(message: impl Into<String>, line: usize, col: usize) -> Self {
        CompileError::Type {
            message: message.into(),
            line,
            col,
            snippet: None,
        }
    }

    pub fn with_snippet(mut self, source: &str) -> Self {
        let line = match &self {
            CompileError::Type { line, .. }
            | CompileError::Unsafe { line, .. }
            | CompileError::NotFound { line, .. }
            | CompileError::NotFunction { line, .. }
            | CompileError::InvalidOperator { line, .. } => *line,
            _ => return self,
        };

        if line > 0 {
            let snippet = extract_snippet(source, line, self.col());
            match &mut self {
                CompileError::Type { snippet: s, .. }
                | CompileError::Unsafe { snippet: s, .. }
                | CompileError::NotFound { snippet: s, .. }
                | CompileError::NotFunction { snippet: s, .. }
                | CompileError::InvalidOperator { snippet: s, .. } => *s = snippet,
                _ => {}
            }
        }
        self
    }

    pub fn col(&self) -> usize {
        match self {
            CompileError::Type { col, .. }
            | CompileError::Unsafe { col, .. }
            | CompileError::NotFound { col, .. }
            | CompileError::NotFunction { col, .. }
            | CompileError::InvalidOperator { col, .. } => *col,
            _ => 0,
        }
    }
}

fn extract_snippet(source: &str, line: usize, col: usize) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    if line == 0 || line > lines.len() {
        return None;
    }

    let start = if line > 1 { line - 2 } else { 0 };
    let end = (line + 1).min(lines.len());

    let mut result = String::new();
    for i in start..end {
        let line_num = i + 1;
        let prefix = if line_num == line { " > " } else { "   " };
        result.push_str(&format!("{}{:4} | {}\n", prefix, line_num, lines[i]));
        if line_num == line && col > 0 {
            let offset = 6 + col;
            result.push_str(&format!("{}^\n", " ".repeat(offset)));
        }
    }
    Some(result)
}

impl From<inkwell::builder::BuilderError> for CompileError {
    fn from(e: inkwell::builder::BuilderError) -> Self {
        CompileError::Inkwell(e.to_string())
    }
}
