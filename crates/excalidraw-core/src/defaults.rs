//! Excalidraw compatibility defaults shared by parser, validation, and render code.

use crate::types::{FillStyle, StrokeStyle, TextAlign, VerticalAlign};

pub(crate) fn default_file_type() -> String {
    "excalidraw".to_owned()
}

pub(crate) fn default_version() -> u32 {
    2
}

pub(crate) fn default_stroke_color() -> String {
    "#1e1e1e".to_owned()
}

pub(crate) fn default_background_color() -> String {
    "transparent".to_owned()
}

pub(crate) fn default_fill_style() -> FillStyle {
    FillStyle::Hachure
}

pub(crate) fn default_stroke_width() -> f64 {
    2.0
}

pub(crate) fn default_stroke_style() -> StrokeStyle {
    StrokeStyle::Solid
}

pub(crate) fn default_roughness() -> f64 {
    1.0
}

pub(crate) fn default_opacity() -> f64 {
    100.0
}

pub(crate) fn default_font_size() -> f64 {
    20.0
}

pub(crate) fn default_font_family() -> u32 {
    5
}

pub(crate) fn default_text_align() -> TextAlign {
    TextAlign::Left
}

pub(crate) fn default_vertical_align() -> VerticalAlign {
    VerticalAlign::Top
}

pub(crate) fn default_line_height() -> f64 {
    1.25
}

/// Excalidraw numeric font-family compatibility mapping.
#[must_use]
pub fn font_family_css(family: u32) -> &'static str {
    match family {
        1 => "Virgil, Excalifont, cursive",
        2 => "Helvetica, Arial, sans-serif",
        3 => "Cascadia Code, Courier New, monospace",
        5 => "Excalifont, cursive",
        6 => "Nunito, sans-serif",
        7 => "Lilita One, cursive",
        8 => "Comic Shanns, Comic Sans MS, cursive",
        _ => "Excalifont, cursive",
    }
}

/// Primary font name used for registration and renderer lookup.
#[must_use]
pub fn font_family_primary(family: u32) -> &'static str {
    match family {
        1 => "Virgil",
        2 => "Helvetica",
        3 => "Cascadia Code",
        5 => "Excalifont",
        6 => "Nunito",
        7 => "Lilita One",
        8 => "Comic Shanns",
        _ => "Excalifont",
    }
}

/// Average advance factor used for deterministic fallback text measurement.
#[must_use]
pub fn font_family_width_factor(family: u32) -> f64 {
    match family {
        2 | 6 => 0.55,
        3 => 0.62,
        7 => 0.66,
        _ => 0.6,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::types::{FillStyle, StrokeStyle, TextAlign, VerticalAlign};

    #[test]
    fn default_values_match_excalidraw_compatibility_contract() -> Result<(), Box<dyn Error>> {
        ensure_eq(&super::default_file_type(), "excalidraw", "file type")?;
        ensure_eq(&super::default_version(), 2_u32, "version")?;
        ensure_eq(&super::default_stroke_color(), "#1e1e1e", "stroke color")?;
        ensure_eq(
            &super::default_background_color(),
            "transparent",
            "background color",
        )?;
        ensure_eq(
            &super::default_fill_style(),
            FillStyle::Hachure,
            "fill style",
        )?;
        ensure_eq(&super::default_stroke_width(), 2.0_f64, "stroke width")?;
        ensure_eq(
            &super::default_stroke_style(),
            StrokeStyle::Solid,
            "stroke style",
        )?;
        ensure_eq(&super::default_roughness(), 1.0_f64, "roughness")?;
        ensure_eq(&super::default_opacity(), 100.0_f64, "opacity")?;
        ensure_eq(&super::default_font_size(), 20.0_f64, "font size")?;
        ensure_eq(&super::default_font_family(), 5_u32, "font family")?;
        ensure_eq(&super::default_text_align(), TextAlign::Left, "text align")?;
        ensure_eq(
            &super::default_vertical_align(),
            VerticalAlign::Top,
            "vertical align",
        )?;
        ensure_eq(&super::default_line_height(), 1.25_f64, "line height")?;
        Ok(())
    }

    #[test]
    fn font_family_css_matches_versioned_mapping() -> Result<(), Box<dyn Error>> {
        ensure_eq(
            &super::font_family_css(1),
            "Virgil, Excalifont, cursive",
            "family 1",
        )?;
        ensure_eq(
            &super::font_family_css(2),
            "Helvetica, Arial, sans-serif",
            "family 2",
        )?;
        ensure_eq(
            &super::font_family_css(3),
            "Cascadia Code, Courier New, monospace",
            "family 3",
        )?;
        ensure_eq(
            &super::font_family_css(5),
            "Excalifont, cursive",
            "family 5",
        )?;
        ensure_eq(&super::font_family_css(6), "Nunito, sans-serif", "family 6")?;
        ensure_eq(
            &super::font_family_css(7),
            "Lilita One, cursive",
            "family 7",
        )?;
        ensure_eq(
            &super::font_family_css(8),
            "Comic Shanns, Comic Sans MS, cursive",
            "family 8",
        )?;
        ensure_eq(
            &super::font_family_css(99),
            "Excalifont, cursive",
            "unknown family fallback",
        )?;
        Ok(())
    }

    #[test]
    fn font_family_registration_and_measurement_mapping_is_stable() -> Result<(), Box<dyn Error>> {
        ensure_eq(&super::font_family_primary(3), "Cascadia Code", "primary")?;
        ensure_eq(
            &super::font_family_primary(99),
            "Excalifont",
            "fallback primary",
        )?;
        ensure_eq(&super::font_family_width_factor(3), 0.62_f64, "mono width")?;
        ensure_eq(&super::font_family_width_factor(2), 0.55_f64, "sans width")?;
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
