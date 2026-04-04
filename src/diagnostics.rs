use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticPhase {
    Scan,
    Parse,
    Type,
    Codegen,
    Verify,
}

impl fmt::Display for DiagnosticPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Scan => "scan",
            Self::Parse => "parse",
            Self::Type => "type",
            Self::Codegen => "codegen",
            Self::Verify => "verify",
        };
        write!(f, "{}", name)
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub phase: DiagnosticPhase,
    pub file: String,
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct Diagnostics {
    entries: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn emit_line(&mut self, phase: DiagnosticPhase, file: &str, line: usize, message: &str) {
        self.entries.push(Diagnostic {
            phase,
            file: file.to_string(),
            line: Some(line),
            message: message.to_string(),
        });
    }

    pub fn emit(&mut self, phase: DiagnosticPhase, file: &str, message: &str) {
        self.entries.push(Diagnostic {
            phase,
            file: file.to_string(),
            line: None,
            message: message.to_string(),
        });
    }

    pub fn extend(&mut self, mut other: Diagnostics) {
        self.entries.append(&mut other.entries);
    }

    pub fn render(&self) -> String {
        if self.entries.is_empty() {
            return "Compilation failed without diagnostics.".to_string();
        }

        let mut out = String::new();
        for entry in &self.entries {
            if let Some(line) = entry.line {
                out.push_str(&format!(
                    "{}:{}: error[{}]: {}\n",
                    entry.file, line, entry.phase, entry.message
                ));
            } else {
                out.push_str(&format!(
                    "{}: error[{}]: {}\n",
                    entry.file, entry.phase, entry.message
                ));
            }
        }
        out.push_str(&format!("{} error(s) emitted.", self.entries.len()));
        out
    }
}
