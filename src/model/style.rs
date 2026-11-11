//! Style model: plain data structs with serde, consumed by the QML generator.
//! No UI or QML dependencies in this module.

use serde::{Deserialize, Serialize};

/// RGBA color, 0..=255 per channel. QML strings are produced by the `qml` module.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0, a: 255 };

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    /// Convert to HSL for color-ramp generation.
    pub fn from_hsl(h_deg: f32, s: f32, l: f32, a: u8) -> Self {
        let h = ((h_deg % 360.0) + 360.0) % 360.0 / 360.0;
        let rgb = hsl_to_rgb(h, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
        Color {
            r: (rgb[0] * 255.0).round() as u8,
            g: (rgb[1] * 255.0).round() as u8,
            b: (rgb[2] * 255.0).round() as u8,
            a,
        }
    }
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    if s == 0.0 {
        return [l, l, l];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    [hue_to_rgb(p, q, h + 1.0 / 3.0), hue_to_rgb(p, q, h), hue_to_rgb(p, q, h - 1.0 / 3.0)]
}

fn hue_to_rgb(p: f32, q: f32, t: f32) -> f32 {
    let mut t = t;
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 0.5 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

/// Hatching rendered by QGIS `LinePatternFill`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hatch {
    /// Line angle in degrees (0..=359).
    pub angle: u16,
    /// Spacing between hatch lines in mm.
    pub distance_mm: f32,
    /// Hatch line width in mm.
    pub line_width_mm: f32,
    pub color: Color,
}

/// Outline / stroke properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outline {
    pub color: Color,
    pub width_mm: f32,
    /// Only "solid" is supported in Phase 1.
    pub style: String,
}

impl Default for Outline {
    fn default() -> Self {
        Outline {
            color: Color::new(0, 0, 0, 255),
            width_mm: 0.26,
            style: "solid".into(),
        }
    }
}

/// Fill style for polygon geometry: flat fill + optional hatching + outline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    pub color: Color,
    pub hatch: Option<Hatch>,
    pub outline: Outline,
}

impl Default for Fill {
    fn default() -> Self {
        Fill {
            color: Color::new(255, 0, 0, 128),
            hatch: None,
            outline: Outline::default(),
        }
    }
}

/// Line style for line geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineSymbol {
    pub color: Color,
    pub width_mm: f32,
}

impl Default for LineSymbol {
    fn default() -> Self {
        LineSymbol {
            color: Color::new(0, 102, 255, 255),
            width_mm: 0.86,
        }
    }
}

/// Marker style for point geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointSymbol {
    /// Marker diameter in mm.
    pub size_mm: f32,
    pub color: Color,
    pub outline: Outline,
}

impl Default for PointSymbol {
    fn default() -> Self {
        PointSymbol {
            size_mm: 2.0,
            color: Color::new(125, 139, 143, 255),
            outline: Outline {
                color: Color::new(35, 35, 35, 255),
                width_mm: 0.4,
                style: "solid".into(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererKind {
    SingleSymbol,
    Categorized,
}

/// One category of a categorized renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Category {
    /// The attribute value (as string) this category matches.
    pub value: String,
    pub label: String,
    pub fill: Fill,
    pub line: LineSymbol,
    pub point: PointSymbol,
    pub render: bool,
}

/// The complete style project, round-trippable as JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleProject {
    pub renderer: RendererKind,
    /// Attribute used by the categorized renderer.
    pub attr: Option<String>,
    /// Base symbols for single-symbol rendering.
    pub fill: Fill,
    pub line: LineSymbol,
    pub point: PointSymbol,
    pub categories: Vec<Category>,
}

impl Default for StyleProject {
    fn default() -> Self {
        StyleProject {
            renderer: RendererKind::SingleSymbol,
            attr: None,
            fill: Fill::default(),
            line: LineSymbol::default(),
            point: PointSymbol::default(),
            categories: Vec::new(),
        }
    }
}

impl StyleProject {
    /// Build categories for `attr` from the given distinct values, assigning a
    /// pleasing color ramp (golden-angle hue rotation at fixed saturation/lightness).
    pub fn build_categories(&mut self, attr: &str, values: &[String]) {
        self.attr = Some(attr.to_string());
        self.categories = values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let hue = (i as f32) * 137.508; // golden angle
                let c = Color::from_hsl(hue, 0.65, 0.55, 255);
                Category {
                    value: v.clone(),
                    label: v.clone(),
                    fill: Fill {
                        color: Color { a: 128, ..c },
                        hatch: self.fill.hatch.clone(),
                        outline: self.fill.outline.clone(),
                    },
                    line: LineSymbol { color: c, ..self.line.clone() },
                    point: PointSymbol { color: c, ..self.point.clone() },
                    render: true,
                }
            })
            .collect();
    }

    /// Bulk action: copy the hatch parameters of the first category (or the base fill)
    /// to every category.
    pub fn apply_hatch_to_all(&mut self, hatch: Option<Hatch>) {
        self.fill.hatch = hatch.clone();
        for c in &mut self.categories {
            c.fill.hatch = hatch.clone();
        }
    }

    /// Bulk action: copy the outline parameters to every category.
    pub fn apply_outline_to_all(&mut self, outline: Outline) {
        self.fill.outline = outline.clone();
        for c in &mut self.categories {
            c.fill.outline = outline.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_project_round_trips_through_json() {
        let mut style = StyleProject::default();
        style.renderer = RendererKind::Categorized;
        style.attr = Some("type".into());
        style.fill.hatch = Some(Hatch {
            angle: 45,
            distance_mm: 2.0,
            line_width_mm: 0.25,
            color: Color::BLACK,
        });
        style.build_categories("type", &["a".into(), "b".into()]);
        let json = serde_json::to_string_pretty(&style).unwrap();
        let back: StyleProject = serde_json::from_str(&json).unwrap();
        assert_eq!(style, back);
    }

    #[test]
    fn color_ramp_produces_distinct_pleasing_colors() {
        let values: Vec<String> = (0..8).map(|i| i.to_string()).collect();
        let mut style = StyleProject::default();
        style.build_categories("n", &values);
        let first = style.categories[0].fill.color;
        let second = style.categories[1].fill.color;
        assert_ne!(first, second);
        assert_eq!(style.categories.len(), 8);
        assert!(style.categories.iter().all(|c| c.render));
    }

    #[test]
    fn hsl_conversion_basics() {
        assert_eq!(Color::from_hsl(0.0, 1.0, 0.5, 255), Color::new(255, 0, 0, 255));
        assert_eq!(Color::from_hsl(120.0, 1.0, 0.5, 255), Color::new(0, 255, 0, 255));
        assert_eq!(Color::from_hsl(0.0, 0.0, 1.0, 255), Color::new(255, 255, 255, 255));
    }
}
