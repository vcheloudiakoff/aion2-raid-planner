#!/bin/sh
# Tests de deploy/check-hardening.sh : un fixture par motif interdit et par clé manquante, fuites et
# déclencheurs de workflow dans des dossiers temporaires. Les adresses et noms d'hôte fautifs sont
# assemblés ici à l'exécution, jamais écrits en clair (le dépôt lui-même est balayé).
#
#   sh deploy/tests/check-hardening.test.sh

set -u

here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
check="$here/../check-hardening.sh"
network="$here/../check-network-crates.sh"
fixtures="$here/fixtures"

tmp=$(mktemp -d) || exit 2
trap 'rm -rf "$tmp"' EXIT
trap 'exit 130' HUP INT TERM
empty="$tmp/empty"
mkdir "$empty"

failures=0
total=0

# expect_ok DESCRIPTION -- ARGS...
expect_ok() {
    desc=$1
    shift 2
    total=$((total + 1))
    if sh "$check" "$@" 2>"$tmp/stderr"; then
        :
    else
        failures=$((failures + 1))
        printf 'FAIL %s: expected exit 0, stderr:\n' "$desc"
        sed 's/^/    /' "$tmp/stderr"
    fi
}

# expect_fail DESCRIPTION EXPECTED_STDERR_FRAGMENT -- ARGS...
expect_fail() {
    desc=$1
    fragment=$2
    shift 3
    total=$((total + 1))
    if sh "$check" "$@" 2>"$tmp/stderr"; then
        failures=$((failures + 1))
        printf 'FAIL %s: expected non-zero exit\n' "$desc"
    elif ! grep -qF -- "$fragment" "$tmp/stderr"; then
        failures=$((failures + 1))
        printf 'FAIL %s: stderr lacks "%s":\n' "$desc" "$fragment"
        sed 's/^/    /' "$tmp/stderr"
    fi
}

# expect_not_echoed DESCRIPTION VALUE : la valeur fautive n'apparaît pas sur stderr.
expect_not_echoed() {
    total=$((total + 1))
    if grep -qF -- "$2" "$tmp/stderr"; then
        failures=$((failures + 1))
        printf 'FAIL %s: stderr echoes the leaked value\n' "$1"
    fi
}

# --- Dépôt réel et fixture de base ----------------------------------------------------------

expect_ok "repository (default arguments)" --
expect_ok "base fixture" -- "$fixtures/base.yml" "$empty"

expect_ok "flow-list fixture" -- "$fixtures/base-flow.yml" "$empty"

# --- Motifs interdits et clés manquantes : un fixture par cas -------------------------------
# fixture | fragment attendu sur stderr

while IFS='|' read -r fixture fragment; do
    [ -n "$fixture" ] || continue
    expect_fail "$fixture" "$fragment" -- "$fixtures/$fixture.yml" "$empty"
done <<'EOF'
forbidden-privileged|forbidden: privileged
forbidden-quoted-privileged|forbidden: privileged
forbidden-pid|forbidden: pid
forbidden-docker-sock|forbidden: docker.sock
forbidden-cap-add|forbidden: cap_add
forbidden-network-mode-host|forbidden: network_mode
forbidden-bind-mount-short|forbidden: bind mount
forbidden-bind-mount-long|forbidden: bind mount
forbidden-bind-mount-relative-source|forbidden: bind mount
forbidden-driver-opts-bind|forbidden: bind mount
forbidden-devices|forbidden: devices
forbidden-volumes-from|forbidden: volumes_from
forbidden-ipc-host|forbidden: ipc: host
forbidden-uts-host|forbidden: uts: host
forbidden-userns-mode-host|forbidden: userns_mode: host
forbidden-cgroup-host|forbidden: cgroup: host
forbidden-seccomp-unconfined|forbidden: unconfined
forbidden-apparmor-unconfined|forbidden: unconfined
forbidden-sysctls|forbidden: sysctls
forbidden-secrets-file|forbidden: secrets file
forbidden-configs-file|forbidden: configs file
forbidden-include|forbidden: include
forbidden-extends|forbidden: extends
missing-user|server: missing: user
missing-user-root|server: missing: user
missing-user-root-name|server: missing: user
missing-user-gid-zero|server: missing: user
missing-user-variable|server: missing: user
missing-read-only|server: missing: read_only
missing-read-only-in-volume|server: missing: read_only
missing-cap-drop|server: missing: cap_drop
missing-mem-limit|server: missing: mem_limit
missing-cpus|server: missing: cpus
missing-pids-limit|server: missing: pids_limit
missing-tmpfs|server: missing: tmpfs
missing-no-new-privileges|server: missing: no-new-privileges
missing-restart|server: missing: restart
missing-restart-no|server: missing: restart
missing-logging|server: missing: logging
missing-second-service|worker: missing: user
missing-port-localhost|port without 127.0.0.1: prefix
missing-port-long-syntax|port without 127.0.0.1: prefix
EOF

