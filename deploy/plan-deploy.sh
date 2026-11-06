#!/bin/sh
# Plan du job `deploy` (AD-10) : que faire entre la révision en place et celle à déployer.
#
#   sh deploy/plan-deploy.sh BASE HEAD      (dans le clone git du dépôt)
#
# BASE : révision de l'image en place (label org.opencontainers.image.revision), vide au premier
#        déploiement. HEAD : commit à déployer (celui testé par le job `ci`).
#
# Sortie standard, deux lignes lues par le workflow :
#   rebuild=0|1   1 si un fichier hors de `reference/` a changé (ou premier déploiement)
#   backup=0|1    1 si une migration est ajoutée : sauvegarde `pre-deploy` avant la construction
#
# Code 1, avec le fichier fautif sur stderr : migration (`*.sql` de crates/server/migrations/)
# modifiée, supprimée, renommée ou changée de type ; BASE inconnue du clone ou qui n'est pas un
# ancêtre de HEAD (historique réécrit, voir deploy/README.md). Code 2 : usage.

set -u

if [ $# -ne 2 ]; then
    echo "usage: plan-deploy.sh BASE HEAD" >&2
    exit 2
fi
base=$1
head=$2

if ! git rev-parse --verify --quiet "$head^{commit}" >/dev/null; then
    echo "unknown head: $head" >&2
    exit 1
fi

if [ -z "$base" ]; then
    # Premier déploiement : rien à comparer ni à sauvegarder.
    printf 'rebuild=1\nbackup=0\n'
    exit 0
fi

# Un SHA complet seulement : un label `dev` ou un nom de branche ne vaut pas révision.
if ! printf '%s\n' "$base" | grep -Eq '^[0-9a-f]{40}([0-9a-f]{24})?$' ||
    ! git rev-parse --verify --quiet "$base^{commit}" >/dev/null; then
    echo "unknown base: $base is not a commit of this clone (see deploy/README.md)" >&2
    exit 1
fi
if ! git merge-base --is-ancestor "$base" "$head"; then
    echo "unknown base: $base is not an ancestor of $head (see deploy/README.md)" >&2
    exit 1
fi

changes=$(git -c core.quotePath=false diff --no-color --name-status --find-renames "$base" "$head") || {
    echo "git diff failed" >&2
    exit 1
}

printf '%s\n' "$changes" | awk -F '\t' '
    function migration(p) { return p ~ /^crates\/server\/migrations\/.*\.sql$/ }
    function outside_reference(p) { return p != "" && p !~ /^reference\// }
    BEGIN { rebuild = 0; backup = 0; bad = 0 }
    NF < 2 { next }
    {
        st = substr($1, 1, 1)
        if (st == "C") {
            # Copie : la source est inchangée, seule la destination compte (un ajout).
            if (migration($3)) backup = 1
            if (outside_reference($3)) rebuild = 1
            next
        }
        if (outside_reference($2) || outside_reference($3)) rebuild = 1
        if (migration($2) || migration($3)) {
            if (st == "A") backup = 1
            else {
                msg = "forbidden migration change (" $1 "): " $2
                if ($3 != "") msg = msg " -> " $3
                print msg | "cat 1>&2"
                bad = 1
            }
        }
    }
    END {
        if (bad) exit 1
        print "rebuild=" rebuild
        print "backup=" backup
    }
'
