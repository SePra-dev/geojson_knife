//! Phase 2 data-table model: column renaming/reordering/deletion (with undo),
//! per-column AND-combined filters, and RFC 7946 GeoJSON export that preserves
//! geometries exactly and applies the new property order/renames.
//! Pure logic, no UI dependencies.

use super::geo::{value_to_string, Feature};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A column of the table, tracking its original property key, the (possibly
/// renamed) display key, and whether it was deleted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnState {
    /// Original property key in the source file.
    pub original: String,
    /// Current (possibly renamed) key; written back on export.
    pub display: String,
    pub deleted: bool,
}

/// Per-column filter. All active filters combine with AND logic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterKind {
    /// Case-insensitive substring match on the value's string form.
    Contains(String),
    /// Exact match on the value's string form.
    Equals(String),
    /// Numeric range (inclusive); non-numeric values do not match.
    Range { min: f64, max: f64 },
}

impl FilterKind {
    pub fn label(&self) -> String {
        match self {
            FilterKind::Contains(s) => format!("contains \"{s}\""),
            FilterKind::Equals(s) => format!("= \"{s}\""),
            FilterKind::Range { min, max } => format!("{min} … {max}"),
        }
    }

    pub fn matches(&self, value: &Value) -> bool {
        match self {
            FilterKind::Contains(s) => {
                value_to_string(value).to_lowercase().contains(&s.to_lowercase())
            }
            FilterKind::Equals(s) => &value_to_string(value) == s,
            FilterKind::Range { min, max } => value
                .as_f64()
                .map(|n| n >= *min && n <= *max)
                .unwrap_or(false),
        }
    }
}

/// Complete table editing state for the session.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TableState {
    pub columns: Vec<ColumnState>,
    /// Snapshots for undo (session-wide, delete/rename/reorder all undoable).
    pub undo_stack: Vec<Vec<ColumnState>>,
    /// Filters keyed by the column's *original* name.
    pub filters: BTreeMap<String, FilterKind>,
}

impl TableState {
    /// Initialize from a dataset's field list (original order).
    pub fn from_fields(fields: &[super::geo::FieldInfo]) -> TableState {
        TableState {
            columns: fields
                .iter()
                .map(|f| ColumnState {
                    original: f.name.clone(),
                    display: f.name.clone(),
                    deleted: false,
                })
                .collect(),
            undo_stack: Vec::new(),
            filters: BTreeMap::new(),
        }
    }

    fn snapshot(&mut self) {
        self.undo_stack.push(self.columns.clone());
    }