expect_fail "missing compose" "missing compose file" -- "$tmp/absent.yml" "$empty"

# Fichier illisible : l'échec d'awk est une violation, jamais un succès silencieux.
cp "$fixtures/base.yml" "$tmp/unreadable.yml"
chmod 000 "$tmp/unreadable.yml"
if [ -r "$tmp/unreadable.yml" ]; then
    printf 'skip unreadable compose: file still readable (root?)\n'
else
    expect_fail "unreadable compose" "unreadable.yml: unreadable" -- "$tmp/unreadable.yml" "$empty"
fi

# --- Fuites -----------------------------------------------------------------------------------

private_ip=$(printf '%s.%s.%s.%s' 192 168 1 10)
public_ip=$(printf '%s.%s.%s.%s' 203 0 113 7)
lan_host=$(printf '%s.%s' nas lan)
tailnet_host=$(printf '%s.%s.%s.%s' box tail1234 ts net)
public_host=$(printf '%s.%s.%s' raid my-own-domain fr)

leak() { # leak NAME CONTENT : un dossier avec un seul fichier, la fuite en ligne 2.
    mkdir -p "$tmp/$1"
    printf 'ligne propre\n%s\n' "$2" >"$tmp/$1/file.txt"
}

leak ip-private "upstream http://$private_ip:8080"
expect_fail "private IPv4" "file.txt:2: leak: IPv4 address" -- "$fixtures/base.yml" "$tmp/ip-private"
expect_not_echoed "private IPv4" "$private_ip"
leak ip-public "remote_ip $public_ip"
expect_fail "public IPv4" "file.txt:2: leak: IPv4 address" -- "$fixtures/base.yml" "$tmp/ip-public"
leak host-lan "reverse_proxy $lan_host:8080"
expect_fail "LAN hostname" "file.txt:2: leak: hostname" -- "$fixtures/base.yml" "$tmp/host-lan"
expect_not_echoed "LAN hostname" "$lan_host"
leak host-tailnet "$tailnet_host"
expect_fail "tailnet hostname" "file.txt:2: leak: hostname" -- "$fixtures/base.yml" "$tmp/host-tailnet"
leak host-public "PUBLIC_BASE_URL=https://$public_host"
expect_fail "public hostname" "file.txt:2: leak: hostname" -- "$fixtures/base.yml" "$tmp/host-public"
leak host-upper "Host: $(printf '%s' "$public_host" | tr 'a-z' 'A-Z')"
expect_fail "uppercase hostname" "file.txt:2: leak: hostname" -- "$fixtures/base.yml" "$tmp/host-upper"

mkdir -p "$tmp/clean"
cat >"$tmp/clean/file.txt" <<'EOF'
ports: "127.0.0.1:8080:8080" and listen 0.0.0.0:8080
PUBLIC_BASE_URL=https://raid.example.org ; mail@example.com ; www.example.net ; nas.example
uses: actions/checkout from https://github.com/actions/checkout
localhost, /data/app.db, deploy/docker-compose.yml, check-hardening.sh, src/main.rs, README.md
FROM node:24.22.2-alpine ; rust:1.99.1-alpine ; env.APP_BUILD ; steps.build.outputs ; /usr/local/bin
EOF
expect_ok "whitelisted names and addresses" -- "$fixtures/base.yml" "$tmp/clean"

