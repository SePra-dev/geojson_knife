//! Pure QML (QGIS 3.x style file) generation: style model -> XML string.
//! No UI dependencies. The emitted structure mirrors what QGIS 3.28 itself
//! saves for `singleSymbol` / `categorizedSymbol` renderers with `SimpleFill`,
//! `LinePatternFill`, `SimpleLine` and `SimpleMarker` symbol layers.

use crate::model::{
    Category, Color, Fill, GeometryKind, LineSymbol, PointSymbol, RendererKind, StyleProject,
};

const QGIS_VERSION: &str = "3.28";

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// QGIS stores colors as `r,g,b,alpha` strings (0-255 each).
pub fn color_string(c: Color) -> String {
    format!("{},{},{},{}", c.r, c.g, c.b, c.a)
}

fn fmt(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() { "0".into() } else { s.into() }
}

fn option(name: &str, value: &str) -> String {
    format!(
        "            <Option type=\"QString\" name=\"{}\" value=\"{}\"/>\n",
        esc(name),
        esc(value)
    )
}

fn layer_header(class: &str, pass: u8) -> String {
    format!(
        "      <layer class=\"{}\" enabled=\"1\" locked=\"0\" pass=\"{}\">\n",
        class,
        pass
    )
}

fn simple_fill(f: &Fill) -> String {
    let mut xml = String::new();
    if let Some(h) = &f.hatch {
        xml.push_str(&layer_header("LinePatternFill", 0));
        xml.push_str("        <Option type=\"Map\">\n");
        xml.push_str(&option("angle", &h.angle.to_string()));
        xml.push_str(&option("color", &color_string(h.color)));
        xml.push_str(&option("distance", &fmt(h.distance_mm)));
        xml.push_str(&option("distance_unit", "MM"));
        xml.push_str(&option("line_width", &fmt(h.line_width_mm)));
        xml.push_str(&option("line_width_unit", "MM"));
        xml.push_str(&option("offset", "0"));
        xml.push_str(&option("offset_unit", "MM"));
        xml.push_str(&option("outline_width_unit", "MM"));
        xml.push_str("        </Option>\n");
        xml.push_str("      </layer>\n");
    }
    xml.push_str(&layer_header("SimpleFill", 0));
    xml.push_str("        <Option type=\"Map\">\n");
    xml.push_str(&option("color", &color_string(f.color)));
    xml.push_str(&option("outline_color", &color_string(f.outline.color)));
    xml.push_str(&option("outline_style", &f.outline.style));
    xml.push_str(&option("outline_width", &fmt(f.outline.width_mm)));
    xml.push_str(&option("style", "solid"));
    xml.push_str("        </Option>\n");
    xml.push_str("      </layer>\n");
    xml
}

fn simple_line(l: &LineSymbol) -> String {
    let mut xml = layer_header("SimpleLine", 0);
    xml.push_str("        <Option type=\"Map\">\n");
    xml.push_str(&option("capstyle", "square"));
    xml.push_str(&option("customdash", "5;2"));
    xml.push_str(&option("customdash_unit", "MM"));
    xml.push_str(&option("draw_inside_polygon", "0"));
    xml.push_str(&option("joinstyle", "bevel"));
    xml.push_str(&option("line_color", &color_string(l.color)));
    xml.push_str(&option("line_style", "solid"));
    xml.push_str(&option("line_width", &fmt(l.width_mm)));
    xml.push_str(&option("line_width_unit", "MM"));
    xml.push_str(&option("offset", "0"));
    xml.push_str(&option("offset_unit", "MM"));
    xml.push_str(&option("ring_filter", "0"));
    xml.push_str(&option("trim_distance_end", "0"));
    xml.push_str(&option("trim_distance_start", "0"));
    xml.push_str(&option("trim_distance_unit", "MM"));
    xml.push_str(&option("tweak_dash_pattern_on_corners", "0"));
    xml.push_str(&option("use_custom_dash", "0"));
    xml.push_str(&option("width_map_unit_scale", "3x:0,0,0,0,0,0"));
    xml.push_str("        </Option>\n");
    xml.push_str("      </layer>\n");
    xml
}

