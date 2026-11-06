#!/bin/sh
# AD-17 : aucune crate client réseau (HTTP, SMTP, DNS) dans l'arbre de dépendances.
#
#   cargo tree -e features --workspace --target all --locked | sh deploy/check-network-crates.sh
#
# Lit la sortie de `cargo tree -e features` sur stdin ; une ligne par occurrence sur stderr
# (`stdin:<ligne>: <texte>`) et code 1 s'il y en a, code 2 si l'entrée est vide.

set -u

input=$(cat)
if [ -z "$input" ]; then
    echo "check-network-crates: empty input (cargo tree failed?)" >&2
    exit 2
fi

# Crate interdite (ligne `nom vX.Y` ou `nom feature "..."`), ou feature client de hyper / hyper-util.
pattern='(^|[^a-z0-9_-])(reqwest|ureq|lettre|hickory(-[a-z0-9-]+)?|trust-dns(-[a-z0-9-]+)?|curl(-sys)?|isahc|attohttpc|surf) (v[0-9]|feature )'
pattern="$pattern"'|(^|[^a-z0-9_-])hyper(-util)? feature "client'

hits=$(printf '%s\n' "$input" | grep -nE "$pattern")
if [ -n "$hits" ]; then
    printf '%s\n' "$hits" | sed 's/^/stdin:/' >&2
    exit 1
fi
exit 0
