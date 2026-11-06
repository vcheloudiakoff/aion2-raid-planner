//! Noyau pur du planificateur : aucune I/O, aucune horloge système, aucun aléa non injecté.

pub mod reference;
mod timestamp;
pub mod validate;

pub use timestamp::{Timestamp, TimestampError};
