use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use anyhow::{Context, Result, bail, ensure};

/// A pair of SGF board coordinates encoded as two lowercase ASCII letters (`a`–`s`).
///
/// SGF uses column-first, row-second ordering (e.g. `dd` = column 4, row 4 in
/// 1-based terms, or the traditional go "4-4 point").  Coordinates are stored
/// as two 5-bit indices packed into a `u16`.
///
/// The special value `tt` (index 19 on each axis) represents a pass; use
/// [`GoCoord::pass`] to construct it and [`GoCoord::is_pass`] to detect it.
///
/// # Display coordinates vs. SGF coordinates
///
/// SGF uses `a`–`s` (19 consecutive letters, no gaps).  Board editors
/// conventionally display columns as `A`–`T` but skip `I` to avoid
/// confusion with the digit `1`;  this is just a rendering concern.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct GoCoord(u16);

/// Komi value stored internally as half-points to avoid floating-point rounding.
///
/// Standard komi is 6.5 points.  Displays as `"6.5"` or `"7"` depending on
/// whether the value is a half-point.
#[derive(Debug, Clone)]
pub struct Komi(i16);

/// Character encoding declared in the SGF `CA` property.
#[derive(Debug, Clone, Default)]
pub enum Charset {
    #[default]
    UTF8,
    Latin1,
    Other(String),
}

/// SGF file-format version declared in the `FF` property.
#[repr(u8)]
#[derive(Debug, Clone, Default)]
pub enum FileFormat {
    FF1 = 1,
    FF2,
    FF3,
    #[default]
    FF4,
}

/// Game type declared in the SGF `GM` property.
///
/// Only Go (`GM[1]`) is handled by the board simulator; all other values are
/// preserved as [`GameType::Other`].
#[repr(u8)]
#[derive(Debug, Clone, Default)]
pub enum GameType {
    #[default]
    Go = 1,
    Other(u8),
}

/// A single [SGF property](https://www.red-bean.com/sgf/properties.html) parsed from a node.
///
/// Unrecognized properties are captured as [`SGFProperty::Unknown`] so they
/// can be captured in [`parse_sgf`](crate::parse_sgf) and
/// [`write_sgf`](crate::write_sgf) without loss.
#[derive(Debug, Clone)]
pub enum SGFProperty {
    /// `AP` — name of the application that created the file.
    AP(String),

    /// `B` — a black move.  The coordinate `tt` represents a pass;
    /// use [`GoCoord::is_pass`] to check.
    B(GoCoord),

    /// `CA` — character encoding of the file (e.g. UTF-8).
    CA(Charset),

    /// `DT` — date the game was played (free-form string, e.g. `"1846-09-11"`).
    DT(String),

    /// `FF` — SGF file-format version.
    FF(FileFormat),

    /// `GM` — game type. `GM[1]` is Go; other values are stored as
    /// [`GameType::Other`] and are not simulated by [`Board`](crate::sgf::Board).
    GM(GameType),

    /// `KM` — komi (points given to white to compensate for black's first-move
    /// advantage).  Stored in half-points to avoid floating-point rounding.
    KM(Komi),

    /// `W` — a white move.  The coordinate `tt` represents a pass;
    /// use [`GoCoord::is_pass`] to check.
    W(GoCoord),

    /// `SZ` — board size in intersections (e.g. `19` for a standard board).
    /// Only 19×19 boards currently supported by the board simulator.
    SZ(u8),

    /// `AB` — add black stones (setup, not a move).  Used for handicap placement
    /// and problem diagrams.  Does not increment the move counter.
    AB(Vec<GoCoord>),

    /// `AW` — add white stones (setup, not a move).  Does not increment the
    /// move counter.
    AW(Vec<GoCoord>),

    /// `PB` — black player's name.
    PB(String),

    /// `PW` — white player's name.
    PW(String),

    /// `BR` — black player's rank (e.g. `"9p"`, `"3k"`).
    BR(String),

    /// `WR` — white player's rank.
    WR(String),

