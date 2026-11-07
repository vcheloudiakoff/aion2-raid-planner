//! `Pseudo[SRV]` (AD-6, CONTACT) : la forme exacte sous laquelle le jeu affiche un joueur, composée
//! ici et nulle part ailleurs, et le signalement des doublons à l'organisateur.
//!
//! Un doublon est signalé, jamais résolu : les deux candidatures restent, l'organisateur choisit
//! (AD-4, AD-13). Aucune table ni index ne porte `Pseudo[SRV]` : le calcul se fait à la lecture,
//! dans un seul événement.

use std::collections::HashMap;

use crate::candidature::CandidatureStatus;
use crate::reference::ReferenceSet;

/// `"{pseudo}[{tag}]"`, avec le `tag` du serveur dans la référence, entrées désactivées comprises.
/// Un serveur absent de la référence prend son id comme tag (repli d'AD-6).
pub fn pseudo_srv(reference: &ReferenceSet, pseudo: &str, server: &str) -> String {
    let tag = reference
        .servers
        .iter()
        .find(|s| s.id == server)
        .map_or(server, |s| s.tag.as_str());
    format!("{pseudo}[{tag}]")
}

/// Ce que le signalement lit d'une candidature : pseudo, id de serveur et statut.
pub type DuplicateKey<'a> = (&'a str, &'a str, CandidatureStatus);

/// Une candidature compte pour les doublons tant que l'organisateur peut encore la retenir.
const fn is_live(status: CandidatureStatus) -> bool {
    matches!(
        status,
        CandidatureStatus::Pending | CandidatureStatus::Accepted
    )
}

/// Pour chaque candidature, dans l'ordre reçu : vrai si elle est `pending` ou `accepted` et que son
/// couple (pseudo sans casse, serveur) apparaît au moins deux fois parmi les `pending` et `accepted`
/// de la liste. Toujours faux pour `rejected` et `withdrawn`.
pub fn duplicates(candidatures: &[DuplicateKey<'_>]) -> Vec<bool> {
    let key = |pseudo: &str, server: &str| (pseudo.to_lowercase(), server.to_owned());
    let mut counts: HashMap<(String, String), usize> = HashMap::new();
    for &(pseudo, server, status) in candidatures {
        if is_live(status) {
            *counts.entry(key(pseudo, server)).or_default() += 1;
        }
    }
    candidatures
        .iter()
        .map(|&(pseudo, server, status)| {
            is_live(status) && counts.get(&key(pseudo, server)).is_some_and(|&n| n >= 2)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::{Faction, FactionEntry, Factions, Label, Server};
    use CandidatureStatus::*;

    fn label(en: &str) -> Label {
        Label {
            en: en.into(),
            fr: None,
        }
    }

    fn reference() -> ReferenceSet {
        let server = |id: &str, tag: &str, enabled: bool| Server {
            id: id.into(),
            region: "r1".into(),
            faction: Faction::Elyos,
            tag: tag.into(),
            label: label(id),
            enabled,
        };
        ReferenceSet {
            regions: vec![],
            factions: Factions {
                elyos: FactionEntry { label: label("E") },
                asmodian: FactionEntry { label: label("A") },
            },
            servers: vec![server("s1", "One", true), server("s_off", "Off", false)],
            room_label_charset: "R".into(),
            room_label_max: 4,
            instances: vec![],
            classes: vec![],
            roles: vec![],
        }
    }

    #[test]
    fn pseudo_srv_uses_the_reference_tag() {
        assert_eq!(pseudo_srv(&reference(), "Nyra", "s1"), "Nyra[One]");
        // Désactivé depuis : son tag reste celui du jeu.
        assert_eq!(pseudo_srv(&reference(), "Nyra", "s_off"), "Nyra[Off]");
        // Pseudo Unicode gardé tel quel.
        assert_eq!(pseudo_srv(&reference(), "Élodie2", "s1"), "Élodie2[One]");
    }

    #[test]
    fn pseudo_srv_falls_back_to_the_server_id() {
        assert_eq!(pseudo_srv(&reference(), "Nyra", "gone"), "Nyra[gone]");
    }

    #[test]
    fn duplicates_ignore_case_and_count_only_pending_and_accepted() {
        assert_eq!(
            duplicates(&[
                ("Abc", "s1", Pending),
                ("abc", "s1", Accepted),
                ("Abc", "s1", Rejected),
            ]),
            [true, true, false]
        );
        // Une seule ligne vivante : pas de doublon, même avec des lignes refusées ou retirées.
        assert_eq!(
            duplicates(&[
                ("Abc", "s1", Pending),
                ("ABC", "s1", Rejected),
                ("abc", "s1", Withdrawn),
            ]),
            [false, false, false]
        );
        // Autre serveur : pas le même joueur.
        assert_eq!(
            duplicates(&[("Abc", "s1", Accepted), ("Abc", "s2", Accepted)]),
            [false, false]
        );
        // Sans casse au sens Unicode.
        assert_eq!(
            duplicates(&[("Élodie", "s1", Pending), ("éLODIE", "s1", Pending)]),
            [true, true]
        );
        // Trois lignes vivantes : toutes signalées, aucune retirée de la liste.
        assert_eq!(
            duplicates(&[
                ("x", "s1", Accepted),
                ("y", "s1", Accepted),
                ("X", "s1", Pending),
                ("x", "s1", Accepted),
            ]),
            [true, false, true, true]
        );
        assert!(duplicates(&[]).is_empty());
    }
}
