//! Aucun « maintenant » pris dans SQL (AD-11) : grep du code du dépôt.

use std::path::{Path, PathBuf};

const EXTENSIONS: &[&str] = &[
    "rs", "sql", "ts", "tsx", "js", "mjs", "toml", "sh", "yml", "yaml",
];
const SKIP_DIRS: &[&str] = &["target", "node_modules", "dist", ".git"];
const ROOTS: &[&str] = &["crates", "web", "reference", "deploy", ".github"];

fn forbidden() -> [String; 6] {
    // Motifs assemblés pour que ce fichier ne se signale pas lui-même. Le littéral « now » entre
    // apostrophes ou guillemets couvre datetime, date, time, julianday et strftime ; les mots-clés
    // couvrent le reste.
    let now = ["n", "ow"].concat();
    [
        format!("'{now}'"),
        format!("\"{now}\""),
        format!("{}(", "unixepoch"),
        format!("{}_{}", "current", "date"),
        format!("{}_{}", "current", "time"),
        format!("{}_{}", "current", "timestamp"),
    ]
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if !SKIP_DIRS.contains(&name.to_string_lossy().as_ref()) {
                walk(&path, out);
            }
        } else if path
            .extension()
            .is_some_and(|e| EXTENSIONS.contains(&e.to_string_lossy().as_ref()))
        {
            out.push(path);
        }
    }
}

#[test]
fn no_sql_clock_in_repository_code() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for root in ROOTS {
        walk(&repo.join(root), &mut files);
    }
    assert!(
        files.iter().any(|f| f.ends_with("no_sql_now.rs")),
        "walk reached the crates"
    );
    let patterns = forbidden();
    let offenders: Vec<String> = files
        .iter()
        .filter_map(|f| {
            let text = std::fs::read_to_string(f).ok()?.to_lowercase();
            let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            patterns
                .iter()
                .any(|p| text.contains(p))
                .then(|| f.display().to_string())
        })
        .collect();
    assert!(offenders.is_empty(), "SQL clock found in: {offenders:?}");
}
