//! Noyau pur du planificateur : aucune I/O, aucune horloge système, aucun aléa non injecté.

pub mod candidature;
pub mod counts;
pub mod event;
pub mod pseudo_srv;
pub mod reference;
pub mod room;
mod timestamp;
pub mod validate;

pub use timestamp::{Timestamp, TimestampError};