mkdir -p "$tmp/excluded/tests/fixtures"
printf '%s\n' "$private_ip" >"$tmp/excluded/tests/fixtures/leak.txt"
printf 'PUBLIC_BASE_URL=https://%s\n' "$public_host" >"$tmp/excluded/.env"
printf 'PUBLIC_BASE_URL=https://%s\n' "$public_host" >"$tmp/excluded/.env.nas"
expect_ok "fixtures and real .env excluded" -- "$fixtures/base.yml" "$tmp/excluded"
printf 'PUBLIC_BASE_URL=https://%s\n' "$public_host" >"$tmp/excluded/.env.example"
expect_fail ".env.example scanned" ".env.example:1: leak: hostname" -- "$fixtures/base.yml" "$tmp/excluded"

# --- Déclencheurs de workflow -----------------------------------------------------------------

mkdir -p "$tmp/wf-pr/workflows" "$tmp/wf-target/workflows"
printf 'on:\n  pull_request:\n    branches: [main]\n' >"$tmp/wf-pr/workflows/ci.yml"
printf 'on: [push, pull_request_target]\n' >"$tmp/wf-target/workflows/ci.yaml"
expect_fail "pull_request trigger" "ci.yml:2: forbidden workflow trigger: pull_request" -- "$fixtures/base.yml" "$tmp/wf-pr"
expect_fail "pull_request_target trigger" "ci.yaml:1: forbidden workflow trigger: pull_request_target" -- "$fixtures/base.yml" "$tmp/wf-target"

# Fixtures de workflow copiés hors de tests/fixtures/ (exclu du balayage).
wf_fixture() { # wf_fixture NAME : dossier temporaire avec ce seul workflow
    mkdir -p "$tmp/wf-$1/workflows"
    cp "$fixtures/workflows/$1.yml" "$tmp/wf-$1/workflows/"
}
wf_fixture workflow-run
expect_fail "workflow_run trigger" "workflow-run.yml:3: forbidden workflow trigger: workflow_run" -- "$fixtures/base.yml" "$tmp/wf-workflow-run"
wf_fixture issue-comment
expect_fail "issue_comment trigger" "issue-comment.yml:2: forbidden workflow trigger: issue_comment" -- "$fixtures/base.yml" "$tmp/wf-issue-comment"
wf_fixture allowed
expect_ok "push, workflow_dispatch and schedule only" -- "$fixtures/base.yml" "$tmp/wf-allowed"

# --- Crates client réseau (check-network-crates.sh) -------------------------------------------

network_case() { # network_case EXPECTED_STATUS LINE
    total=$((total + 1))
    printf '%s\n' "$2" | sh "$network" 2>/dev/null
    got=$?
    if [ "$got" -ne "$1" ]; then
        failures=$((failures + 1))
        printf 'FAIL network crates: "%s" exit %s, expected %s\n' "$2" "$got" "$1"
    fi
}

while IFS= read -r line; do
    [ -n "$line" ] && network_case 1 "$line"
done <<'EOF'
├── reqwest v0.12.9
│   ├── ureq v2.10.1
└── lettre feature "default"
│   │   ├── hickory-resolver v0.24.1
├── hickory-proto feature "tokio-runtime"
├── trust-dns-resolver v0.23.2
├── curl v0.4.47
│   └── curl-sys v0.4.78+curl-8.11.0
├── isahc v1.7.2
├── attohttpc v0.28.0
└── surf v2.3.2
│   │   ├── hyper feature "client"
│   │   └── hyper-util feature "client-legacy"
EOF

while IFS= read -r line; do
    [ -n "$line" ] && network_case 0 "$line"
done <<'EOF'
├── axum v0.8.9
│   ├── hyper v1.7.0
│   │   ├── hyper feature "server"
│   │   ├── hyper-util v0.1.21
│   │   └── hyper-util feature "server-auto"
├── tracing-subscriber feature "env-filter"
├── surfer v1.0.0
├── libcurl-helper v0.1.0
└── foo feature "curl"
EOF

total=$((total + 1))
if printf '' | sh "$network" 2>/dev/null; then
    failures=$((failures + 1))
    printf 'FAIL network crates: empty input accepted\n'
fi

# --- Bilan ------------------------------------------------------------------------------------

if [ "$failures" -ne 0 ]; then
    printf '%s/%s check-hardening tests failed\n' "$failures" "$total"
    exit 1
fi
printf 'check-hardening: %s tests passed\n' "$total"