    /// Undo the last structural edit. Returns true if anything was undone.
    pub fn undo(&mut self) -> bool {
        match self.undo_stack.pop() {
            Some(cols) => {
                self.columns = cols;
                true
            }
            None => false,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Rename a column (by original name) to `new_name`.
    pub fn rename_column(&mut self, original: &str, new_name: &str) -> bool {
        if new_name.is_empty() {
            return false;
        }
        let idx = match self.columns.iter().position(|c| c.original == original) {
            Some(i) => i,
            None => return false,
        };
        if self.columns[idx].display == new_name {
            return false;
        }
        self.snapshot();
        self.columns[idx].display = new_name.to_string();
        true
    }

    /// Soft-delete a column (by original name). Undoable.
    pub fn delete_column(&mut self, original: &str) -> bool {
        let idx = match self.columns.iter().position(|c| c.original == original) {
            Some(i) => i,
            None => return false,
        };
        if self.columns[idx].deleted {
            return false;
        }
        self.snapshot();
        self.columns[idx].deleted = true;
        true
    }

    /// Move a column from index `from` to index `to` (both into the full column
    /// list, including deleted columns). Undoable.
    pub fn move_column(&mut self, from: usize, to: usize) -> bool {
        if from >= self.columns.len() || to >= self.columns.len() || from == to {
            return false;
        }
        self.snapshot();
        let col = self.columns.remove(from);
        self.columns.insert(to, col);
        true
    }

    /// Active (non-deleted) columns in their current order.
    pub fn active_columns(&self) -> Vec<&ColumnState> {
        self.columns.iter().filter(|c| !c.deleted).collect()
    }

    /// Map an index into the active (non-deleted) column list to an index into
    /// the full column list (including deleted columns).
    pub fn full_index(&self, visible_idx: usize) -> usize {
        let mut n = 0;
        for (i, c) in self.columns.iter().enumerate() {
            if !c.deleted {
                if n == visible_idx {
                    return i;
                }
                n += 1;
            }
        }
        self.columns.len().saturating_sub(1)
    }

    /// Does a feature pass all active filters?
    pub fn row_matches(&self, feature: &Feature) -> bool {
        self.filters.iter().all(|(original, filter)| {
            // Filters on deleted columns are ignored.
            let active = self
                .columns
                .iter()
                .any(|c| !c.deleted && c.original == *original);
            if !active {
                return true;
            }
            match feature.get(original) {
                Some(v) if !v.is_null() => filter.matches(v),
                _ => false,
            }
        })
    }

    /// Indices of the features that pass all filters.
    pub fn visible_row_indices(&self, features: &[Feature]) -> Vec<usize> {
        features
            .iter()
            .enumerate()
            .filter(|(_, f)| self.row_matches(f))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn active_filter_count(&self) -> usize {
        self.filters.len()
    }
}

/// Export the (possibly edited and filtered) dataset as an RFC 7946 GeoJSON
/// FeatureCollection string (UTF-8, pretty-printed).
///
/// Geometries are preserved exactly; properties use the current column order
/// and renamed keys. If `respect_filters` is true, only features passing all
/// filters are exported.
pub fn export_geojson(features: &[Feature], state: &TableState, respect_filters: bool) -> String {
    let indices: Vec<usize> = if respect_filters {
        state.visible_row_indices(features)
    } else {
        (0..features.len()).collect()
    };

    let columns = state.active_columns();
    let out_features: Vec<Value> = indices
        .iter()
        .map(|&i| {
            let f = &features[i];
            let mut props = Map::new();
            for col in &columns {
                let v = f.get(&col.original).cloned().unwrap_or(Value::Null);
                props.insert(col.display.clone(), v);
            }
            let mut obj = Map::new();
            obj.insert("type".into(), Value::String("Feature".into()));
            obj.insert("geometry".into(), f.geometry.clone());
            obj.insert("properties".into(), Value::Object(props));
            Value::Object(obj)
        })
        .collect();

    let mut root = Map::new();
    root.insert("type".into(), Value::String("FeatureCollection".into()));
    root.insert(
        "features".into(),
        Value::Array(out_features),
    );
    serde_json::to_string_pretty(&Value::Object(root)).expect("GeoJSON serialization cannot fail")
}

#[cfg(test)]
mod tests {
    use super::super::geo::parse_geojson;
    use super::*;

    fn sample() -> Vec<Feature> {
        let data = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","geometry":{"type":"Point","coordinates":[1.25,2.5]},
             "properties":{"name":"Alpha","kind":"a","size":10}},
            {"type":"Feature","geometry":{"type":"Point","coordinates":[3.5,4.25]},
             "properties":{"name":"Beta","kind":"b","size":50}},
            {"type":"Feature","geometry":{"type":"Point","coordinates":[5.0,6.0]},
             "properties":{"name":"Gamma","kind":"a","size":90}}
        ]}"#;
        parse_geojson(data).unwrap().features
    }

    fn state() -> TableState {
        TableState {
            columns: vec![
                ColumnState { original: "name".into(), display: "name".into(), deleted: false },
                ColumnState { original: "kind".into(), display: "kind".into(), deleted: false },
                ColumnState { original: "size".into(), display: "size".into(), deleted: false },
            ],
            undo_stack: vec![],
            filters: BTreeMap::new(),
        }
    }

