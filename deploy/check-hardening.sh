#!/bin/sh
# Contrôle du durcissement (AD-8, AD-10) et des fuites du dépôt public, sans yq : sh et awk POSIX.
#
#   sh deploy/check-hardening.sh [COMPOSE [DIR...]]
#
# COMPOSE : fichier compose à contrôler (défaut deploy/docker-compose.yml).
# DIR     : dossiers balayés pour les fuites (défaut deploy/ et .github/workflows/) ; les fichiers
#           sous `*/workflows/` ne peuvent en plus déclencher que sur `push`, `workflow_dispatch`
#           ou `schedule` (liste blanche : ni pull request, ni workflow_run, ni issue_comment...).
#           `*/tests/fixtures/` et les `.env` réels (non versionnés) sont exclus du balayage.
#
# Une ligne par violation sur stderr, sans jamais recopier la valeur fautive ; code 1 s'il y en a.

set -u

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
compose=${1:-$root/deploy/docker-compose.yml}
[ $# -gt 0 ] && shift
[ $# -eq 0 ] && set -- "$root/deploy" "$root/.github/workflows"

# Chemin affiché : relatif à la racine du dépôt quand il y est.
rel() {
    case $1 in
        "$root"/*) printf '%s' "${1#"$root"/}" ;;
        *) printf '%s' "$1" ;;
    esac
}

out=$(mktemp) || exit 2
trap 'rm -f "$out" "$out.files"' EXIT
trap 'exit 130' HUP INT TERM

# --- Compose : motifs interdits et clés obligatoires, par service -----------------------------

if [ -f "$compose" ]; then
    awk -v name="$(rel "$compose")" '
    function strip(s) {
        sub(/^[ \t]*#.*$/, "", s)
        sub(/[ \t]+#.*$/, "", s)
        sub(/[ \t]+$/, "", s)
        return s
    }
    function unquote(s) {
        gsub(/^[ \t"\047]+|[ \t"\047]+$/, "", s)
        return s
    }
    function bad(msg) { print name ":" NR ": " msg }
    # Clé de mapping, guillemets facultatifs : "privileged": true.
    function is_key(s) { return s ~ /^["\047]?[A-Za-z0-9_.-]+["\047]?[ \t]*:([ \t]|$)/ }
    function key_of(s) { sub(/[ \t]*:([ \t].*)?$/, "", s); return unquote(s) }
    function value_of(s) { sub(/^["\047]?[A-Za-z0-9_.-]+["\047]?[ \t]*:[ \t]*/, "", s); return s }
    function service_child(d) { return d == 3 && key[1] == "services" }
    # Éléments d une liste en flux : [a, "b", c].
    function flow(v, arr) {
        sub(/^\[/, "", v); sub(/\]$/, "", v)
        return split(v, arr, ",")
    }
    function check_port(v) {
        if (unquote(v) !~ /^127\.0\.0\.1:/) bad("port without 127.0.0.1: prefix")
    }
    function check_volume(v,    src) {
        v = unquote(v)
        if (index(v, ":") == 0) return
        src = substr(v, 1, index(v, ":") - 1)
        if (src !~ /^[A-Za-z0-9][A-Za-z0-9_.-]*$/) bad("forbidden: bind mount")
    }
    # Élément de liste (bloc ou flux) de la clé k, située au niveau d.
    function item_value(k, d, v,    u) {
        u = unquote(v)
        if (k == "ports") check_port(v)
        else if (k == "volumes" && key[1] == "services") check_volume(v)
        if (service_child(d) && k == "cap_drop" && toupper(u) == "ALL") has[key[2], "cap_drop"] = 1
        if (service_child(d) && k == "security_opt" && u ~ /^no-new-privileges[ \t]*[:=][ \t]*true$/)
            has[key[2], "no-new-privileges"] = 1
    }
    # Clé k de valeur v, poussée au niveau d.
    function handle_key(k, v, d,    u, n, i, arr) {
        u = unquote(v)
        if (k ~ /^(privileged|pid|cap_add|network_mode|devices|volumes_from|sysctls|extends)$/)
            bad("forbidden: " k)
        if (k == "include" && d == 1) bad("forbidden: include")
        if (k ~ /^(ipc|uts|userns_mode|cgroup)$/ && u == "host") bad("forbidden: " k ": host")
        if (k == "file" && (key[1] == "secrets" || key[1] == "configs")) bad("forbidden: " key[1] " file")
        if (k == "type" && u == "bind") bad("forbidden: bind mount")
        if (k == "o" && u ~ /bind/) bad("forbidden: bind mount")
        if (k == "device") bad("forbidden: bind mount")
        if (k == "source" && u ~ /^[.\/~$]/) bad("forbidden: bind mount")

        if (d == 2 && key[1] == "services") svc[k] = 1
        if (service_child(d)) {
            s = key[2]
            if (k == "user" && u ~ /^[1-9][0-9]*:[1-9][0-9]*$/) has[s, "user"] = 1
            if (k == "read_only" && u == "true") has[s, "read_only"] = 1
            if (k == "mem_limit" && u != "") has[s, "mem_limit"] = 1
            if (k == "cpus" && u != "") has[s, "cpus"] = 1
            if (k == "pids_limit" && u != "") has[s, "pids_limit"] = 1
            if (k == "restart" && u != "" && u != "no") has[s, "restart"] = 1
            if (k == "logging") has[s, "logging"] = 1
            if (k == "tmpfs") has[s, "tmpfs"] = 1
            if (k == "ports" && v != "" && v !~ /^\[/) bad("port without 127.0.0.1: prefix")
            if (k == "volumes" && v != "" && v !~ /^\[/) bad("forbidden: bind mount")
        }
        if (v ~ /^\[/) {
            n = flow(v, arr)
            for (i = 1; i <= n; i++) item_value(k, d, arr[i])
        }
    }
    {
        line = strip($0)
        if (line ~ /^[ \t]*$/) next
        match(line, /^[ \t]*/); ind = RLENGTH
        content = substr(line, ind + 1)

        if (line ~ /docker\.sock/) bad("forbidden: docker.sock")
        if (line ~ /unconfined/) bad("forbidden: unconfined")

        if (content ~ /^-([ \t]|$)/) {
            # Élément de liste : son propriétaire est la dernière clé moins indentée (ou au même
            # niveau, style compact).
            while (depth > 0 && ind_of[depth] > ind) depth--
            k = depth > 0 ? key[depth] : ""
            v = content; sub(/^-[ \t]*/, "", v)
            if (is_key(v)) {
                # Élément mapping (syntaxe longue) : refusé pour un port (host_ip non contrôlé),
                # sinon une clé de plus, au niveau de son texte.
                if (k == "ports") bad("port without 127.0.0.1: prefix")
                kk = key_of(v)
                depth++; ind_of[depth] = ind + 2; key[depth] = kk
                handle_key(kk, value_of(v), depth)
            } else {
                item_value(k, depth, v)
            }
            next
        }

        if (is_key(content)) {
            k = key_of(content)
            while (depth > 0 && ind_of[depth] >= ind) depth--
            depth++; ind_of[depth] = ind; key[depth] = k
            handle_key(k, value_of(content), depth)
        }
    }
    END {
        n = split("user read_only cap_drop mem_limit cpus pids_limit no-new-privileges restart logging tmpfs", req, " ")
        found = 0
        for (s in svc) {
            found = 1
            for (i = 1; i <= n; i++) if (!((s, req[i]) in has)) print name ": " s ": missing: " req[i]
        }
        if (!found) print name ": missing: services"
    }
    ' "$compose" >>"$out" || printf '%s: unreadable\n' "$(rel "$compose")" >>"$out"
else
    printf '%s: missing compose file\n' "$(rel "$compose")" >>"$out"
fi

# --- Fuites : adresses IPv4 et noms d'hôte réels ; déclencheurs de workflow --------------------

for dir in "$@"; do
    if [ ! -d "$dir" ]; then
        printf '%s: missing directory\n' "$(rel "$dir")" >>"$out"
        continue
    fi
    find "$dir" -type d -path '*/tests/fixtures' -prune -o -type f -print | sort >"$out.files"
    while IFS= read -r f; do
        case ${f##*/} in
            .env.example) ;;
            .env | .env.*) continue ;;
        esac
        wf=0
        case $f in
            */workflows/*.yml | */workflows/*.yaml) wf=1 ;;
        esac
        awk -v name="$(rel "$f")" -v wf="$wf" '
        function bad(msg) { print name ":" FNR ": " msg }
        BEGIN {
            # TLD publics courants et suffixes de réseau privé ; pas de sh, rs, md, py, yml, db...
            tld = "com|net|org|edu|gov|io|co|me|info|biz|dev|app|xyz|cloud|online|site|tech|" \
                  "fr|de|eu|uk|be|ch|nl|it|es|us|ca|" \
                  "lan|local|localdomain|home|internal|intranet|corp|arpa"
            host_re = "[a-z0-9_-]+(\\.[a-z0-9_-]+)*\\.(" tld ")"
        }
        function allowed_host(t) {
            return t ~ /(^|\.)example\.(com|net|org)$/ || t ~ /\.example$/ || \
                   t ~ /(^|\.)github\.com$/ || t == "localhost"
        }
        function trigger(t) {
            gsub(/^[ \t"\047]+|[ \t"\047]+$/, "", t)
            if (t != "" && t !~ /^(push|workflow_dispatch|schedule)$/) bad("forbidden workflow trigger: " t)
        }
        # Déclencheurs de `on:` : scalaire, liste en flux, mapping en flux ou en bloc.
        function triggers(l,    c, v, n, i, arr, ind) {
            sub(/[ \t]+#.*$/, "", l)
            if (l ~ /^[ \t]*(#.*)?$/) return
            match(l, /^[ \t]*/); ind = RLENGTH
            c = substr(l, ind + 1)
            if (ind == 0) {
                in_on = 0
                if (c !~ /^["\047]?on["\047]?[ \t]*:/) return
                v = c; sub(/^["\047]?on["\047]?[ \t]*:[ \t]*/, "", v)
                if (v == "") { in_on = 1; on_ind = -1; return }
                gsub(/[][{}]/, "", v)
                n = split(v, arr, ",")
                for (i = 1; i <= n; i++) { sub(/:.*/, "", arr[i]); trigger(arr[i]) }
            } else if (in_on) {
                if (on_ind < 0) on_ind = ind
                if (ind == on_ind) { sub(/^-[ \t]*/, "", c); sub(/[ \t]*:.*$/, "", c); trigger(c) }
            }
        }
        function ipv4(t,    parts, n, i) {
            n = split(t, parts, ".")
            if (n != 4) return 0
            for (i = 1; i <= 4; i++) if (length(parts[i]) > 3 || parts[i] + 0 > 255) return 0
            return 1
        }
        {
            if (wf) triggers($0)

            s = $0
            while (match(s, /[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+/)) {
                t = substr(s, RSTART, RLENGTH)
                before = RSTART > 1 ? substr(s, RSTART - 1, 1) : ""
                after = substr(s, RSTART + RLENGTH, 2)
                s = substr(s, RSTART + RLENGTH)
                if (before ~ /[0-9.]/ || after ~ /^[0-9]/ || after ~ /^\.[0-9]/) continue
                if (ipv4(t) && t != "127.0.0.1" && t != "0.0.0.0") { bad("leak: IPv4 address"); break }
            }

            s = tolower($0)
            while (match(s, host_re)) {
                t = substr(s, RSTART, RLENGTH)
                after = substr(s, RSTART + RLENGTH, 1)
                s = substr(s, RSTART + RLENGTH)
                if (after ~ /[a-z0-9_-]/) continue
                if (!allowed_host(t)) { bad("leak: hostname"); break }
            }
        }
        ' "$f" >>"$out" || printf '%s: unreadable\n' "$(rel "$f")" >>"$out"
    done <"$out.files"
    rm -f "$out.files"
done

if [ -s "$out" ]; then
    cat "$out" >&2
    exit 1
fi
exit 0
