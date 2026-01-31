use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Email and password are required")]
    MissingCredentials,

    #[error("All fields are required")]
    MissingRegistrationFields,

    #[error("Invalid email format")]
    InvalidEmail,

    #[error("Password must be at least {min_len} characters")]
    WeakPassword { min_len: usize },
}
