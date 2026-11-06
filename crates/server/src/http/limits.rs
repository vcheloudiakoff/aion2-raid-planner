//! Frein anti-abus (AD-7) : classes de route, limiteur par IP visiteur, disjoncteur global.
//!
//! Tout vit en mémoire et repart de zéro au redémarrage : aucune table, aucune IP écrite. La classe
//! `nokey` (`/api/health`, assets, `index.html`, repli 404 de `/api`) ne passe ni par l'extracteur
//! d'IP ni par le limiteur ; les autres classes passent par [`enforce`], posé en `route_layer`.

use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::HeaderName;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use governor::clock::Clock as _;
use governor::{DefaultKeyedRateLimiter, Quota};

use crate::config::{Config, Rate};
use crate::http::client_ip::{ClientIpSource, IpKey};
use crate::http::errors::{Code, Problem};
use crate::infra::clock::Clock;

/// Classe d'une route, qui fixe son quota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteClass {
    /// Création d'un événement (POST, jeton de formulaire exigé).
    Create,
    /// Dépôt d'une candidature (POST, jeton de formulaire exigé).
    Candidature,
    /// Autres écritures.
    Write,
    /// Lectures de l'API.
    Read,
    /// Exemptée de clé et de limiteur.
    NoKey,
}

impl RouteClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Candidature => "candidature",
            Self::Write => "write",
            Self::Read => "read",
            Self::NoKey => "nokey",
        }
    }
}

type Limiter = DefaultKeyedRateLimiter<IpKey>;

/// `count` jetons d'un coup, puis un jeton rendu toutes les `per / count`.
fn limiter(rate: Rate) -> Limiter {
    let count = NonZeroU32::new(rate.count).expect("config rejects a zero rate");
    let period = (rate.per / rate.count).max(Duration::from_nanos(1));
    let quota = Quota::with_period(period)
        .expect("period is non-zero")
        .allow_burst(count);
    Limiter::keyed(quota)
}

/// Délai annoncé en secondes entières, au moins 1.
pub fn retry_after_secs(wait: Duration) -> u64 {
    let secs = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
    secs.max(1)
}

/// État anti-abus partagé : source de l'IP, un limiteur par classe à clé, disjoncteur.
pub struct Limits {
    ip: ClientIpSource,
    create: Limiter,
    candidature: Limiter,
    write: Limiter,
    read: Limiter,
    pub breaker: Breaker,
}

impl Limits {
    pub fn new(config: &Config, clock: Arc<dyn Clock>) -> Self {
        let header = HeaderName::from_bytes(config.client_ip_header.as_bytes())
            .expect("config validates CLIENT_IP_HEADER as a header token");
        Self {
            ip: ClientIpSource {
                header,
                trust_header: config.trust_client_ip_header,
            },
            create: limiter(config.rate_create),
            candidature: limiter(config.rate_candidature),
            write: limiter(config.rate_write),
            read: limiter(config.rate_read),
            breaker: Breaker::new(
                clock,
                config.max_events_per_hour,
                config.max_candidatures_per_hour,
            ),
        }
    }

    fn limiter(&self, class: RouteClass) -> Option<&Limiter> {
        match class {
            RouteClass::Create => Some(&self.create),
            RouteClass::Candidature => Some(&self.candidature),
            RouteClass::Write => Some(&self.write),
            RouteClass::Read => Some(&self.read),
            RouteClass::NoKey => None,
        }
    }

    /// Résout l'IP puis consomme un jeton du quota de la classe.
    pub fn check(&self, class: RouteClass, parts: &Parts) -> Result<(), Problem> {
        let Some(limiter) = self.limiter(class) else {
            return Ok(());
        };
        let key = self.ip.resolve(parts).inspect_err(|_| {
            tracing::warn!(
                class = class.as_str(),
                "client ip header missing or invalid"
            );
        })?;
        limiter.check_key(&key).map_err(|not_until| {
            let wait = not_until.wait_time_from(limiter.clock().now());
            tracing::info!(class = class.as_str(), "rate limited");
            Problem::new(Code::LimitRateLimited).retry_after(retry_after_secs(wait))
        })
    }

    /// Oublie les clés revenues à plein quota : la carte ne grossit pas sans borne.
    pub fn retain_recent(&self) {
        for limiter in [&self.create, &self.candidature, &self.write, &self.read] {
            limiter.retain_recent();
            limiter.shrink_to_fit();
        }
    }

    /// Nombre de clés suivies, toutes classes confondues.
    pub fn tracked_keys(&self) -> usize {
        [&self.create, &self.candidature, &self.write, &self.read]
            .iter()
            .map(|l| l.len())
            .sum()
    }
}

