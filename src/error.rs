use std::fmt;

#[derive(Debug, Clone)]
pub struct CompileError {
    pub message: String,
    pub line: usize,
    pub col: usize,
    pub snippet: Option<String>,
}

impl CompileError {
    pub fn new(message: impl Into<String>, line: usize, col: usize) -> Self {
        Self { message: message.into(), line, col, snippet: None }
    }

    pub fn with_snippet(mut self, source: &str) -> Self {
        if self.line > 0 {
            self.snippet = extract_snippet(source, self.line, self.col);
        }
        self
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "error at line {}, col {}: {}", self.line, self.col, self.message)?;
        } else {
            write!(f, "error: {}", self.message)?;
        }
        if let Some(ref snippet) = self.snippet {
            write!(f, "\n{}", snippet)?;
        }
        Ok(())
    }
}

impl std::error::Error for CompileError {}

fn extract_snippet(source: &str, line: usize, col: usize) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    if line == 0 || line > lines.len() { return None; }

    let start = if line > 1 { line - 2 } else { 0 };
    let end = (line + 1).min(lines.len());

    let mut result = String::new();
    for i in start..end {
        let line_num = i + 1;
        let prefix = if line_num == line { " > " } else { "   " };
        result.push_str(&format!("{}{:4} | {}\n", prefix, line_num, lines[i]));
        if line_num == line && col > 0 {
            let offset = 6 + col; // " > " + "NNN | " + col
            result.push_str(&format!("{}^\n", " ".repeat(offset)));
        }
    }
    Some(result)
}
