//! Commandes : chaque écriture ouvre une transaction sur le pool d'écriture, applique les règles de
//! `domain` avec le `ReferenceSet` pris une fois, écrit et commit (AD-16).

pub mod create_event;
