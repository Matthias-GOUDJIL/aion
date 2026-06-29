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

    #[error("Not Found: {kind} '{name}' is not defined{suggestion}")]
    NotFound {
        kind: String,
        name: String,
        suggestion: String,
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

    #[error("LLVM error: {message}")]
    Inkwell {
        message: String,
        line: usize,
        col: usize,
        snippet: Option<String>,
    },

    #[error("IO error: {message}")]
    Io {
        message: String,
        line: usize,
        col: usize,
        snippet: Option<String>,
    },

    #[error("Import error: {message}")]
    Import {
        message: String,
        line: usize,
        col: usize,
        snippet: Option<String>,
    },

    #[error("internal compiler error: {message}")]
    Internal {
        message: String,
        line: usize,
        col: usize,
        snippet: Option<String>,
    },

    #[error("warning: {message}")]
    Warning {
        message: String,
        line: usize,
        col: usize,
        snippet: Option<String>,
    },
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

    /// Construct an `Internal` error with the typed location prefix applied
    /// (the `#[error("internal compiler error: {message}")]` format already
    /// prepends the label, so the caller passes the raw message). Use this
    /// instead of building the struct variant directly so the line/col default
    /// is centralized and future layout changes hit one site. #40.
    pub fn internal(message: impl Into<String>) -> Self {
        CompileError::Internal {
            message: message.into(),
            line: 0,
            col: 0,
            snippet: None,
        }
    }

    pub fn inkwell(message: impl Into<String>) -> Self {
        CompileError::Inkwell {
            message: message.into(),
            line: 0,
            col: 0,
            snippet: None,
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        CompileError::Io {
            message: message.into(),
            line: 0,
            col: 0,
            snippet: None,
        }
    }

    pub fn import(message: impl Into<String>) -> Self {
        CompileError::Import {
            message: message.into(),
            line: 0,
            col: 0,
            snippet: None,
        }
    }

    pub fn warning(message: impl Into<String>, line: usize, col: usize) -> Self {
        CompileError::Warning {
            message: message.into(),
            line,
            col,
            snippet: None,
        }
    }

    /// `NotFound` carrying an optional "did you mean X?" suggestion. Pass an
    /// empty `suggestion` string when no close match was found. #40.
    pub fn not_found(
        kind: impl Into<String>,
        name: impl Into<String>,
        suggestion: impl Into<String>,
        line: usize,
        col: usize,
    ) -> Self {
        let mut s = suggestion.into();
        if !s.is_empty() {
            s = format!(" (did you mean '{}'?)", s);
        }
        CompileError::NotFound {
            kind: kind.into(),
            name: name.into(),
            suggestion: s,
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
            | CompileError::InvalidOperator { line, .. }
            | CompileError::Inkwell { line, .. }
            | CompileError::Io { line, .. }
            | CompileError::Import { line, .. }
            | CompileError::Internal { line, .. }
            | CompileError::Warning { line, .. } => *line,
        };

        if line > 0 {
            let snippet = extract_snippet(source, line, self.col());
            match &mut self {
                CompileError::Type { snippet: s, .. }
                | CompileError::Unsafe { snippet: s, .. }
                | CompileError::NotFound { snippet: s, .. }
                | CompileError::NotFunction { snippet: s, .. }
                | CompileError::InvalidOperator { snippet: s, .. }
                | CompileError::Inkwell { snippet: s, .. }
                | CompileError::Io { snippet: s, .. }
                | CompileError::Import { snippet: s, .. }
                | CompileError::Internal { snippet: s, .. }
                | CompileError::Warning { snippet: s, .. } => *s = snippet,
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
            | CompileError::InvalidOperator { col, .. }
            | CompileError::Inkwell { col, .. }
            | CompileError::Io { col, .. }
            | CompileError::Import { col, .. }
            | CompileError::Internal { col, .. }
            | CompileError::Warning { col, .. } => *col,
        }
    }

    /// True if this error is non-fatal (only the `Warning` variant today).
    /// Used by the driver to print to stderr without halting compilation. #40.
    pub fn is_warning(&self) -> bool {
        matches!(self, CompileError::Warning { .. })
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
    for (i, text) in lines.iter().enumerate().skip(start).take(end - start) {
        let line_num = i + 1;
        let prefix = if line_num == line { " > " } else { "   " };
        result.push_str(&format!("{}{:4} | {}\n", prefix, line_num, text));
        if line_num == line && col > 0 {
            let offset = 6 + col;
            result.push_str(&format!("{}^\n", " ".repeat(offset)));
        }
    }
    Some(result)
}

/// Returns the closest match from `candidates` to `name`, or `None` if no
/// candidate is "close enough" (Levenshtein distance <= 3 AND within the
/// worst case of (len/3)). Used by the type checker for "did you mean X?"
/// suggestions on undefined-variable / function / field errors. #40.
pub fn suggest_closest<'a, I>(name: &str, candidates: I) -> Option<String>
where
    I: IntoIterator<Item = &'a String>,
{
    let mut best: Option<(usize, String)> = None;
    for c in candidates {
        if c == name {
            continue;
        }
        // Skip qualified names — only suggest simple names the user typed.
        let token = c.rsplit('.').next().unwrap_or(c);
        if token.is_empty() {
            continue;
        }
        let d = levenshtein(name, token);
        let threshold = (name.len().max(token.len()) / 3).max(1);
        // Require both a small absolute distance and a relative threshold so
        // that short names (len 1-2) don't match wildly unrelated candidates.
        if d <= 3 && d <= threshold {
            match &best {
                Some((bd, _)) if d >= *bd => {}
                _ => best = Some((d, token.to_string())),
            }
        }
    }
    best.map(|(_, s)| s)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr: Vec<usize> = vec![0; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

impl From<inkwell::builder::BuilderError> for CompileError {
    fn from(e: inkwell::builder::BuilderError) -> Self {
        CompileError::inkwell(e.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::{levenshtein, suggest_closest};

    #[test]
    fn levenshtein_basic() {
        // Substitution, insertion, deletion each cost 1.
        assert_eq!(levenshtein("greeb", "greet"), 1); // substitution
        assert_eq!(levenshtein("nme", "name"), 1); // insertion
        assert_eq!(levenshtein("greet_word", "greet_world"), 1); // deletion
        assert_eq!(levenshtein("grety", "greet"), 2); // two substitutions
    }

    #[test]
    fn suggest_closest_simple() {
        let cs: Vec<String> = vec!["greet".to_string()];
        assert_eq!(suggest_closest("greeb", &cs), Some("greet".to_string()));
        // Distance-2 typo rejected by the default 3-edit / (len/3) threshold.
        assert_eq!(suggest_closest("grety", &cs), None);
    }
}