/// État du middleware : les limites partagées et la classe des routes qu'il couvre.
#[derive(Clone)]
pub struct Guard {
    limits: Arc<Limits>,
    class: RouteClass,
}

impl Guard {
    pub fn new(limits: &Arc<Limits>, class: RouteClass) -> Self {
        Self {
            limits: limits.clone(),
            class,
        }
    }
}

/// Middleware à poser par `route_layer(middleware::from_fn_with_state(Guard::new(..), enforce))`.
pub async fn enforce(State(guard): State<Guard>, req: Request, next: Next) -> Response {
    let (parts, body) = req.into_parts();
    if let Err(problem) = guard.limits.check(guard.class, &parts) {
        return problem.into_response();
    }
    next.run(Request::from_parts(parts, body)).await
}

/// Détente du disjoncteur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trip {
    Events,
    Candidatures,
}

/// Fenêtre d'une heure glissante.
pub const BREAKER_WINDOW_SECS: i64 = 3600;

/// Disjoncteur global à deux détentes, compté sur une heure glissante selon la [`Clock`] injectée.
/// `check` avant l'écriture, `record` après le commit.
///
/// `check` et `record` prennent le verrou séparément : deux commandes concurrentes pourraient toutes
/// deux passer `check` et dépasser `MAX_*_PER_HOUR`. Une commande appelle donc `check` puis `record`
/// à l'intérieur de sa transaction d'écriture : le pool d'écriture à une seule connexion sérialise
/// les transactions, ce qui rend la paire atomique en pratique.
pub struct Breaker {
    clock: Arc<dyn Clock>,
    events: Window,
    candidatures: Window,
}

struct Window {
    max: u32,
    /// Instants (secondes Unix) des écritures commises dans la fenêtre.
    recent: Mutex<VecDeque<i64>>,
}

impl Window {
    fn new(max: u32) -> Self {
        Self {
            max,
            recent: Mutex::new(VecDeque::new()),
        }
    }

    /// Retire ce qui est sorti de la fenêtre, et ce qui est daté du futur (horloge revenue en
    /// arrière) ; rend le plus ancien instant restant.
    fn prune(recent: &mut VecDeque<i64>, now: i64) -> Option<i64> {
        recent.retain(|&t| t <= now && now - t < BREAKER_WINDOW_SECS);
        recent.iter().copied().min()
    }
}

impl Breaker {
    pub fn new(clock: Arc<dyn Clock>, max_events: u32, max_candidatures: u32) -> Self {
        Self {
            clock,
            events: Window::new(max_events),
            candidatures: Window::new(max_candidatures),
        }
    }

    fn window(&self, trip: Trip) -> &Window {
        match trip {
            Trip::Events => &self.events,
            Trip::Candidatures => &self.candidatures,
        }
    }

    /// Seuil atteint : 503 `limit.paused`, `Retry-After` jusqu'à la sortie de la plus ancienne écriture.
    pub fn check(&self, trip: Trip) -> Result<(), Problem> {
        let now = self.clock.now().unix_seconds();
        let window = self.window(trip);
        let mut recent = window.recent.lock().expect("breaker mutex");
        let oldest = Window::prune(&mut recent, now);
        if recent.len() < window.max as usize {
            return Ok(());
        }
        let wait = oldest
            .map_or(1, |t| t + BREAKER_WINDOW_SECS - now)
            .clamp(1, BREAKER_WINDOW_SECS);
        tracing::warn!(trip = ?trip, "circuit breaker open");
        Err(Problem::new(Code::LimitPaused).retry_after(wait as u64))
    }

    /// Compte une écriture commise.
    pub fn record(&self, trip: Trip) {
        let now = self.clock.now().unix_seconds();
        let mut recent = self.window(trip).recent.lock().expect("breaker mutex");
        Window::prune(&mut recent, now);
        recent.push_back(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_rounds_up_to_at_least_one_second() {
        assert_eq!(retry_after_secs(Duration::ZERO), 1);
        assert_eq!(retry_after_secs(Duration::from_millis(1)), 1);
        assert_eq!(retry_after_secs(Duration::from_millis(1000)), 1);
        assert_eq!(retry_after_secs(Duration::from_millis(1001)), 2);
        assert_eq!(retry_after_secs(Duration::from_secs(1800)), 1800);
    }

    #[test]
    fn route_class_names() {
        let all = [
            RouteClass::Create,
            RouteClass::Candidature,
            RouteClass::Write,
            RouteClass::Read,
            RouteClass::NoKey,
        ];
        let names: Vec<_> = all.iter().map(|c| c.as_str()).collect();
        assert_eq!(names, ["create", "candidature", "write", "read", "nokey"]);
    }
}
