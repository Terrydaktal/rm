#[derive(Debug)]
pub struct AppError {
    pub code: u8,
    pub message: String,
}

impl AppError {
    pub fn operation(message: impl Into<String>) -> Self {
        Self {
            code: 1,
            message: message.into(),
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            code: 2,
            message: message.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
