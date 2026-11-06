//! Horloge injectée : le seul « maintenant » du serveur (AD-11).

use std::sync::Mutex;

use domain::Timestamp;

pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

/// Horloge système, tronquée à la seconde.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_datetime(chrono::Utc::now()).expect("system clock within years 0000-9999")
    }
}

/// Horloge de test : renvoie l'instant fixé, déplaçable à la main.
#[derive(Debug)]
pub struct FixedClock(Mutex<Timestamp>);

impl FixedClock {
    pub fn new(at: Timestamp) -> Self {
        Self(Mutex::new(at))
    }

    pub fn set(&self, at: Timestamp) {
        *self.0.lock().expect("clock mutex") = at;
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        *self.0.lock().expect("clock mutex")
    }
}
