//! Pure GeoJSON (RFC 7946) parsing and inspection. No UI dependencies.
//!
//! Supported root objects: `FeatureCollection` and single `Feature`.
//! Supported geometry types: Point, MultiPoint, LineString, MultiLineString,
//! Polygon, MultiPolygon, GeometryCollection.

use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashSet};
use thiserror::Error;

/// Maximum number of distinct values kept per attribute for preview / categorization.
pub const MAX_DISTINCT_VALUES: usize = 1000;

#[derive(Debug, Error)]
pub enum GeoJsonError {
    #[error("not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported or missing GeoJSON \"type\" (expected FeatureCollection or Feature)")]
    UnsupportedType,
    #[error("GeoJSON object is missing required field '{0}'")]
    MissingField(&'static str),
    #[error("feature #{0} has no \"geometry\" member")]
    FeatureWithoutGeometry(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GeometryKind {
    Point,
    MultiPoint,
    LineString,
    MultiLineString,
    Polygon,
    MultiPolygon,
    GeometryCollection,
}

impl GeometryKind {
    pub fn label(self) -> &'static str {
        match self {
            GeometryKind::Point => "Point",
            GeometryKind::MultiPoint => "MultiPoint",
            GeometryKind::LineString => "LineString",
            GeometryKind::MultiLineString => "MultiLineString",
            GeometryKind::Polygon => "Polygon",
            GeometryKind::MultiPolygon => "MultiPolygon",
            GeometryKind::GeometryCollection => "GeometryCollection",
        }
    }

    pub fn from_str(s: &str) -> Option<GeometryKind> {
        match s {
            "Point" => Some(GeometryKind::Point),
            "MultiPoint" => Some(GeometryKind::MultiPoint),
            "LineString" => Some(GeometryKind::LineString),
            "MultiLineString" => Some(GeometryKind::MultiLineString),
            "Polygon" => Some(GeometryKind::Polygon),
            "MultiPolygon" => Some(GeometryKind::MultiPolygon),
            "GeometryCollection" => Some(GeometryKind::GeometryCollection),
            _ => None,
        }
    }

    /// Symbol family used by QGIS for this geometry kind.
    pub fn symbol_family(self) -> &'static str {
        match self {
            GeometryKind::Point | GeometryKind::MultiPoint => "marker",
            GeometryKind::LineString
            | GeometryKind::MultiLineString
            | GeometryKind::GeometryCollection => "line",
            GeometryKind::Polygon | GeometryKind::MultiPolygon => "fill",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrType {
    Text,
    Number,
    Bool,
    Mixed,
    Null,
}

impl AttrType {
    pub fn label(self) -> &'static str {
        match self {
            AttrType::Text => "text",
            AttrType::Number => "number",
            AttrType::Bool => "bool",
            AttrType::Mixed => "mixed",
            AttrType::Null => "null",
        }
    }
}

/// A single feature: geometry kind plus its properties in original file order.
#[derive(Debug, Clone)]
pub struct Feature {
    pub kind: GeometryKind,
    /// Properties in the order they appear in the file (first feature defines field order).
    pub properties: Vec<(String, Value)>,
}

impl Feature {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.properties
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
}

/// Per-attribute summary computed over the whole dataset.
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub ty: AttrType,
    pub distinct: Vec<String>,
    pub distinct_count: usize,
    pub null_count: usize,
    pub num_min: Option<f64>,
    pub num_max: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Dataset {
    pub features: Vec<Feature>,
    /// Field list in the order found in the first feature that has properties.
    pub fields: Vec<FieldInfo>,
    /// All geometry kinds present (sorted).
    pub geometry_kinds: Vec<GeometryKind>,
}

impl Dataset {
    fn collect_kinds(features: &[Feature]) -> Vec<GeometryKind> {
        let mut set: BTreeSet<GeometryKind> = BTreeSet::new();
        for f in features {
            set.insert(f.kind);
        }
        set.into_iter().collect()
    }

    fn analyze_fields(features: &[Feature]) -> Vec<FieldInfo> {
        // Field order from the first feature that carries properties.
        let mut order: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for f in features {
            for (k, _) in &f.properties {
                if seen.insert(k.clone()) {
                    order.push(k.clone());
                }
            }
            if !order.is_empty() {
                break;
            }
        }

        order
            .into_iter()
            .map(|name| {
                let mut ty: Option<AttrType> = None;
                let mut distinct_set: BTreeSet<String> = BTreeSet::new();
                let mut distinct: Vec<String> = Vec::new();
                let mut distinct_count = 0usize;
                let mut null_count = 0usize;
                let mut num_min = f64::INFINITY;
                let mut num_max = f64::NEG_INFINITY;

                for f in features {
                    let v = match f.get(&name) {
                        None => {
                            null_count += 1;
                            continue;
                        }
                        Some(v) => v,
                    };
                    if v.is_null() {
                        null_count += 1;
                        continue;
                    }
                    let this_ty = value_type(v);
                    ty = Some(match (ty, this_ty) {
                        (Some(a), b) if a == b => a,
                        (Some(_), _) => AttrType::Mixed,
                        (None, b) => b,
                    });
                    if let Value::Number(n) = v {
                        if let Some(fv) = n.as_f64() {
                            num_min = num_min.min(fv);
                            num_max = num_max.max(fv);
                        }
                    }
                    let key = value_to_string(v);
                    if distinct_set.insert(key) {
                        distinct_count += 1;
                        if distinct.len() < MAX_DISTINCT_VALUES {
                            distinct.push(value_to_string(v));
                        }
                    }
                }

                FieldInfo {
                    name,
                    ty: ty.unwrap_or(AttrType::Null),
                    distinct,
                    distinct_count,
                    null_count,
                    num_min: if num_min.is_finite() { Some(num_min) } else { None },
                    num_max: if num_max.is_finite() { Some(num_max) } else { None },
                }
            })
            .collect()
    }
}

fn value_type(v: &Value) -> AttrType {
    match v {
        Value::String(_) => AttrType::Text,
        Value::Number(_) => AttrType::Number,
        Value::Bool(_) => AttrType::Bool,
        _ => AttrType::Mixed,
    }
}

/// String form of a property value used for distinct-value previews and category values.
pub fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn parse_geometry_type(v: &Value) -> Result<String, GeoJsonError> {
    let obj = v
        .as_object()
        .ok_or(GeoJsonError::UnsupportedType)?;
    obj.get("type")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or(GeoJsonError::MissingField("type"))
}

fn properties_to_vec(props: Option<&Value>) -> Vec<(String, Value)> {
    match props.and_then(|p| p.as_object()) {
        Some(Map { .. }) => {
            // serde_json is compiled with `preserve_order`, so iteration order == file order.
            props
                .unwrap()
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        }
        _ => Vec::new(),
    }
}

fn feature_from_value(v: &Value, index: usize) -> Result<Feature, GeoJsonError> {
    let obj = v
        .as_object()
        .ok_or(GeoJsonError::UnsupportedType)?;
    let geom = obj
        .get("geometry")
        .ok_or(GeoJsonError::FeatureWithoutGeometry(index))?;
    let kind = if geom.is_null() {
        GeometryKind::GeometryCollection // treat null geometry as inert; kind unused for rendering
    } else {
        let ty = parse_geometry_type(geom)?;
        GeometryKind::from_str(&ty).ok_or(GeoJsonError::UnsupportedType)?
    };
    Ok(Feature {
        kind,
        properties: properties_to_vec(obj.get("properties")),
    })
}

/// Parse a GeoJSON string into a [`Dataset`].
///
/// Never panics on malformed input; all error cases are returned as `Err`.
pub fn parse_geojson(text: &str) -> Result<Dataset, GeoJsonError> {
    let root: Value = serde_json::from_str(text)?;
    let ty = parse_geometry_type(&root)?;

    let features: Vec<Feature> = match ty.as_str() {
        "FeatureCollection" => {
            let arr = root
                .get("features")
                .ok_or(GeoJsonError::MissingField("features"))?
                .as_array()
                .ok_or(GeoJsonError::MissingField("features"))?;
            let mut out = Vec::with_capacity(arr.len());
            for (i, f) in arr.iter().enumerate() {
                out.push(feature_from_value(f, i)?);
            }
            out
        }
        "Feature" => vec![feature_from_value(&root, 0)?],
        _ => return Err(GeoJsonError::UnsupportedType),
    };

    Ok(Dataset {
        geometry_kinds: Dataset::collect_kinds(&features),
        fields: Dataset::analyze_fields(&features),
        features,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_feature_collection_with_field_order() {
        let data = r#"{
            "type": "FeatureCollection",
            "features": [
                {"type":"Feature","geometry":{"type":"Point","coordinates":[1,2]},
                 "properties":{"zeta":1,"alpha":2}},
                {"type":"Feature","geometry":{"type":"Point","coordinates":[3,4]},
                 "properties":{"zeta":3,"alpha":4}}
            ]
        }"#;
        let ds = parse_geojson(data).unwrap();
        assert_eq!(ds.features.len(), 2);
        assert_eq!(ds.geometry_kinds, vec![GeometryKind::Point]);
        // field order preserved from first feature
        assert_eq!(ds.fields[0].name, "zeta");
        assert_eq!(ds.fields[1].name, "alpha");
        assert_eq!(ds.fields[0].ty, AttrType::Number);
        assert_eq!(ds.fields[0].num_min, Some(1.0));
        assert_eq!(ds.fields[0].num_max, Some(3.0));
    }

    #[test]
    fn parses_single_feature() {
        let data = r#"{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,0]]]},"properties":{"name":"a"}}"#;
        let ds = parse_geojson(data).unwrap();
        assert_eq!(ds.features.len(), 1);
        assert_eq!(ds.geometry_kinds, vec![GeometryKind::Polygon]);
        assert_eq!(ds.fields[0].name, "name");
        assert_eq!(ds.fields[0].ty, AttrType::Text);
    }

    #[test]
    fn all_geometry_kinds_supported() {
        for ty in [
            "Point",
            "MultiPoint",
            "LineString",
            "MultiLineString",
            "Polygon",
            "MultiPolygon",
            "GeometryCollection",
        ] {
            let data = format!(
                r#"{{"type":"Feature","geometry":{{"type":"{ty}","coordinates":[]}},"properties":{{}}}}"#
            );
            let ds = parse_geojson(&data)
                .unwrap_or_else(|e| panic!("{ty} should parse, got {e}"));
            assert_eq!(ds.geometry_kinds.len(), 1);
        }
    }

    #[test]
    fn mixed_collection_reports_all_kinds() {
        let data = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","geometry":{"type":"Point","coordinates":[0,0]},"properties":{}},
            {"type":"Feature","geometry":{"type":"LineString","coordinates":[[0,0],[1,1]]},"properties":{}}
        ]}"#;
        let ds = parse_geojson(data).unwrap();
        assert_eq!(
            ds.geometry_kinds,
            vec![GeometryKind::Point, GeometryKind::LineString]
        );
    }

    #[test]
    fn malformed_json_is_a_friendly_error() {
        let err = parse_geojson("{ not json").unwrap_err();
        assert!(err.to_string().contains("not valid JSON"), "{err}");
    }

    #[test]
    fn unsupported_root_type_is_an_error() {
        let err = parse_geojson(r#"{"type":"Geometry","coordinates":[0,0]}"#).unwrap_err();
        assert!(matches!(err, GeoJsonError::UnsupportedType));
    }

    #[test]
    fn missing_features_field_is_an_error() {
        let err = parse_geojson(r#"{"type":"FeatureCollection"}"#).unwrap_err();
        assert!(matches!(err, GeoJsonError::MissingField("features")));
    }

    #[test]
    fn null_and_nested_values_counted_correctly() {
        let data = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","geometry":{"type":"Point","coordinates":[0,0]},
             "properties":{"a":null,"b":{"nested":true},"c":"x"}},
            {"type":"Feature","geometry":{"type":"Point","coordinates":[1,0]},
             "properties":{"a":1,"b":[1,2],"c":"x"}},
            {"type":"Feature","geometry":{"type":"Point","coordinates":[2,0]},
             "properties":{"a":2,"b":true,"c":"y"}}
        ]}"#;
        let ds = parse_geojson(data).unwrap();
        let a = &ds.fields[0];
        assert_eq!(a.null_count, 1);
        assert_eq!(a.ty, AttrType::Number); // nulls don't count towards type
        let b = &ds.fields[1];
        assert_eq!(b.ty, AttrType::Mixed);
        assert_eq!(b.distinct_count, 3);
        let c = &ds.fields[2];
        assert_eq!(c.distinct_count, 2);
    }

    #[test]
    fn null_geometry_feature_does_not_panic() {
        let data = r#"{"type":"Feature","geometry":null,"properties":{"a":1}}"#;
        let ds = parse_geojson(data).unwrap();
        assert_eq!(ds.features.len(), 1);
    }
}
