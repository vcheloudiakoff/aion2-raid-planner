//! Bornes des champs saisis par un visiteur (AD-7). Fonctions pures, une erreur typée par cas : la
//! couche `http` les traduit en codes d'erreur, jamais en phrases.

/// Longueur maximale d'un pseudo, en caractères (SPEC de l'epic 2, *Assumptions*).
pub const PSEUDO_MAX_CHARS: usize = 16;
/// Longueur maximale d'une note d'événement, en caractères.
pub const NOTE_MAX_CHARS: usize = 280;
/// CP maximal accepté (SPEC de l'epic 2, *Assumptions*).
pub const CP_MAX: i64 = 999_999;
/// Longueur maximale d'un lien Discord, en caractères.
pub const DISCORD_URL_MAX_CHARS: usize = 256;
/// Hôtes admis pour `discord_url`, comparés exactement (casse ASCII ignorée).
pub const DISCORD_HOSTS: &[&str] = &["discord.gg", "discord.com", "www.discord.com"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FieldError {
    #[error("pseudo is empty")]
    PseudoEmpty,
    #[error("pseudo is too long")]
    PseudoTooLong,
    #[error("pseudo contains a character outside letters and digits")]
    PseudoCharset,
    #[error("note is too long")]
    NoteTooLong,
    #[error("note contains a URL")]
    NoteUrl,
    #[error("CP is out of range")]
    CpOutOfRange,
    #[error("discord URL is too long")]
    DiscordTooLong,
    #[error("discord URL must start with https://")]
    DiscordScheme,
    #[error("discord URL must not carry user information")]
    DiscordUserinfo,
    #[error("discord URL must not carry a port")]
    DiscordPort,
    #[error("discord URL host is not allowed")]
    DiscordHost,
    #[error("discord URL path is empty")]
    DiscordPath,
    #[error("discord URL contains a forbidden character")]
    DiscordCharset,
}

/// Pseudo : 1 à 16 caractères, tous `char::is_alphanumeric` (lettres de toute écriture et chiffres).
pub fn pseudo(value: &str) -> Result<(), FieldError> {
    let count = value.chars().count();
    if count == 0 {
        return Err(FieldError::PseudoEmpty);
    }
    if count > PSEUDO_MAX_CHARS {
        return Err(FieldError::PseudoTooLong);
    }
    if !value.chars().all(char::is_alphanumeric) {
        return Err(FieldError::PseudoCharset);
    }
    Ok(())
}

/// Note : au plus 280 caractères, sans URL (`://` ou `www.`, casse ignorée). Vide admise.
pub fn note(value: &str) -> Result<(), FieldError> {
    if value.chars().count() > NOTE_MAX_CHARS {
        return Err(FieldError::NoteTooLong);
    }
    let lower = value.to_lowercase();
    if lower.contains("://") || lower.contains("www.") {
        return Err(FieldError::NoteUrl);
    }
    Ok(())
}

/// CP : entier de 0 à 999 999.
pub fn cp(value: i64) -> Result<u32, FieldError> {
    if (0..=CP_MAX).contains(&value) {
        // Borné ci-dessus : tient dans un `u32`.
        Ok(value as u32)
    } else {
        Err(FieldError::CpOutOfRange)
    }
}

/// Lien Discord : `https://`, un hôte exact de [`DISCORD_HOSTS`], ni userinfo ni port, un chemin non
/// vide. ASCII imprimable seulement, sans `\` (qu'un navigateur lit comme `/`).
pub fn discord_url(value: &str) -> Result<(), FieldError> {
    if value.chars().count() > DISCORD_URL_MAX_CHARS {
        return Err(FieldError::DiscordTooLong);
    }
    let rest = value
        .strip_prefix("https://")
        .ok_or(FieldError::DiscordScheme)?;
    if !value.bytes().all(|c| c.is_ascii_graphic() && c != b'\\') {
        return Err(FieldError::DiscordCharset);
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.contains('@') {
        return Err(FieldError::DiscordUserinfo);
    }
    if authority.contains(':') {
        return Err(FieldError::DiscordPort);
    }
    if !DISCORD_HOSTS
        .iter()
        .any(|h| h.eq_ignore_ascii_case(authority))
    {
        return Err(FieldError::DiscordHost);
    }
    let path = tail
        .strip_prefix('/')
        .ok_or(FieldError::DiscordPath)?
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    if path.is_empty() {
        return Err(FieldError::DiscordPath);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudo_bounds() {
        assert_eq!(pseudo(""), Err(FieldError::PseudoEmpty));
        assert_eq!(pseudo("a"), Ok(()));
        assert_eq!(pseudo(&"a".repeat(16)), Ok(()));
        assert_eq!(pseudo(&"a".repeat(17)), Err(FieldError::PseudoTooLong));
        // Bornes en caractères, pas en octets.
        assert_eq!(pseudo(&"é".repeat(16)), Ok(()));
        assert_eq!(pseudo(&"한".repeat(17)), Err(FieldError::PseudoTooLong));
    }

    #[test]
    fn pseudo_charset() {
        for ok in ["Héloïse", "한글이름", "Zoë42", "Ærøskøbing", "123"] {
            assert_eq!(pseudo(ok), Ok(()), "{ok}");
        }
        for bad in [
            "a[SRV]", "[", "]", "a b", " a", "a-b", "a_b", "a.b", "a'b", "a\tb",
        ] {
            assert_eq!(pseudo(bad), Err(FieldError::PseudoCharset), "{bad}");
        }
    }

    #[test]
    fn note_bounds() {
        assert_eq!(note(""), Ok(()));
        assert_eq!(note(&"a".repeat(280)), Ok(()));
        assert_eq!(note(&"a".repeat(281)), Err(FieldError::NoteTooLong));
        assert_eq!(note(&"é".repeat(280)), Ok(()));
        assert_eq!(note("Rendez-vous à 21 h, vocal prêt."), Ok(()));
    }

    #[test]
    fn note_rejects_urls() {
        for bad in [
            "voir https://example.org",
            "HTTP://x",
            "ftp://x",
            "www.example.org",
            "WWW.Example.org",
        ] {
            assert_eq!(note(bad), Err(FieldError::NoteUrl), "{bad}");
        }
    }

    #[test]
    fn cp_bounds() {
        assert_eq!(cp(0), Ok(0));
        assert_eq!(cp(1), Ok(1));
        assert_eq!(cp(999_999), Ok(999_999));
        assert_eq!(cp(1_000_000), Err(FieldError::CpOutOfRange));
        assert_eq!(cp(-1), Err(FieldError::CpOutOfRange));
        assert_eq!(cp(i64::MAX), Err(FieldError::CpOutOfRange));
    }

    #[test]
    fn discord_url_accepts_allowlisted_hosts() {
        for ok in [
            "https://discord.gg/abcDEF",
            "https://discord.com/invite/abc",
            "https://www.discord.com/channels/1/2",
            "https://Discord.GG/abc",
            "https://discord.gg/abc?x=1",
        ] {
            assert_eq!(discord_url(ok), Ok(()), "{ok}");
        }
    }

    #[test]
    fn discord_url_length_bound() {
        let prefix = "https://discord.gg/";
        let at = |n: usize| format!("{prefix}{}", "a".repeat(n - prefix.len()));
        assert_eq!(discord_url(&at(256)), Ok(()));
        assert_eq!(discord_url(&at(257)), Err(FieldError::DiscordTooLong));
    }

    #[test]
    fn discord_url_rejections() {
        for (bad, err) in [
            ("http://discord.gg/abc", FieldError::DiscordScheme),
            ("discord.gg/abc", FieldError::DiscordScheme),
            ("HTTPS://discord.gg/abc", FieldError::DiscordScheme),
            ("https://discord.com.evil.io/abc", FieldError::DiscordHost),
            ("https://evil.io/discord.gg", FieldError::DiscordHost),
            ("https://cdn.discord.com/abc", FieldError::DiscordHost),
            ("https://discord.gg./abc", FieldError::DiscordHost),
            ("https://user@discord.gg/abc", FieldError::DiscordUserinfo),
            (
                "https://discord.gg@evil.io/abc",
                FieldError::DiscordUserinfo,
            ),
            ("https://discord.gg:443/abc", FieldError::DiscordPort),
            ("https://discord.gg", FieldError::DiscordPath),
            ("https://discord.gg/", FieldError::DiscordPath),
            ("https://discord.gg?x=1", FieldError::DiscordPath),
            ("https://discord.gg/?x=1", FieldError::DiscordPath),
            ("https://discord.gg/a b", FieldError::DiscordCharset),
            (
                "https://evil.io\\@discord.gg/abc",
                FieldError::DiscordCharset,
            ),
            ("https://discord.gg/é", FieldError::DiscordCharset),
        ] {
            assert_eq!(discord_url(bad), Err(err), "{bad}");
        }
    }
}
