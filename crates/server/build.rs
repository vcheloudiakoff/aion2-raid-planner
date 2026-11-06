//! Fixe le hash de build depuis `APP_BUILD` à la compilation (`dev` sinon) et recompile quand le
//! build Vite change. N'invoque jamais npm (AD-1).

fn main() {
    println!("cargo:rerun-if-env-changed=APP_BUILD");
    println!("cargo:rerun-if-changed=../../web/dist");
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=tests/fixtures/migrations");
    let build = std::env::var("APP_BUILD").ok().filter(|v| !v.is_empty());
    let build = build.unwrap_or_else(|| "dev".to_owned());
    assert!(
        build.len() <= 64
            && build
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c)),
        "APP_BUILD must be at most 64 characters among [A-Za-z0-9._-]"
    );
    println!("cargo:rustc-env=APP_BUILD_HASH={build}");
}
