#![doc = include_str!("../README.md")]

mod auth;
mod client;
mod error;
mod pagination;

pub mod app;
pub mod project;
pub mod project_role;
pub mod user;
pub mod user_grant;
pub mod user_key;

pub use auth::AdminCredential;
pub use client::ZitadelClient;
pub use error::ZitadelError;
pub use pagination::{Page, PageRequest};

pub type Result<T> = std::result::Result<T, ZitadelError>;