fn simple_marker(p: &PointSymbol) -> String {
    let mut xml = layer_header("SimpleMarker", 0);
    xml.push_str("        <Option type=\"Map\">\n");
    xml.push_str(&option("angle", "0"));
    xml.push_str(&option("capstyle", "square"));
    xml.push_str(&option("name", "circle"));
    xml.push_str(&option("color", &color_string(p.color)));
    xml.push_str(&option("outline_color", &color_string(p.outline.color)));
    xml.push_str(&option("outline_style", &p.outline.style));
    xml.push_str(&option("outline_width", &fmt(p.outline.width_mm)));
    xml.push_str(&option("outline_width_unit", "MM"));
    xml.push_str(&option("size", &fmt(p.size_mm)));
    xml.push_str(&option("size_unit", "MM"));
    xml.push_str("        </Option>\n");
    xml.push_str("      </layer>\n");
    xml
}

fn symbol_xml(name: &str, kind: GeometryKind, style: &StyleProject, cat: Option<&Category>) -> String {
    let (sym_type, body) = match kind.symbol_family() {
        "fill" => {
            let f = cat.map(|c| &c.fill).unwrap_or(&style.fill);
            ("fill", simple_fill(f))
        }
        "line" => {
            let l = cat.map(|c| &c.line).unwrap_or(&style.line);
            ("line", simple_line(l))
        }
        _ => {
            let p = cat.map(|c| &c.point).unwrap_or(&style.point);
            ("marker", simple_marker(p))
        }
    };
    format!(
        "    <symbol type=\"{sym_type}\" name=\"{name}\" force_raster=\"0\" alpha=\"1\" is_animated=\"0\" frame_rate=\"10\" clip_to_extent=\"0\">\n{body}    </symbol>\n"
    )
}

/// QGIS quotes the attribute name inside `attr="..."` so that names with spaces work.
fn attr_expression(attr: &str) -> String {
    format!("\"{attr}\"")
}

