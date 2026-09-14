use std::fmt;

#[derive(Debug)]
pub enum EvaluatorError {
    JavaScriptError(String),
    ParseError(String),
    VariableError(String),
    TypeError(String),
    StateError(String),
    FunctionError(String),
}

impl fmt::Display for EvaluatorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EvaluatorError::JavaScriptError(msg) => write!(f, "JavaScript runtime error: {}", msg),
            EvaluatorError::ParseError(msg) => write!(f, "Expression parsing error: {}", msg),
            EvaluatorError::VariableError(msg) => write!(f, "Variable resolution error: {}", msg),
            EvaluatorError::TypeError(msg) => write!(f, "Type conversion error: {}", msg),
            EvaluatorError::StateError(msg) => write!(f, "State management error: {}", msg),
            EvaluatorError::FunctionError(msg) => write!(f, "Function registration error: {}", msg),
        }
    }
}

impl std::error::Error for EvaluatorError {}

pub type Result<T> = std::result::Result<T, EvaluatorError>;
