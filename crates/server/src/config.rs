//! Configuration lue une seule fois au démarrage, fail-fast (`configuration.md` de la spec epic 1).
//!
//! Pas de chargeur `.env` : la source est l'environnement du processus, ou une fonction injectée en
//! test. Une erreur ne cite jamais que le nom de la variable, jamais sa valeur.

use std::fmt;
use std::time::Duration;

/// Port d'écoute du conteneur, toujours 8080 (AD-8) ; `PORT` ne règle que le port hôte du compose.
pub const LISTEN_PORT: u16 = 8080;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Port hôte (`127.0.0.1:${PORT}` dans le compose). Le binaire écoute toujours sur 8080.
    pub port: u16,
    /// Seule source d'URL absolue (AD-9), sans `/` final.
    pub public_base_url: String,
    pub form_token_secret: FormTokenSecret,
    pub client_ip_header: String,
    pub trust_client_ip_header: bool,
    pub min_submit_delay: Duration,
    pub max_events_per_hour: u32,
    pub max_candidatures_per_hour: u32,
    pub rate_create: Rate,
    pub rate_candidature: Rate,
    pub rate_write: Rate,
    pub rate_read: Rate,
    pub rust_log: String,
}

/// Clé HMAC du jeton de formulaire, au moins 32 octets ; jamais affichée.
#[derive(Clone, PartialEq, Eq)]
pub struct FormTokenSecret(Vec<u8>);

impl FormTokenSecret {
    pub const MIN_BYTES: usize = 32;

    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for FormTokenSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FormTokenSecret(<redacted>)")
    }
}

/// Quota : `count` requêtes par `per`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    pub count: u32,
    pub per: Duration,
}

