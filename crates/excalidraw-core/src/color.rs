//! Excalidraw color parsing helpers.

use thiserror::Error;

/// Parsed sRGB color with normalized alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Color {
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    #[must_use]
    pub const fn transparent() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ColorParseError {
    #[error("color is empty")]
    Empty,

    #[error("unsupported color format: {0}")]
    UnsupportedFormat(String),

    #[error("invalid hex color digit in {0}")]
    InvalidHex(String),
}

/// Parse an Excalidraw-style color value.
pub fn parse_excalidraw_color(input: &str) -> Result<Color, ColorParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ColorParseError::Empty);
    }

    if trimmed.eq_ignore_ascii_case("transparent") {
        return Ok(Color::transparent());
    }

    let Some(hex) = trimmed.strip_prefix('#') else {
        return Err(ColorParseError::UnsupportedFormat(trimmed.to_owned()));
    };

    match hex.len() {
        3 => parse_short_hex(hex),
        6 => parse_long_hex(hex),
        8 => parse_long_hex_with_alpha(hex),
        _ => Err(ColorParseError::UnsupportedFormat(trimmed.to_owned())),
    }
}

fn parse_short_hex(hex: &str) -> Result<Color, ColorParseError> {
    let mut chars = hex.chars();
    let Some(r) = chars.next() else {
        return Err(ColorParseError::InvalidHex(hex.to_owned()));
    };
    let Some(g) = chars.next() else {
        return Err(ColorParseError::InvalidHex(hex.to_owned()));
    };
    let Some(b) = chars.next() else {
        return Err(ColorParseError::InvalidHex(hex.to_owned()));
    };

    Ok(Color::rgb(
        hex_digit(r, hex)? * 17,
        hex_digit(g, hex)? * 17,
        hex_digit(b, hex)? * 17,
    ))
}

fn parse_long_hex(hex: &str) -> Result<Color, ColorParseError> {
    Ok(Color::rgb(
        hex_pair(hex, 0)?,
        hex_pair(hex, 2)?,
        hex_pair(hex, 4)?,
    ))
}

fn parse_long_hex_with_alpha(hex: &str) -> Result<Color, ColorParseError> {
    let alpha = f32::from(hex_pair(hex, 6)?) / 255.0;
    Ok(Color {
        r: hex_pair(hex, 0)?,
        g: hex_pair(hex, 2)?,
        b: hex_pair(hex, 4)?,
        a: alpha,
    })
}

fn hex_pair(hex: &str, start: usize) -> Result<u8, ColorParseError> {
    let mut chars = hex.chars().skip(start);
    let Some(high) = chars.next() else {
        return Err(ColorParseError::InvalidHex(hex.to_owned()));
    };
    let Some(low) = chars.next() else {
        return Err(ColorParseError::InvalidHex(hex.to_owned()));
    };

    Ok(hex_digit(high, hex)? * 16 + hex_digit(low, hex)?)
}

fn hex_digit(value: char, original: &str) -> Result<u8, ColorParseError> {
    let Some(digit) = value.to_digit(16) else {
        return Err(ColorParseError::InvalidHex(original.to_owned()));
    };

    u8::try_from(digit).map_err(|_| ColorParseError::InvalidHex(original.to_owned()))
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{parse_excalidraw_color, Color, ColorParseError};

    #[test]
    fn parses_transparent_and_hex_colors() -> Result<(), Box<dyn Error>> {
        ensure_eq(
            &parse_excalidraw_color("transparent")?,
            Color::transparent(),
            "transparent",
        )?;
        ensure_eq(
            &parse_excalidraw_color("#fff")?,
            Color::rgb(255, 255, 255),
            "short hex",
        )?;
        ensure_eq(
            &parse_excalidraw_color("#1e1e1e")?,
            Color::rgb(30, 30, 30),
            "long hex",
        )?;

        let with_alpha = parse_excalidraw_color("#ff000080")?;
        ensure_eq(&with_alpha.r, 255_u8, "alpha red channel")?;
        ensure_eq(&with_alpha.g, 0_u8, "alpha green channel")?;
        ensure_eq(&with_alpha.b, 0_u8, "alpha blue channel")?;
        if (with_alpha.a - 0.501_960_8).abs() < f32::EPSILON {
            Ok(())
        } else {
            Err(format!(
                "alpha: expected approximately 0.5019608, got {}",
                with_alpha.a
            )
            .into())
        }
    }

    #[test]
    fn rejects_invalid_colors_with_structured_errors() -> Result<(), Box<dyn Error>> {
        ensure_eq(
            &parse_excalidraw_color(""),
            Err(ColorParseError::Empty),
            "empty color",
        )?;
        ensure_eq(
            &parse_excalidraw_color("red"),
            Err(ColorParseError::UnsupportedFormat("red".to_owned())),
            "named color",
        )?;
        ensure_eq(
            &parse_excalidraw_color("#12"),
            Err(ColorParseError::UnsupportedFormat("#12".to_owned())),
            "bad length",
        )?;
        ensure_eq(
            &parse_excalidraw_color("#xxf"),
            Err(ColorParseError::InvalidHex("xxf".to_owned())),
            "bad digit",
        )?;
        Ok(())
    }

    fn ensure_eq<T, U>(actual: &T, expected: U, label: &str) -> Result<(), Box<dyn Error>>
    where
        T: PartialEq<U> + std::fmt::Debug,
        U: std::fmt::Debug,
    {
        if actual.eq(&expected) {
            Ok(())
        } else {
            Err(format!("{label}: expected {expected:?}, got {actual:?}").into())
        }
    }
}
