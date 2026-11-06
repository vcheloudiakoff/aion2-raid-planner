#!/bin/sh
# Tests de deploy/plan-deploy.sh sur des dépôts git temporaires : un cas par ligne de la matrice
# de la story 1-4.
#
#   sh deploy/tests/plan-deploy.test.sh

set -u

here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
plan="$here/../plan-deploy.sh"

tmp=$(mktemp -d) || exit 2
trap 'rm -rf "$tmp"' EXIT
trap 'exit 130' HUP INT TERM

# Git isolé de la configuration du poste (hooks, signature, branche par défaut).
export GIT_CONFIG_NOSYSTEM=1
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@example.org
export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@example.org

failures=0
total=0
repo="$tmp/repo"

fail() {
    failures=$((failures + 1))
    printf 'FAIL %s: %s\n' "$desc" "$1"
    sed 's/^/    /' "$tmp/out" "$tmp/err"
}

# Dépôt neuf : une migration, un fichier de code, un fichier de référence.
new_repo() {
    rm -rf "$repo"
    mkdir -p "$repo/crates/server/migrations" "$repo/crates/server/src" "$repo/reference"
    cd "$repo" || exit 2
    git init -q -b main .
    : >crates/server/migrations/.gitkeep
    echo "CREATE TABLE a (x);" >crates/server/migrations/0001_init.sql
    echo "fn main() {}" >crates/server/src/main.rs
    echo "label = 1" >reference/servers.toml
    commit "initial"
    base=$(git rev-parse HEAD)
}

commit() {
    git add -A && git commit -q --allow-empty -m "$1"
}

run_plan() {
    total=$((total + 1))
    (cd "$repo" && sh "$plan" "$@") >"$tmp/out" 2>"$tmp/err"
    code=$?
}

# expect_plan DESCRIPTION REBUILD BACKUP -- BASE HEAD
expect_plan() {
    desc=$1
    want=$(printf 'rebuild=%s\nbackup=%s' "$2" "$3")
    shift 4
    run_plan "$@"
    if [ "$code" -ne 0 ]; then
        fail "exit $code, expected 0"
    elif [ "$(cat "$tmp/out")" != "$want" ]; then
        fail "unexpected plan"
    fi
}

# expect_error DESCRIPTION CODE STDERR_FRAGMENT -- BASE HEAD
expect_error() {
    desc=$1
    want_code=$2
    fragment=$3
    shift 4
    run_plan "$@"
    if [ "$code" -ne "$want_code" ]; then
        fail "exit $code, expected $want_code"
    elif [ -s "$tmp/out" ]; then
        fail "stdout not empty"
    elif ! grep -qF -- "$fragment" "$tmp/err"; then
        fail "stderr lacks \"$fragment\""
    fi
}

# --- Cas nominaux ---------------------------------------------------------------------------

new_repo
expect_plan "first deployment" 1 0 -- "" "$(git rev-parse HEAD)"
expect_plan "same revision (manual re-run)" 0 0 -- "$base" "$base"

echo "fn main() { () }" >crates/server/src/main.rs
commit "code"
expect_plan "code changed" 1 0 -- "$base" "$(git rev-parse HEAD)"

new_repo
echo "label = 2" >reference/servers.toml
echo "label = 1" >reference/classes.toml
commit "reference"
expect_plan "reference only" 0 0 -- "$base" "$(git rev-parse HEAD)"

new_repo
commit "empty"
expect_plan "empty commit" 0 0 -- "$base" "$(git rev-parse HEAD)"

new_repo
echo "CREATE TABLE b (x);" >crates/server/migrations/0002_b.sql
commit "migration added"
expect_plan "migration added" 1 1 -- "$base" "$(git rev-parse HEAD)"

new_repo
echo "CREATE TABLE b (x);" >crates/server/migrations/0002_b.sql
git rm -q crates/server/migrations/.gitkeep
echo "label = 3" >reference/servers.toml
commit "migration added, .gitkeep removed"
expect_plan "migration added with .gitkeep removed" 1 1 -- "$base" "$(git rev-parse HEAD)"

new_repo
echo "-- notes" >crates/server/migrations/README
commit "non-sql file in migrations"
expect_plan "non-sql file in migrations" 1 0 -- "$base" "$(git rev-parse HEAD)"

new_repo
echo "label = 2" >reference/servers.toml
commit "reference"
echo "CREATE TABLE b (x);" >crates/server/migrations/0002_b.sql
commit "migration"
expect_plan "several commits, migration in the middle" 1 1 -- "$base" "$(git rev-parse HEAD)"

# --- Garde-fou des migrations ---------------------------------------------------------------

new_repo
echo "CREATE TABLE a (y);" >crates/server/migrations/0001_init.sql
commit "migration modified"
expect_error "migration modified" 1 "(M): crates/server/migrations/0001_init.sql" -- \
    "$base" "$(git rev-parse HEAD)"

new_repo
git rm -q crates/server/migrations/0001_init.sql
commit "migration deleted"
expect_error "migration deleted" 1 "(D): crates/server/migrations/0001_init.sql" -- \
    "$base" "$(git rev-parse HEAD)"

new_repo
git mv crates/server/migrations/0001_init.sql crates/server/migrations/0001_first.sql
commit "migration renamed"
expect_error "migration renamed" 1 "crates/server/migrations/0001_init.sql -> crates/server/migrations/0001_first.sql" -- \
    "$base" "$(git rev-parse HEAD)"

new_repo
git mv crates/server/migrations/0001_init.sql crates/server/0001_init.sql
commit "migration moved out"
expect_error "migration moved out of the folder" 1 "crates/server/migrations/0001_init.sql" -- \
    "$base" "$(git rev-parse HEAD)"

new_repo
echo "label = 2" >reference/servers.toml
commit "reference"
echo "CREATE TABLE a (y);" >crates/server/migrations/0001_init.sql
commit "migration modified later"
echo "CREATE TABLE b (x);" >crates/server/migrations/0002_b.sql
commit "migration added"
expect_error "modified migration hidden among commits" 1 "0001_init.sql" -- \
    "$base" "$(git rev-parse HEAD)"

# --- Base inconnue ----------------------------------------------------------------------------

new_repo
echo "fn main() { () }" >crates/server/src/main.rs
commit "code"
head=$(git rev-parse HEAD)
expect_error "base absent from the clone" 1 "unknown base" -- \
    "0123456789abcdef0123456789abcdef01234567" "$head"
expect_error "base is the dev label" 1 "unknown base" -- dev "$head"
expect_error "base is an abbreviated sha" 1 "unknown base" -- "$(printf '%.12s' "$base")" "$head"

git checkout -q -b rewritten "$base"
echo "fn main() { 1; }" >crates/server/src/main.rs
commit "rewritten history"
expect_error "base not an ancestor of head" 1 "not an ancestor" -- "$(git rev-parse HEAD)" "$head"

expect_error "unknown head" 1 "unknown head" -- "$base" "0123456789abcdef0123456789abcdef01234567"

# --- Usage ------------------------------------------------------------------------------------

total=$((total + 1))
desc="usage"
(cd "$repo" && sh "$plan" "$base") >"$tmp/out" 2>"$tmp/err"
code=$?
[ "$code" -eq 2 ] && grep -qF usage "$tmp/err" || fail "exit $code, expected 2 with usage"

cd "$here" || exit 2
if [ "$failures" -ne 0 ]; then
    printf '%s/%s plan-deploy tests failed\n' "$failures" "$total"
    exit 1
fi
printf 'plan-deploy: %s tests passed\n' "$total"
