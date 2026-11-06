//! Le seul type d'instant du projet (AD-11).
//!
//! Une seule forme textuelle, en base comme en JSON : `YYYY-MM-DDTHH:MM:SSZ`, à la seconde, sans
//! fraction ni décalage autre que `Z`.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Datelike, NaiveDateTime, SubsecRound, Utc};

const FORMAT: &str = "%Y-%m-%dT%H:%M:%S";
const LEN: usize = "YYYY-MM-DDTHH:MM:SSZ".len();

/// Instant UTC à la seconde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(DateTime<Utc>);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimestampError {
    #[error("timestamp must have the form YYYY-MM-DDTHH:MM:SSZ")]
    Format,
    #[error("timestamp is out of range")]
    Range,
}

impl Timestamp {
    /// Construit un instant depuis un `DateTime<Utc>`, fraction de seconde tronquée.
    /// Refuse les années hors de 0000 à 9999, que la forme textuelle ne sait pas écrire.
    pub fn from_datetime(dt: DateTime<Utc>) -> Result<Self, TimestampError> {
        if !(0..=9999).contains(&dt.year()) {
            return Err(TimestampError::Range);
        }
        Ok(Self(dt.trunc_subsecs(0)))
    }

    pub fn from_unix_seconds(secs: i64) -> Result<Self, TimestampError> {
        let dt = DateTime::<Utc>::from_timestamp(secs, 0).ok_or(TimestampError::Range)?;
        Self::from_datetime(dt)
    }

    pub fn unix_seconds(self) -> i64 {
        self.0.timestamp()
    }

    pub fn as_datetime(self) -> DateTime<Utc> {
        self.0
    }
}

impl FromStr for Timestamp {
    type Err = TimestampError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let b = s.as_bytes();
        // Contrôle de forme octet par octet avant chrono, qui tolère signes, années longues et
        // secondes intercalaires.
        let shape_ok = b.len() == LEN
            && b.iter().enumerate().all(|(i, &c)| match i {
                4 | 7 => c == b'-',
                10 => c == b'T',
                13 | 16 => c == b':',
                19 => c == b'Z',
                _ => c.is_ascii_digit(),
            });
        if !shape_ok {
            return Err(TimestampError::Format);
        }
        let naive = NaiveDateTime::parse_from_str(&s[..LEN - 1], FORMAT)
            .map_err(|_| TimestampError::Format)?;
        let dt = naive.and_utc();
        if dt.timestamp_subsec_nanos() != 0 {
            // Seconde intercalaire (`:60`) : chrono la représente par une fraction.
            return Err(TimestampError::Format);
        }
        Self::from_datetime(dt)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}Z", self.0.format(FORMAT))
    }
}

impl serde::Serialize for Timestamp {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Timestamp {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = Timestamp;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a timestamp of the form YYYY-MM-DDTHH:MM:SSZ")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Timestamp, E> {
                v.parse().map_err(E::custom)
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

#[cfg(feature = "sqlx")]
mod sqlx_impl {
    use super::Timestamp;
    use sqlx::encode::IsNull;
    use sqlx::error::BoxDynError;
    use sqlx::sqlite::{SqliteTypeInfo, SqliteValueRef};
    use sqlx::{Database, Decode, Encode, Sqlite, Type};

    impl Type<Sqlite> for Timestamp {
        fn type_info() -> SqliteTypeInfo {
            <String as Type<Sqlite>>::type_info()
        }
        fn compatible(ty: &SqliteTypeInfo) -> bool {
            <String as Type<Sqlite>>::compatible(ty)
        }
    }

    impl<'q> Encode<'q, Sqlite> for Timestamp {
        fn encode_by_ref(
            &self,
            buf: &mut <Sqlite as Database>::ArgumentBuffer<'q>,
        ) -> Result<IsNull, BoxDynError> {
            <String as Encode<'q, Sqlite>>::encode(self.to_string(), buf)
        }
    }

    impl<'r> Decode<'r, Sqlite> for Timestamp {
        fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
            let s = <&str as Decode<'r, Sqlite>>::decode(value)?;
            Ok(s.parse()?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_form_and_round_trips() {
        let s = "2026-11-06T10:00:00Z";
        let ts: Timestamp = s.parse().unwrap();
        assert_eq!(ts.to_string(), s);
        assert_eq!(ts.unix_seconds(), 1_793_959_200);
    }

    #[test]
    fn rejects_every_other_form() {
        for bad in [
            "2026-11-06T10:00:00+00:00",
            "2026-11-06T10:00:00.000Z",
            "2026-11-06T10:00:00.5Z",
            "2026-11-06T10:00:00",
            "2026-11-06 10:00:00Z",
            "2026-11-06t10:00:00z",
            "2026-11-06T10:00Z",
            "+2026-11-06T10:00:00Z",
            "20260-11-06T10:00:00Z",
            " 2026-11-06T10:00:00Z",
            "2026-11-06T10:00:00Z ",
            "2026-13-06T10:00:00Z",
            "2026-02-30T10:00:00Z",
            "2026-11-06T24:00:00Z",
            "2026-12-31T23:59:60Z",
            "",
        ] {
            assert_eq!(
                bad.parse::<Timestamp>(),
                Err(TimestampError::Format),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn truncates_fraction_from_datetime() {
        let dt = DateTime::<Utc>::from_timestamp(1_793_959_200, 999_000_000).unwrap();
        let ts = Timestamp::from_datetime(dt).unwrap();
        assert_eq!(ts.to_string(), "2026-11-06T10:00:00Z");
    }

    #[test]
    fn extreme_years() {
        assert_eq!(
            "0000-01-01T00:00:00Z"
                .parse::<Timestamp>()
                .unwrap()
                .to_string(),
            "0000-01-01T00:00:00Z"
        );
        assert_eq!(
            "9999-12-31T23:59:59Z"
                .parse::<Timestamp>()
                .unwrap()
                .to_string(),
            "9999-12-31T23:59:59Z"
        );
        let too_late = "9999-12-31T23:59:59Z"
            .parse::<Timestamp>()
            .unwrap()
            .unix_seconds()
            + 1;
        assert_eq!(
            Timestamp::from_unix_seconds(too_late),
            Err(TimestampError::Range)
        );
    }

    #[test]
    fn serde_is_strict() {
        let ts: Timestamp = serde_json::from_str("\"2026-11-06T10:00:00Z\"").unwrap();
        assert_eq!(
            serde_json::to_string(&ts).unwrap(),
            "\"2026-11-06T10:00:00Z\""
        );
        assert!(serde_json::from_str::<Timestamp>("\"2026-11-06T10:00:00+00:00\"").is_err());
        assert!(serde_json::from_str::<Timestamp>("1793268000").is_err());
    }
}
