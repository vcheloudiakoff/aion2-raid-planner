//! Noyau pur du planificateur : aucune I/O, aucune horloge système, aucun aléa non injecté.

mod timestamp;

pub use timestamp::{Timestamp, TimestampError};
