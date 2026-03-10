#![allow(dead_code)]
#![allow(clippy::new_without_default)]

pub mod audio;
pub mod cli;
pub mod config;
pub mod detection;
pub mod error;
#[cfg(feature = "gui")]
pub mod gui;
pub mod inspect;
pub mod sanitization;
pub mod ui;
pub mod verification;
