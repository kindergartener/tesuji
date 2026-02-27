use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use anyhow::{Context, Result, bail, ensure};

/// Encodes Go board coordinates as two lowercase letters from a-s
/// Stored as two 5-bit values packed into a u16:
/// bits [4:0] = first
/// bits [9:5] = second
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct GoCoord(u16);

#[derive(Debug, Clone)]
pub struct Komi(i16);

#[derive(Debug, Clone, Default)]
pub enum Charset {
    #[default]
    UTF8,
    Latin1,
    Other(String),
}

#[repr(u8)]
#[derive(Debug, Clone, Default)]
pub enum FileFormat {
    FF1 = 1,
    FF2,
    FF3,
    #[default]
    FF4,
}

#[repr(u8)]
#[derive(Debug, Clone, Default)]
pub enum GameType {
    #[default]
    Go = 1,
    Other(u8),
}

#[derive(Debug, Clone)]
pub enum SGFProperty {
    /// Application
    AP(String),

    /// Black move
    B(GoCoord),

    /// Charset
    CA(Charset),

    /// Date
    DT(String),

    /// File format
    FF(FileFormat),

    /// Game
    GM(GameType),

    /// Komi (in half-points)
    KM(Komi),

    /// White move
    W(GoCoord),

    /// Board size
    SZ(u8),

    /// Add black stones for handicap games
    AB(Vec<GoCoord>),

    /// Add white stones for handicap games
    AW(Vec<GoCoord>),

    /// Black player name
    PB(String),

    /// White player name
    PW(String),

    /// Result
    RE(String),

    /// Comment
    C(String),

    /// Application-specific or unrecognized property
    Unknown(String, Vec<String>),
}

impl GoCoord {
    pub fn new(a: char, b: char) -> Result<Self> {
        let encode = |c: char| -> Option<u16> {
            if c.is_ascii_lowercase() {
                let v = c as u16 - b'a' as u16;
                if v < 19 { Some(v) } else { None }
            } else {
                None
            }
        };

        let a = encode(a).context(format!("Invalid Go coordinate: first char {:?}", a))?;
        let b = encode(b).context(format!("Invalid Go coordinate: second char {:?}", b))?;

        Ok(Self(a | (b << 5)))
    }

    pub fn first(self) -> char {
        (b'a' + (self.0 & 0b11111) as u8) as char
    }

    pub fn second(self) -> char {
        (b'a' + ((self.0 >> 5) & 0b11111) as u8) as char
    }
}

impl Display for GoCoord {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.first(), self.second())
    }
}

impl FromStr for GoCoord {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut chars = s.chars();
        let a = chars
            .next()
            .context(format!("Invalid Go coordinate {:?}: too short", s))?;
        let b = chars
            .next()
            .context(format!("Invalid Go coordinate {:?}: too short", s))?;

        ensure!(
            chars.next().is_none(),
            format!("Invalid Go coordinate {:?}: too long", s)
        );

        Self::new(a, b)
    }
}

impl Komi {
    fn new(n: f64) -> Self {
        Komi((n * 2.0).round() as i16)
    }
}

impl Default for Komi {
    fn default() -> Self {
        Komi::new(6.5)
    }
}

impl FromStr for Komi {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let n: f64 = s.parse().context("Komi must be a number")?;
        Ok(Komi::new(n))
    }
}

impl FromStr for GameType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let n: u8 = s.parse().context("GameType must be a number")?;
        Ok(match n {
            1 => GameType::Go,
            n => GameType::Other(n),
        })
    }
}

impl FromStr for FileFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "1" => FileFormat::FF1,
            "2" => FileFormat::FF2,
            "3" => FileFormat::FF3,
            "4" => FileFormat::FF4,
            _ => bail!("FileType must be a number 1-4"),
        })
    }
}

impl FromStr for Charset {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "UTF-8" | "utf-8" => Ok(Charset::UTF8),
            "Latin-1" | "ISO-8859-1" => Ok(Charset::Latin1),
            other => Ok(Charset::Other(other.to_string())),
        }
    }
}
