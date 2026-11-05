# Forged idea — aion2-raid-planner

Forge du 2026-11-05, mode attaque, verdict **HARDENED**. Entrées : `docs/intent-dump.md`,
`docs/domaine-aion2.md`, captures du client global fournies par Vincent. Journal : `.memlog.md` à côté.
Ce fichier remplace, point par point, ce que l'intent dump disait de différent (voir son §12).

## En une phrase

Un **système de booking** de raids Sanctuaire pour une légion et son cercle : un événement à créneaux,
des candidatures (pseudo, serveur, classe, CP, disponibilités), une **composition** proposée par un
solveur ou faite à la main, des titulaires et un banc, et un **rendez-vous dans une salle du jeu** dont
l'outil donne le nom et les 10 slots. Sans compte, sans contact stocké, sans fiabilité ni historique.
Une vitrine.

## Verrous (ne pas rouvrir)

- **Rendez-vous en jeu (Q6).** La liste des salles de Sanctuaire du client global est régionale, tous
  serveurs et deux factions, avec recherche par nom. Un raid de l'outil se joue dans une salle que
  l'organisateur crée à l'heure verrouillée (Faction : All). L'outil génère un nom de salle unique et
  rend les 10 slots (deux groupes de 5, rôle par slot). L'identifiant en jeu d'un joueur est `Pseudo[SRV]`.