/// Generate a complete QGIS 3.x `.qml` style file for the given style project and
/// target geometry kind.
pub fn generate_qml(style: &StyleProject, kind: GeometryKind) -> String {
    let mut xml = String::new();
    xml.push_str("<!DOCTYPE qgis PUBLIC 'http://mrcc.com/qgis.dtd' 'SYSTEM'>\n");
    xml.push_str(&format!(
        "<qgis version=\"{QGIS_VERSION}\" styleCategories=\"Symbology\">\n"
    ));

    match style.renderer {
        RendererKind::SingleSymbol => {
            xml.push_str("  <renderer-v2 type=\"singleSymbol\" symbol=\"0\" enableorderby=\"0\" forceraster=\"0\">\n");
            xml.push_str("    <symbols>\n");
            xml.push_str(&symbol_xml("0", kind, style, None));
            xml.push_str("    </symbols>\n");
            xml.push_str("  </renderer-v2>\n");
        }
        RendererKind::Categorized => {
            let attr = style.attr.clone().unwrap_or_default();
            xml.push_str(&format!(
                "  <renderer-v2 attr=\"{}\" type=\"categorizedSymbol\" symbol=\"0\" enableorderby=\"0\" forceraster=\"0\">\n",
                esc(&attr_expression(&attr))
            ));
            xml.push_str("    <categories>\n");
            for (i, c) in style.categories.iter().enumerate() {
                let render = if c.render { "true" } else { "false" };
                xml.push_str(&format!(
                    "      <category render=\"{}\" symbol=\"{}\" label=\"{}\" value=\"{}\"/>\n",
                    render,
                    i,
                    esc(&c.label),
                    esc(&c.value)
                ));
            }
            xml.push_str("    </categories>\n");
            xml.push_str("    <symbols>\n");
            for (i, c) in style.categories.iter().enumerate() {
                xml.push_str(&symbol_xml(&i.to_string(), kind, style, Some(c)));
            }
            xml.push_str("    </symbols>\n");
            xml.push_str("  </renderer-v2>\n");
        }
    }

    xml.push_str("</qgis>\n");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Fill, Hatch, Outline};

    fn base_style() -> StyleProject {
        StyleProject::default()
    }

    /// Read a golden file, normalizing line endings (git may check out CRLF on Windows).
    fn read_golden(name: &str) -> String {
        let raw = std::fs::read_to_string(format!("tests/golden/{name}")).unwrap();
        raw.replace("\r\n", "\n")
    }

    #[test]
    fn color_string_format() {
        assert_eq!(color_string(Color::new(255, 0, 0, 128)), "255,0,0,128");
        assert_eq!(color_string(Color::BLACK), "0,0,0,255");
    }

    #[test]
    fn single_symbol_fill_matches_golden_file() {
        let mut style = base_style();
        style.fill = Fill {
            color: Color::new(255, 0, 0, 128),
            hatch: Some(Hatch {
                angle: 45,
                distance_mm: 2.0,
                line_width_mm: 0.25,
                color: Color::BLACK,
            }),
            outline: Outline {
                color: Color::BLACK,
                width_mm: 0.26,
                style: "solid".into(),
            },
        };
        let xml = generate_qml(&style, GeometryKind::Polygon);
        let expected = read_golden("single_fill.qml");
        assert_eq!(xml, expected);
    }

    #[test]
    fn single_symbol_line_matches_golden_file() {
        let style = base_style();
        let xml = generate_qml(&style, GeometryKind::LineString);
        let expected = read_golden("single_line.qml");
        assert_eq!(xml, expected);
    }

    #[test]
    fn single_symbol_marker_matches_golden_file() {
        let style = base_style();
        let xml = generate_qml(&style, GeometryKind::Point);
        let expected = read_golden("single_marker.qml");
        assert_eq!(xml, expected);
    }

    #[test]
    fn categorized_fill_matches_golden_file() {
        let mut style = base_style();
        style.renderer = RendererKind::Categorized;
        style.build_categories("type", &["forest".into(), "water<1>".into()]);
        let xml = generate_qml(&style, GeometryKind::Polygon);
        let expected = read_golden("categorized_fill.qml");
        assert_eq!(xml, expected);
    }

    /// Regenerates the golden files: `cargo test -- --ignored regenerate_golden`
    /// Review the diff before committing.
    #[test]
    #[ignore]
    fn regenerate_golden() {
        let mut style = base_style();
        style.fill = Fill {
            color: Color::new(255, 0, 0, 128),
            hatch: Some(Hatch {
                angle: 45,
                distance_mm: 2.0,
                line_width_mm: 0.25,
                color: Color::BLACK,
            }),
            outline: Outline {
                color: Color::BLACK,
                width_mm: 0.26,
                style: "solid".into(),
            },
        };
        std::fs::create_dir_all("tests/golden").unwrap();
        std::fs::write(
            "tests/golden/single_fill.qml",
            generate_qml(&style, GeometryKind::Polygon),
        )
        .unwrap();
        std::fs::write(
            "tests/golden/single_line.qml",
            generate_qml(&StyleProject::default(), GeometryKind::LineString),
        )
        .unwrap();
        std::fs::write(
            "tests/golden/single_marker.qml",
            generate_qml(&StyleProject::default(), GeometryKind::Point),
        )
        .unwrap();
        let mut cat_style = base_style();
        cat_style.renderer = RendererKind::Categorized;
        cat_style.build_categories("type", &["forest".into(), "water<1>".into()]);
        std::fs::write(
            "tests/golden/categorized_fill.qml",
            generate_qml(&cat_style, GeometryKind::Polygon),
        )
        .unwrap();
    }

    #[test]
    fn xml_escapes_special_characters() {
        let mut style = base_style();
        style.renderer = RendererKind::Categorized;
        style.build_categories("name", &["a&b".into(), "c\"d".into()]);
        let xml = generate_qml(&style, GeometryKind::Point);
        assert!(xml.contains("value=\"a&amp;b\""));
        assert!(xml.contains("value=\"c&quot;d\""));
        assert!(!xml.contains("value=\"a&b\""));
    }

    #[test]
    fn attr_is_quoted_and_escaped() {
        let mut style = base_style();
        style.renderer = RendererKind::Categorized;
        style.build_categories("my attr<", &["x".into()]);
        let xml = generate_qml(&style, GeometryKind::Point);
        assert!(xml.contains("attr=\"&quot;my attr&lt;&quot;\""));
    }

    #[test]
    fn generated_xml_is_well_formed() {
        // Well-formedness check without an XML dependency: balanced tags by scan.
        for kind in [
            GeometryKind::Point,
            GeometryKind::LineString,
            GeometryKind::Polygon,
        ] {
            let mut style = base_style();
            let single = generate_qml(&style, kind);
            assert!(single.starts_with("<!DOCTYPE qgis"));
            assert!(single.ends_with("</qgis>\n"));
            style.renderer = RendererKind::Categorized;
            style.build_categories("k", &["a".into(), "b".into(), "c".into()]);
            let cat = generate_qml(&style, kind);
            assert!(cat.contains("type=\"categorizedSymbol\""));
            assert!(cat.matches("<symbol ").count() == 3);
            assert!(cat.matches("</symbol>").count() == 3);
        }
    }
}
