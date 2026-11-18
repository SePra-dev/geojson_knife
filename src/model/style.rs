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
    Graduated,
}

/// Classification method for graduated (class-break) rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationMethod {
    EqualInterval,
    Quantile,
}

/// One class break of a graduated renderer: lower <= value < upper
/// (the last break is inclusive of its upper bound).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassBreak {
    pub lower: f64,
    pub upper: f64,
    pub fill: Fill,
    pub line: LineSymbol,
    pub point: PointSymbol,
    pub render: bool,
}

/// Configuration + computed breaks of a graduated renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraduatedConfig {
    pub attr: String,
    pub method: ClassificationMethod,
    pub n_classes: u8,
    /// Two ramp stop colors; class colors interpolate between them.
    pub ramp_low: Color,
    pub ramp_high: Color,
    /// Computed class breaks (built by `build_breaks`).
    pub breaks: Vec<ClassBreak>,
}

impl Default for GraduatedConfig {
    fn default() -> Self {
        GraduatedConfig {
            attr: String::new(),
            method: ClassificationMethod::EqualInterval,
            n_classes: 5,
            ramp_low: Color::new(255, 245, 240, 255),
            ramp_high: Color::new(103, 0, 13, 255),
            breaks: Vec::new(),
        }
    }
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
    /// Configuration and computed breaks for the graduated renderer.
    pub graduated: GraduatedConfig,
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
            graduated: GraduatedConfig::default(),
        }
    }
}

impl Color {
    /// Linear interpolation between two colors (RGB, alpha included).
    pub fn lerp(a: Color, b: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color {
            r: (a.r as f32 + (b.r as f32 - a.r as f32) * t).round() as u8,
            g: (a.g as f32 + (b.g as f32 - a.g as f32) * t).round() as u8,
            b: (a.b as f32 + (b.b as f32 - a.b as f32) * t).round() as u8,
            a: (a.a as f32 + (b.a as f32 - a.a as f32) * t).round() as u8,
        }
    }
}

/// Compute class-break edges (lower/upper bounds) from numeric values.
/// Pure function; returns `n_classes` monotonically increasing breaks.
pub fn classify_breaks(values: &[f64], n_classes: u8, method: ClassificationMethod) -> Vec<(f64, f64)> {
    let n_classes = n_classes.max(1) as usize;
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if sorted.is_empty() {
        return vec![(0.0, 1.0)];
    }
    let min = sorted[0];
    max_quiet(&sorted)
        .map(|max| {
            if (max - min).abs() < f64::EPSILON {
                // All values identical: n_classes with zero width, except first/last.
                let mut out = Vec::with_capacity(n_classes);
                for _ in 0..n_classes {
                    out.push((min, min));
                }
                out[0].0 = min - 0.5;
                out[n_classes - 1].1 = min + 0.5;
                return out;
            }
            let mut edges: Vec<f64> = match method {
                ClassificationMethod::EqualInterval => {
                    let step = (max - min) / n_classes as f64;
                    (0..=n_classes).map(|i| min + step * i as f64).collect()
                }
                ClassificationMethod::Quantile => {
                    let q = |p: f64| -> f64 {
                        if sorted.len() == 1 {
                            return sorted[0];
                        }
                        let pos = p * (sorted.len() - 1) as f64;
                        let lo = pos.floor() as usize;
                        let hi = pos.ceil() as usize;
                        let frac = pos - lo as f64;
                        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
                    };
                    (0..=n_classes)
                        .map(|i| q(i as f64 / n_classes as f64))
                        .collect()
                }
            };
            // Force last edge to at least the true max so no value is dropped.
            let last = edges.len() - 1;
            if edges[last] < max {
                edges[last] = max;
            }
            edges
                .windows(2)
                .map(|w| (w[0], w[1]))
                .collect()
        })
        .unwrap_or_else(|| vec![(min, min)])
}

fn max_quiet(sorted: &[f64]) -> Option<f64> {
    sorted.iter().copied().fold(None, |acc, v| match acc {
        Some(m) if m >= v => Some(m),
        _ => Some(v),
    })
}

