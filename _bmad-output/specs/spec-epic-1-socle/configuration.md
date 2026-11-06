# Configuration — variables d'environnement

Lues une seule fois dans `Config` au démarrage, fail-fast sur valeur invalide ; pas de chargeur `.env` dans
le binaire. Obligatoires en `${VAR:?}` dans `deploy/docker-compose.yml` ; toutes présentes, sans valeur
secrète, dans `deploy/.env.example`. Chemins fixes, non configurables : `/data/app.db`, `/data/reference/`,
`/data/backups/`.

| Variable | Obligatoire | Défaut | Epic qui la consomme | Règle |
| --- | --- | --- | --- | --- |
| `PORT` | non | `8080` (hôte) | 1 | port hôte lié à `127.0.0.1:${PORT}` ; le conteneur écoute toujours 8080 (AD-8) |
| `PUBLIC_BASE_URL` | oui | — | 1 | seule source d'URL absolue (AD-9) |
| `FORM_TOKEN_SECRET` | oui | — | 1 | clé HMAC du jeton de formulaire, ≥ 32 octets (AD-7) |
| `CLIENT_IP_HEADER` | non | `X-Client-IP` | 1 | jamais `X-Forwarded-For` (AD-7) |
| `TRUST_CLIENT_IP_HEADER` | non | `true` | 1 | `false` réservé au poste de développement : repli sur l'adresse du pair |
| `MIN_SUBMIT_DELAY_MS` | non | `3000` | 1 | âge minimal du jeton de formulaire |
| `MAX_EVENTS_PER_HOUR` | non | `[ASSUMPTION]` `30` | 1 (primitive), 2 | disjoncteur global, compté sur écritures commises → 503 |
| `MAX_CANDIDATURES_PER_HOUR` | non | `[ASSUMPTION]` `300` | 1 (primitive), 2 | idem |
| `RATE_CREATE`, `RATE_CANDIDATURE`, `RATE_WRITE`, `RATE_READ` | non | `[ASSUMPTION]` `5/h`, `20/h`, `120/min`, `600/min` | 1 | quotas par IP et par classe de route ; noms des variables `[ASSUMPTION]` ; calibrés pour une légion derrière une seule box |
| `ADMIN_SECRET` | non | absente | 5 | absente = routes `/api/admin/*` absentes (AD-3) |
| `ROOM_NAME_TEMPLATE` | non | défini en epic 3 | 3 | AD-18 |
| `COMPOSE_TIME_BUDGET_MS` | non | `1000` | 4 | AD-13 |
| `PURGE_AFTER_DAYS` | non | `7` | 5 | AD-14 |
| `RUST_LOG` | non | `info` | 1 | `debug` interdit en production (AD-15) |

`POW_ENABLED` n'est pas créée en v1 (preuve de travail différée).

Une variable introduite par un epic ultérieur s'ajoute à ce tableau dans la spec de cet epic, et à
`.env.example` dans la story qui la lit.