    /// `HA` — handicap stone count.  The actual stone placements are given by
    /// [`SGFProperty::AB`].
    HA(u8),

    /// `RE` — game result (free-form string, e.g. `"B+3.5"`, `"W+R"`, `"0"`).
    RE(String),

    /// `C` — node comment.
    C(String),

    /// Any property tag not recognized by the parser.  The first field is the
    /// raw tag string (e.g. `"LB"`); the second is the list of raw value
    /// strings.
    Unknown(String, Vec<String>),
}

impl GoCoord {
    /// Construct a coordinate from two SGF characters.
    ///
    /// Both characters must be ASCII lowercase letters in the range `a`–`s`
    /// (0–18, inclusive).
    ///
    /// # Errors
    ///
    /// Returns an error if either character is outside `a`–`s`.
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

    /// Construct a GoCoord from (col, row) 0-based indices.
    /// `col` maps to the first SGF character, `row` to the second.
    pub fn from_colrow(col: usize, row: usize) -> Self {
        Self(col as u16 | ((row as u16) << 5))
    }

    /// The conventional SGF pass coordinate `tt` (index 19 in each axis).
    pub fn pass() -> Self {
        Self(19 | (19 << 5))
    }

    /// Return `true` if this coordinate represents a pass (`tt`).
    pub fn is_pass(self) -> bool {
        (self.0 & 0b11111) == 19
    }

    /// The column character (first SGF character, e.g. `'d'` in `dd`).
    pub fn first(self) -> char {
        (b'a' + (self.0 & 0b11111) as u8) as char
    }

    /// The row character (second SGF character, e.g. `'d'` in `dd`).
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

impl Display for Komi {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let n = self.0;
        if n % 2 == 0 {
            write!(f, "{}", n / 2)
        } else {
            write!(f, "{}.5", n / 2)
        }
    }
}

impl Display for FileFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let n = match self {
            Self::FF1 => 1,
            Self::FF2 => 2,
            Self::FF3 => 3,
            Self::FF4 => 4,
        };
        write!(f, "{}", n)
    }
}

impl Display for GameType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let n = match self {
            Self::Go => 1,
            Self::Other(other) => *other,
        };
        write!(f, "{}", n)
    }
}

impl Display for Charset {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::UTF8 => "UTF-8",
            Self::Latin1 => "Latin-1",
            Self::Other(other) => other,
        };
        write!(f, "{}", s)
    }
}

impl Display for SGFProperty {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AP(s) => write!(f, "AP[{}]", s),
            Self::B(coord) => write!(f, "B[{}]", coord),
            Self::W(coord) => write!(f, "W[{}]", coord),
            Self::AB(coords) => {
                write!(f, "AB")?;
                for coord in coords {
                    write!(f, "[{}]", coord)?;
                }
                Ok(())
            }
            Self::AW(coords) => {
                write!(f, "AW")?;
                for coord in coords {
                    write!(f, "[{}]", coord)?;
                }
                Ok(())
            }
            Self::CA(charset) => write!(f, "CA[{}]", charset),
            Self::DT(s) => write!(f, "DT[{}]", s),
            Self::FF(ff) => write!(f, "FF[{}]", ff),
            Self::GM(gt) => write!(f, "GM[{}]", gt),
            Self::KM(komi) => write!(f, "KM[{}]", komi),
            Self::SZ(n) => write!(f, "SZ[{}]", n),
            Self::PB(s) => write!(f, "PB[{}]", s),
            Self::PW(s) => write!(f, "PW[{}]", s),
            Self::BR(s) => write!(f, "BR[{}]", s),
            Self::WR(s) => write!(f, "WR[{}]", s),
            Self::HA(n) => write!(f, "HA[{}]", n),
            Self::RE(s) => write!(f, "RE[{}]", s),
            Self::C(s) => write!(f, "C[{}]", s),
            Self::Unknown(key, values) => {
                write!(f, "{}", key)?;
                for v in values {
                    write!(f, "[{}]", v)?;
                }
                Ok(())
            }
        }
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
