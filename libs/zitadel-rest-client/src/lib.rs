#![doc = include_str!("../README.md")]

mod auth;
mod client;
mod error;
mod pagination;

pub use auth::AdminCredential;
pub use client::ZitadelClient;
pub use error::ZitadelError;
pub use pagination::{Page, PageRequest};

pub type Result<T> = std::result::Result<T, ZitadelError>;