impl StyleProject {
    /// Build the class breaks for the graduated renderer from numeric values.
    /// Class colors interpolate between the configured ramp stops; the base
    /// symbol (outline, hatch, sizes) is inherited from the single-symbol style.
    pub fn build_breaks(&mut self, values: &[f64]) {
        let n = self.graduated.n_classes.max(1);
        let pairs = classify_breaks(values, n, self.graduated.method);
        self.graduated.breaks = pairs
            .iter()
            .enumerate()
            .map(|(i, (lower, upper))| {
                let t = if pairs.len() <= 1 {
                    0.5
                } else {
                    i as f32 / (pairs.len() - 1) as f32
                };
                let c = Color::lerp(self.graduated.ramp_low, self.graduated.ramp_high, t);
                ClassBreak {
                    lower: *lower,
                    upper: *upper,
                    fill: Fill {
                        color: Color { a: self.fill.color.a, ..c },
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
    fn graduated_config_round_trips() {
        let mut style = StyleProject::default();
        style.renderer = RendererKind::Graduated;
        style.graduated.attr = "size".into();
        style.graduated.method = ClassificationMethod::Quantile;
        style.graduated.n_classes = 4;
        style.build_breaks(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let json = serde_json::to_string_pretty(&style).unwrap();
        let back: StyleProject = serde_json::from_str(&json).unwrap();
        assert_eq!(style, back);
    }

    #[test]
    fn equal_interval_breaks_are_monotonic() {
        let values: Vec<f64> = (0..40).map(|i| i as f64 * 2.5).collect();
        let breaks = classify_breaks(&values, 5, ClassificationMethod::EqualInterval);
        assert_eq!(breaks.len(), 5);
        assert_eq!(breaks[0].0, 0.0);
        assert_eq!(breaks[4].1, 97.5);
        for w in breaks.windows(2) {
            assert!(w[0].1 <= w[1].0 + 1e-9);
        }
        // every value falls into some break (within float tolerance)
        for v in &values {
            assert!(breaks.iter().any(|(lo, hi)| *v >= *lo && *v <= *hi + 1e-9));
        }
    }

    #[test]
    fn quantile_breaks_split_data_evenly() {
        let values: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let breaks = classify_breaks(&values, 4, ClassificationMethod::Quantile);
        assert_eq!(breaks.len(), 4);
        // Interpolated quantiles split approximately evenly (25 +/- 1 per break)
        for (i, (lo, hi)) in breaks.iter().enumerate() {
            let upper_incl = if i == breaks.len() - 1 { *hi } else { f64::NEG_INFINITY };
            let count = values
                .iter()
                .filter(|v| **v >= *lo && (**v < *hi || **v <= upper_incl))
                .count();
            assert!((24..=26).contains(&count), "break {i}: [{lo}, {hi}) had {count}");
        }
    }

    #[test]
    fn identical_values_do_not_panic() {
        let breaks = classify_breaks(&[5.0; 10], 3, ClassificationMethod::EqualInterval);
        assert_eq!(breaks.len(), 3);
    }

    #[test]
    fn empty_values_give_fallback_breaks() {
        let breaks = classify_breaks(&[], 3, ClassificationMethod::EqualInterval);
        assert_eq!(breaks, vec![(0.0, 1.0)]);
    }

    #[test]
    fn nan_values_do_not_panic() {
        let breaks = classify_breaks(&[1.0, f64::NAN, 3.0], 2, ClassificationMethod::EqualInterval);
        assert_eq!(breaks.len(), 2);
    }

    #[test]
    fn ramp_colors_interpolate() {
        let a = Color::new(0, 0, 0, 255);
        let b = Color::new(255, 255, 255, 255);
        assert_eq!(Color::lerp(a, b, 0.0), a);
        assert_eq!(Color::lerp(a, b, 1.0), b);
        assert_eq!(Color::lerp(a, b, 0.5), Color::new(128, 128, 128, 255));
        // out-of-range t is clamped
        assert_eq!(Color::lerp(a, b, 2.0), b);
    }

    #[test]
    fn hsl_conversion_basics() {
        assert_eq!(Color::from_hsl(0.0, 1.0, 0.5, 255), Color::new(255, 0, 0, 255));
        assert_eq!(Color::from_hsl(120.0, 1.0, 0.5, 255), Color::new(0, 255, 0, 255));
        assert_eq!(Color::from_hsl(0.0, 0.0, 1.0, 255), Color::new(255, 255, 255, 255));
    }
}
