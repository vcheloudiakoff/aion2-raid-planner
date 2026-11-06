# Revue éditoriale du document « Raid Planner, comment ça marche » — lens structure

Modèle retenu : Explanation (Conceptual) pour le corps ; Strategic/Pyramid pour « Ce que tu dois relire ».
Texte visible : 2 703 mots (Sécurité 636, À relire 471, Vie d'un événement 318).

| Pass | Original | Revised | Changes |
| --- | --- | --- | --- |
| structure | §À relire — ordre des 10 blocs : bibliothèque, outillage, exploitation, données, produit ×3, sécurité, produit, technique, puis juridique | MOVE : par enjeu — juridique → produit (rôle dérivé en premier, liste des inscrits, secret dans l'adresse, verrouillage global) → données → sécurité (runner) → exploitation (sans shell) → technique (10 s) → outillage et bibliothèque en dernier | La section d'action ouvre sur ses deux items les moins lourds et les plus jargonneux ; le bloc signalé par deux relecteurs est en neuvième position. |
| structure | §À relire — paragraphe final « question qui n'est pas technique » | MOVE : premier bloc `.review`, étiquette « juridique », avec un id ; « voir plus bas » du tableau Sécurité pointe dessus ; intro scindée « Une question ouverte » / « Dix choix faits sans toi » | Seul point non tranché, enterré après dix blocs mis en valeur. |
| structure | §Sécurité — tableau de 12 lignes | MOVE : quatre groupes — Le conteneur (1, 2, 7) · Ce qui n'existe pas (3, 9, 10) · Les visiteurs et l'abus (4, 5, 6, 8) · Le déploiement (11, 12) | Plus longue section sans respiration ; l'ordre alterne les thèmes. |
| structure | §Boîte › « C'est le runner qui le fait, tu ne le vois pas. » | CONDENSE/MOVE : définir « runner » à sa première occurrence (agent de déploiement installé sur le NAS, renvoi à §Mettre en ligne) | Mot employé deux fois avant d'être présenté. |
| structure | Corps — décisions présentées comme acquises mais ouvertes en §À relire (effacement 7 jours, groupe par créneau retenu, copier le lien, rôle déduit) ; trois décisions à relire absentes du corps (liste des inscrits, 10 secondes, runner groupe docker) | QUESTION : marqueur « (à relire) » lié au bloc ; une phrase à leur place naturelle pour les trois absentes | Le corps et la liste d'action ne se répondent pas. |
| structure | Ordre des sections 2 à 7 (hébergement, produit, hébergement) | QUESTION : remonter Droits et Vie avant Trajet, ou garder | L'ordre actuel reste défendable, Vincent connaît le produit. |
| structure | §Sécurité ligne débit : phrase sur le module Caddy (21 mots) | CONDENSE : parenthèse courte | La colonne promet « ce qu'elle empêche » ; la phrase justifie une alternative. |
| structure | §Droits — 3e paragraphe (hachage, en-tête, aucun effet sur GET) | QUESTION : laisser, ou déplacer en trois lignes du tableau Sécurité (groupe « Les liens ») | Seulement si le regroupement du tableau est adopté. |
| structure | §Mettre en ligne › « Ajouter un serveur de jeu… » | PRESERVE | Renforcement, pas redondance. |
| structure | Tuiles et « En cinq phrases » | PRESERVE | Modèle mental avant le détail. |

Mineures : fusion SQLx + TypeScript en « Versions d'outils » ; « l'adresse du proxy » jamais introduit (dire « VPS ») ; « pas d'environnement de test » doublé ; titres métaphoriques (optionnel).
Bilan : le problème est l'ordre, pas la longueur ; net ≈ +20 mots si tout est accepté.
