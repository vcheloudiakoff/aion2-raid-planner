//! Binaire unique du planificateur : API sous `/api/*` et SPA embarquée (AD-1).
//! Couches `http → app → infra` ; le noyau pur vit dans le crate `domain`.

pub mod app;
pub mod config;
pub mod healthcheck;
pub mod http;
pub mod infra;