- **Candidature.** Pseudo, serveur (la faction s'en déduit : un serveur = une faction), classe, CP déclaré
  en nombre, disponibilités par créneau, préférence de créneau. Pas de Gear Score, pas de Discord par joueur.
  Le CP n'est pas vérifié : pré-filtre ; la vérification se fait en jeu à la demande de rejoint.
- **Vitrine.** Aucune information de fiabilité, de réputation ni d'historique joueur, jamais. Les absents
  s'absorbent par un banc de remplaçants et une re-confirmation par lien à un délai choisi par
  l'organisateur. La confiance et le vocal se font sur le Discord du raid.
- **Identité (Q2).** Liens porteurs de droits : lien public (lire, candidater), lien secret d'organisateur
  affiché une fois, lien secret de participant ; mémorisés par le navigateur, copiables pour changer
  d'appareil. **Aucune récupération** : lien perdu = réinscription ou expiration.
- **Anti-abus (Q1).** Limitation de débit par IP (24 h max), disjoncteur global de création, champs bornés,
  honeypot et délai, expiration automatique, tri des candidatures par l'organisateur, lien
  d'administration secret dans `.env`. Pas de défi humain ni de bouton signaler ; preuve de travail
  auto-hébergée derrière un drapeau. Pertes acceptées : un soir de création perdu, un nettoyage manuel.
  Fil de déclenchement : nettoyage routinier → preuve de travail + revoir la liste publique ; sinon l'outil
  ne vaut pas la peine.
- **Public (Q9, Q4).** La légion de Vincent et son cercle (≈15 actifs par jour, ≤120 membres). Le public du
  site est un pari écrit, pas une promesse. Un raid est listé par défaut avec validation ; option « lien seul ».
- **Contact (Q3).** Quatre gestes sans canal : `Pseudo[SRV]` à copier (le principal), lien Discord du raid
  visible des acceptés, message prêt à coller dans Discord, l'inscrit rouvre son lien et lit son statut.
  Aucune notification en v1.
- **Hébergement (Q5).** L'edge déjà en production : VPS européen, tunnel WireGuard sortant depuis le NAS,
  garde anti-fuite sur le NAS, conteneurs sur localhost, en-têtes à l'edge. Nom d'hôte gratuit pour le pilote
  dans une zone qui ne pointe pas vers la maison, URL publique en configuration ; vrai domaine si concluant,
  avec redirection. L'edge laisse passer les écritures : l'anti-abus est une limitation de débit.
- **Modèle.** Un événement avec 1 à N créneaux produit K groupes (créneau, 10 titulaires, banc, salle).
  K = 1 et tri manuel = booking simple ; K > 1 et proposition = cas légion. Deux liens par événement :
  direct (accepté d'office) et public (à valider). Bornes min/max par classe fixées par l'organisateur.
- **Composition.** Solveur déterministe, sans IA ni réseau. Deux modes : « composer automatiquement
  selon les critères » ou à la main. Contraintes dures : disponible au créneau, bornes par classe, 10 par
  groupe, un joueur par groupe. Objectif : le plus de groupes complets, puis équilibre des CP entre groupes,
  préférence en départage ; option « le plus fort d'abord ». Proposition corrigeable, jamais imposée.
  Construite en dernier dans la v1.
- **Langue (Q10).** Textes externalisés dès la v1, livrés en français et en anglais.

## Rejeté, et pourquoi

- Pseudo Discord par inscrit : liste de cibles scrapable, contredit « données minimales ».
- Réputation, note, historique, liste noire : exigent une identité persistante (= un compte) et font de
  Vincent un arbitre.
- Récupération de lien (email, code, question) : chaque porte de sortie devient un compte.
- Défi humain tiers (Turnstile) : script tiers, interdit par la CSP stricte.
- Bouton signaler : spammable sans compte, arbitre.
- Modèle de langage pour composer : coût récurrent, tiers, données de joueurs sorties, résultat non
  reproductible.
- Entité « campagne » séparée : un événement à N créneaux et K groupes suffit.
- Matchmaking d'inconnus au hasard : reste un non-objectif ; ce n'est pas la composition d'un vivier connu.
- Liste de joueurs seuls avec contact (LFG inversé) : hors v1, contredit « données minimales ».
- Notifications : aucun canal sans email ni push ; hors v1.
- Sous-domaine de la zone du NAS : révèle l'adresse du domicile en une requête DNS.

## Points faibles acceptés

1. La liste publique est vide au départ : « trouver les derniers joueurs » est un pari.
2. CP déclaré, invérifiable ; le tri réel se fait en jeu.
3. Secrets dans l'URL : mauvais collage, PC partagé, stockage effacé. Parades d'architecture (liens
   distincts, aucun effet de bord sur GET, pas de secret dans les métadonnées, pas de referer, expiration).
4. Obligations légales d'un site public à contenu visiteur hébergé chez un particulier (mentions,
   conservation des données d'identification) : à vérifier hors forge ; possible conflit avec 24 h de rétention IP.
5. L'écran de proposition de composition sur mobile est le plus gros risque de glissement en solo.
6. Deux candidatures avec le même `Pseudo[SRV]` : l'outil signale, l'organisateur tranche.

## Renvoyé à la spine d'architecture (critère fixé)

- Q7 déploiement : le motif push-to-deploy par runner auto-hébergé existe déjà.
- Q8 données de référence : fichiers de données éditables lus sans reconstruire l'image ; Vincent les
  met à jour après vérification dans le client.
- Q11 stack : le moins de pièces à sécuriser et maintenir seul, dans une techno que Vincent pratique ;
  SQLite dans un volume dédié proposé.
- Limitation de débit : Caddy de base n'en a pas (module à compiler ou limite applicative) ; IP visiteur
  de confiance uniquement depuis le saut du tunnel.

## Gardé en mémoire (hors v1)

- « Mes événements » local au navigateur de l'organisateur et « Dupliquer l'événement » sont bon marché et
  peuvent entrer en v1 ; export/import d'un événement en fichier (le « seed ») plus tard.
- Connexion pour le seul leader : c'est un compte, donc un changement de produit à remonter.
- Disponibilités récurrentes d'un joueur, liste de joueurs seuls.

## Signal de succès (inchangé)

Un vrai raid Sanctuaire à 10 monté de bout en bout avec l'outil, au moins deux serveurs, idéalement les
deux factions, date sortie de la grille, salle trouvée par son nom, raid joué. Personne n'a demandé
« comment je me connecte ? ».