    #[test]
    fn rename_writes_back_on_export() {
        let mut st = state();
        assert!(st.rename_column("name", "label"));
        let json = export_geojson(&sample(), &st, false);
        assert!(json.contains("\"label\""));
        assert!(!json.contains("\"name\""));
        let parsed: Value = serde_json::from_str(&json).unwrap();
        let props = &parsed["features"][0]["properties"];
        let keys: Vec<&str> = props.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, ["label", "kind", "size"]);
    }

    #[test]
    fn rename_rejects_empty_and_missing() {
        let mut st = state();
        assert!(!st.rename_column("name", ""));
        assert!(!st.rename_column("nope", "x"));
        assert!(!st.rename_column("name", "name")); // no-op
    }

    #[test]
    fn delete_and_undo() {
        let mut st = state();
        assert!(st.delete_column("kind"));
        assert_eq!(st.active_columns().len(), 2);
        assert!(st.can_undo());
        assert!(st.undo());
        assert_eq!(st.active_columns().len(), 3);
        assert!(!st.undo()); // stack empty
    }

    #[test]
    fn delete_removes_property_on_export() {
        let mut st = state();
        st.delete_column("kind");
        let json = export_geojson(&sample(), &st, false);
        assert!(!json.contains("\"kind\""));
    }

    #[test]
    fn move_column_reorders_properties_on_export() {
        let mut st = state();
        assert!(st.move_column(2, 0)); // size first
        let json = export_geojson(&sample(), &st, false);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        let keys: Vec<&str> = parsed["features"][0]["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        assert_eq!(keys, ["size", "name", "kind"]);
    }

    #[test]
    fn move_column_rejects_out_of_range() {
        let mut st = state();
        assert!(!st.move_column(0, 3));
        assert!(!st.move_column(3, 0));
        assert!(!st.move_column(1, 1));
    }

    #[test]
    fn filters_combine_with_and() {
        let mut st = state();
        st.filters.insert("kind".into(), FilterKind::Equals("a".into()));
        st.filters.insert("size".into(), FilterKind::Range { min: 50.0, max: 100.0 });
        let feats = sample();
        // kind=a: Alpha(10), Gamma(90); size 50..100: Beta(50), Gamma(90) → AND: Gamma
        let idx = st.visible_row_indices(&feats);
        assert_eq!(idx, vec![2]);
    }

    #[test]
    fn contains_filter_is_case_insensitive() {
        let mut st = state();
        st.filters.insert("name".into(), FilterKind::Contains("ALP".into()));
        let idx = st.visible_row_indices(&sample());
        assert_eq!(idx, vec![0]);
    }

    #[test]
    fn range_filter_excludes_non_numeric() {
        let mut st = state();
        st.filters.insert("name".into(), FilterKind::Range { min: 0.0, max: 10.0 });
        assert!(st.visible_row_indices(&sample()).is_empty());
    }

    #[test]
    fn filter_on_deleted_column_is_ignored() {
        let mut st = state();
        st.delete_column("kind");
        st.filters.insert("kind".into(), FilterKind::Equals("zzz".into()));
        assert_eq!(st.visible_row_indices(&sample()).len(), 3);
    }

    #[test]
    fn export_respects_or_ignores_filters() {
        let mut st = state();
        st.filters.insert("kind".into(), FilterKind::Equals("b".into()));
        let feats = sample();
        let filtered = export_geojson(&feats, &st, true);
        let all = export_geojson(&feats, &st, false);
        let f: Value = serde_json::from_str(&filtered).unwrap();
        let a: Value = serde_json::from_str(&all).unwrap();
        assert_eq!(f["features"].as_array().unwrap().len(), 1);
        assert_eq!(a["features"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn export_preserves_geometry_exactly() {
        let st = state();
        let feats = sample();
        let json = export_geojson(&feats, &st, false);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["features"][1]["geometry"]["coordinates"][0], 3.5);
        assert_eq!(parsed["features"][1]["geometry"]["coordinates"][1], 4.25);
        assert_eq!(parsed["type"], "FeatureCollection");
    }

    #[test]
    fn export_is_valid_utf8_rfc7946_shape() {
        let mut st = state();
        st.rename_column("name", "näme ✓");
        let json = export_geojson(&sample(), &st, false);
        assert!(json.contains("näme ✓"));
        assert!(json.contains("\"type\": \"FeatureCollection\""));
        assert!(json.contains("\"type\": \"Feature\""));
    }

    #[test]
    fn from_fields_initializes_original_order() {
        let ds = parse_geojson(
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[0,0]},"properties":{"b":1,"a":2}}]}"#,
        )
        .unwrap();
        let st = TableState::from_fields(&ds.fields);
        let names: Vec<&str> = st.columns.iter().map(|c| c.original.as_str()).collect();
        assert_eq!(names, ["b", "a"]);
        assert!(st.active_filter_count() == 0);
    }

    #[test]
    fn full_index_maps_visible_to_full_positions() {
        let mut st = state();
        st.delete_column("kind"); // "kind" is index 1 in the full list
        assert_eq!(st.full_index(0), 0); // "name"
        assert_eq!(st.full_index(1), 2); // "size"
    }

    #[test]
    fn undo_restores_after_multiple_edits() {
        let mut st = state();
        st.rename_column("name", "x");
        st.delete_column("kind");
        st.move_column(0, 1);
        assert_eq!(st.undo_stack.len(), 3);
        st.undo();
        st.undo();
        st.undo();
        assert_eq!(st, state());
    }
}