impl Rate {
    fn parse(s: &str) -> Option<Self> {
        let (count, unit) = s.split_once('/')?;
        let count: u32 = count.parse().ok().filter(|&c| c > 0)?;
        let per = match unit {
            "s" => Duration::from_secs(1),
            "min" => Duration::from_secs(60),
            "h" => Duration::from_secs(3600),
            _ => return None,
        };
        Some(Self { count, per })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable {0}")]
    Missing(&'static str),
    #[error("invalid value for environment variable {0}")]
    Invalid(&'static str),
}

impl ConfigError {
    pub fn variable(&self) -> &'static str {
        match self {
            Self::Missing(v) | Self::Invalid(v) => v,
        }
    }
}

impl Config {
    /// Lit l'environnement du processus.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_source(|name| std::env::var(name).ok())
    }

    /// Lit une source quelconque ; une valeur vide compte comme absente.
    pub fn from_source(source: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let get = |name: &str| source(name).filter(|v| !v.is_empty());
        let required = |name: &'static str| get(name).ok_or(ConfigError::Missing(name));
        fn parse_or<T>(
            value: Option<String>,
            name: &'static str,
            default: T,
            parse: impl Fn(&str) -> Option<T>,
        ) -> Result<T, ConfigError> {
            match value {
                None => Ok(default),
                Some(v) => parse(&v).ok_or(ConfigError::Invalid(name)),
            }
        }

        let port = parse_or(get("PORT"), "PORT", 8080, |v| {
            v.parse().ok().filter(|&p: &u16| p > 0)
        })?;

        let public_base_url = required("PUBLIC_BASE_URL")?;
        let public_base_url =
            parse_base_url(&public_base_url).ok_or(ConfigError::Invalid("PUBLIC_BASE_URL"))?;

        let secret = required("FORM_TOKEN_SECRET")?;
        if secret.len() < FormTokenSecret::MIN_BYTES {
            return Err(ConfigError::Invalid("FORM_TOKEN_SECRET"));
        }
        let form_token_secret = FormTokenSecret(secret.into_bytes());

        let client_ip_header = parse_or(
            get("CLIENT_IP_HEADER"),
            "CLIENT_IP_HEADER",
            "x-client-ip".to_owned(),
            |v| {
                let v = v.to_ascii_lowercase();
                let token = v.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-');
                (token && v != "x-forwarded-for").then_some(v)
            },
        )?;

        let trust_client_ip_header = parse_or(
            get("TRUST_CLIENT_IP_HEADER"),
            "TRUST_CLIENT_IP_HEADER",
            true,
            |v| match v {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
        )?;

        let min_submit_delay = parse_or(
            get("MIN_SUBMIT_DELAY_MS"),
            "MIN_SUBMIT_DELAY_MS",
            Duration::from_millis(3000),
            // Au moins une heure : tout jeton serait trop jeune ou expiré.
            |v| {
                v.parse()
                    .ok()
                    .filter(|&ms: &u64| ms < 3_600_000)
                    .map(Duration::from_millis)
            },
        )?;
        let positive = |v: &str| v.parse().ok().filter(|&n: &u32| n > 0);
        let max_events_per_hour = parse_or(
            get("MAX_EVENTS_PER_HOUR"),
            "MAX_EVENTS_PER_HOUR",
            30,
            positive,
        )?;
        let max_candidatures_per_hour = parse_or(
            get("MAX_CANDIDATURES_PER_HOUR"),
            "MAX_CANDIDATURES_PER_HOUR",
            300,
            positive,
        )?;

        let rate = |name: &'static str, count: u32, secs: u64| {
            let default = Rate {
                count,
                per: Duration::from_secs(secs),
            };
            parse_or(get(name), name, default, Rate::parse)
        };
        let rate_create = rate("RATE_CREATE", 5, 3600)?;
        let rate_candidature = rate("RATE_CANDIDATURE", 20, 3600)?;
        let rate_write = rate("RATE_WRITE", 120, 60)?;
        let rate_read = rate("RATE_READ", 600, 60)?;

        let rust_log = parse_or(get("RUST_LOG"), "RUST_LOG", "info".to_owned(), |v| {
            (production_log_levels(v) && tracing_subscriber::EnvFilter::try_new(v).is_ok())
                .then(|| v.to_owned())
        })?;

        Ok(Self {
            port,
            public_base_url,
            form_token_secret,
            client_ip_header,
            trust_client_ip_header,
            min_submit_delay,
            max_events_per_hour,
            max_candidatures_per_hour,
            rate_create,
            rate_candidature,
            rate_write,
            rate_read,
            rust_log,
        })
    }
}

/// Configuration de test : variables obligatoires remplies, `extra` par-dessus.
#[cfg(test)]
pub(crate) fn test_config(extra: &[(&str, &str)]) -> Config {
    Config::from_source(|k| {
        extra
            .iter()
            .rev()
            .find(|(name, _)| *name == k)
            .map(|(_, v)| (*v).to_owned())
            .or_else(|| match k {
                "PUBLIC_BASE_URL" => Some("https://raid.example.org".to_owned()),
                "FORM_TOKEN_SECRET" => Some("0123456789abcdef0123456789abcdef".to_owned()),
                _ => None,
            })
    })
    .expect("valid test configuration")
}

/// Chaque directive doit finir sur `off`, `error`, `warn` ou `info` : `debug` et `trace` sont
/// interdits en production (AD-15), et une cible nue (`server`) vaut `trace` pour `EnvFilter`.
fn production_log_levels(v: &str) -> bool {
    v.split(',').filter(|d| !d.trim().is_empty()).all(|d| {
        let level = d.rsplit('=').next().unwrap_or_default().trim();
        ["off", "error", "warn", "info"]
            .iter()
            .any(|l| level.eq_ignore_ascii_case(l))
    })
}

/// `http://` ou `https://`, un hôte, ni query, ni fragment, ni espace ; `/` final retiré.
fn parse_base_url(v: &str) -> Option<String> {
    let rest = v
        .strip_prefix("https://")
        .or_else(|| v.strip_prefix("http://"))?;
    let host = rest.split('/').next().unwrap_or("");
    let clean =
        !host.is_empty() && !v.contains(['?', '#', '@']) && v.bytes().all(|c| c.is_ascii_graphic());
    clean.then(|| v.trim_end_matches('/').to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_source(|k| map.get(k).cloned())
    }

    fn minimal() -> Vec<(&'static str, &'static str)> {
        vec![
            ("PUBLIC_BASE_URL", "https://raid.example.org/"),
            ("FORM_TOKEN_SECRET", SECRET),
        ]
    }

    #[test]
    fn defaults_apply() {
        let c = load(&minimal()).unwrap();
        assert_eq!(c.port, 8080);
        assert_eq!(c.public_base_url, "https://raid.example.org");
        assert_eq!(c.client_ip_header, "x-client-ip");
        assert!(c.trust_client_ip_header);
        assert_eq!(c.min_submit_delay, Duration::from_millis(3000));
        assert_eq!(
            (c.max_events_per_hour, c.max_candidatures_per_hour),
            (30, 300)
        );
        assert_eq!(
            c.rate_create,
            Rate {
                count: 5,
                per: Duration::from_secs(3600)
            }
        );
        assert_eq!(
            c.rate_candidature,
            Rate {
                count: 20,
                per: Duration::from_secs(3600)
            }
        );
        assert_eq!(
            c.rate_write,
            Rate {
                count: 120,
                per: Duration::from_secs(60)
            }
        );
        assert_eq!(
            c.rate_read,
            Rate {
                count: 600,
                per: Duration::from_secs(60)
            }
        );
        assert_eq!(c.rust_log, "info");
    }

    #[test]
    fn overrides_apply() {
        let mut vars = minimal();
        vars.extend([
            ("PORT", "9000"),
            ("CLIENT_IP_HEADER", "X-Real-Visitor"),
            ("TRUST_CLIENT_IP_HEADER", "false"),
            ("MIN_SUBMIT_DELAY_MS", "0"),
            ("RATE_READ", "10/s"),
            ("RUST_LOG", "warn,server=info"),
        ]);
        let c = load(&vars).unwrap();
        assert_eq!(c.port, 9000);
        assert_eq!(c.client_ip_header, "x-real-visitor");
        assert!(!c.trust_client_ip_header);
        assert_eq!(c.min_submit_delay, Duration::ZERO);
        assert_eq!(
            c.rate_read,
            Rate {
                count: 10,
                per: Duration::from_secs(1)
            }
        );
        assert_eq!(c.rust_log, "warn,server=info");
    }

    #[test]
    fn required_variables_fail_fast() {
        assert_eq!(
            load(&[("FORM_TOKEN_SECRET", SECRET)]),
            Err(ConfigError::Missing("PUBLIC_BASE_URL"))
        );
        assert_eq!(
            load(&[
                ("PUBLIC_BASE_URL", "https://raid.example.org"),
                ("FORM_TOKEN_SECRET", "")
            ]),
            Err(ConfigError::Missing("FORM_TOKEN_SECRET"))
        );
    }

    #[test]
    fn short_secret_is_rejected_without_echoing_it() {
        let short = "too-short-secret-value-31-bytes";
        assert_eq!(short.len(), 31);
        let err = load(&[
            ("PUBLIC_BASE_URL", "https://raid.example.org"),
            ("FORM_TOKEN_SECRET", short),
        ])
        .unwrap_err();
        assert_eq!(err, ConfigError::Invalid("FORM_TOKEN_SECRET"));
        assert!(!err.to_string().contains(short));
    }

    #[test]
    fn malformed_values_name_the_variable() {
        for (name, bad) in [
            ("PUBLIC_BASE_URL", "raid.example.org"),
            ("PUBLIC_BASE_URL", "https://"),
            ("PUBLIC_BASE_URL", "https://raid.example.org/?x=1"),
            ("PORT", "0"),
            ("PORT", "eighty"),
            ("PORT", "70000"),
            ("CLIENT_IP_HEADER", "X-Forwarded-For"),
            ("CLIENT_IP_HEADER", "bad header"),
            ("TRUST_CLIENT_IP_HEADER", "yes"),
            ("MIN_SUBMIT_DELAY_MS", "-1"),
            ("MIN_SUBMIT_DELAY_MS", "3600000"),
            ("MIN_SUBMIT_DELAY_MS", "99999999"),
            ("MAX_EVENTS_PER_HOUR", "0"),
            ("MAX_CANDIDATURES_PER_HOUR", "many"),
            ("RATE_CREATE", "5"),
            ("RATE_CANDIDATURE", "20/day"),
            ("RATE_WRITE", "0/min"),
            ("RATE_READ", "/min"),
            ("RUST_LOG", "server=nope=bad"),
            ("RUST_LOG", "debug"),
            ("RUST_LOG", "warn,server=trace"),
            ("RUST_LOG", "server=DEBUG"),
            ("RUST_LOG", "server"),
        ] {
            let mut vars: Vec<(&str, &str)> =
                minimal().into_iter().filter(|(k, _)| *k != name).collect();
            vars.push((name, bad));
            let err = load(&vars).unwrap_err();
            assert_eq!(err, ConfigError::Invalid(name), "{name}={bad}");
            assert!(err.to_string().contains(name));
            assert!(
                !err.to_string().contains(bad),
                "{name} error echoes its value"
            );
        }
    }

    #[test]
    fn min_submit_delay_just_below_one_hour_is_accepted() {
        let mut vars = minimal();
        vars.push(("MIN_SUBMIT_DELAY_MS", "3599999"));
        assert_eq!(
            load(&vars).unwrap().min_submit_delay,
            Duration::from_millis(3_599_999)
        );
    }

    #[test]
    fn debug_never_prints_the_secret() {
        let c = load(&minimal()).unwrap();
        assert!(!format!("{c:?}").contains(SECRET));
    }
}
