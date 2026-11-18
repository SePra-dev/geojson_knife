//! Pure coordinate transformation between WGS84 lon/lat (EPSG:4326) and
//! ETRS89 / UTM zone 33N (EPSG:25833), using the standard UTM series
//! (Snyder / Krüger, 4th–6th order). Accuracy: millimeter–centimeter level.
//!
//! Datum note: the ETRS89 ↔ WGS84 datum difference is below one meter across
//! Europe and is treated as zero, as is common practice. What this module
//! changes is purely the map projection (lon/lat ↔ UTM 33N, central meridian
//! 15°E, k0 = 0.9996, GRS80/WGS84 ellipsoid — identical to <0.1 mm rounding).
//!
//! No panics: out-of-range or malformed input returns `Err`.

use serde_json::Value;
use thiserror::Error;

pub const EPSG_WGS84: u32 = 4326;
pub const EPSG_ETRS89_UTM33N: u32 = 25833;

const ZONE_LON0: f64 = 15.0; // central meridian of UTM zone 33N
const FALSE_EASTING: f64 = 500_000.0;
const K0: f64 = 0.9996;
const A: f64 = 6_378_137.0; // GRS80 / WGS84 semi-major axis
const F: f64 = 1.0 / 298.257_222_101; // GRS80 flattening (≈ WGS84)
const E2: f64 = 2.0 * F - F * F; // first eccentricity squared
const EP2: f64 = E2 / (1.0 - E2); // second eccentricity squared

const MIN_LAT: f64 = -80.0;
const MAX_LAT: f64 = 84.0;

#[derive(Debug, Error, PartialEq)]
pub enum ProjError {
    #[error("latitude {0:.2} is outside the valid UTM range [{MIN_LAT}°, {MAX_LAT}°]")]
    LatitudeOutOfRange(f64),
    #[error("northing {0:.0} is outside the valid UTM range")]
    NorthingOutOfRange(f64),
    #[error("coordinate is not a finite number")]
    NonFinite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Crs {
    /// Geographic WGS84 longitude/latitude (x = lon, y = lat).
    Wgs84,
    /// ETRS89 UTM zone 33N (x = easting, y = northing). EPSG:25833.
    Etrs89Utm33n,
}

impl Crs {
    pub fn label(self) -> &'static str {
        match self {
            Crs::Wgs84 => "WGS84 (EPSG:4326, lon/lat)",
            Crs::Etrs89Utm33n => "ETRS89 UTM 33N (EPSG:25833)",
        }
    }

    pub fn epsg(self) -> u32 {
        match self {
            Crs::Wgs84 => EPSG_WGS84,
            Crs::Etrs89Utm33n => EPSG_ETRS89_UTM33N,
        }
    }
}

/// Project WGS84 lon/lat (degrees) to UTM 33N easting/northing (meters).
pub fn wgs84_to_utm33n(lon: f64, lat: f64) -> Result<(f64, f64), ProjError> {
    if !lon.is_finite() || !lat.is_finite() {
        return Err(ProjError::NonFinite);
    }
    if !(MIN_LAT..=MAX_LAT).contains(&lat) {
        return Err(ProjError::LatitudeOutOfRange(lat));
    }
    let phi = lat.to_radians();
    let dlam = (lon - ZONE_LON0).to_radians();
    let s = phi.sin();
    let c = phi.cos();
    let t = s / c;
    let tan2 = t * t;
    let c2 = EP2 * c * c;
    let n = A / (1.0 - E2 * s * s).sqrt();
    let aa = c * dlam;
    let aa2 = aa * aa;

    // Meridian arc M.
    let e4 = E2 * E2;
    let e6 = e4 * E2;
    let m = A
        * ((1.0 - E2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * phi
            - (3.0 * E2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * (2.0 * phi).sin()
            + (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * (4.0 * phi).sin()
            - (35.0 * e6 / 3072.0) * (6.0 * phi).sin());

    let easting = FALSE_EASTING
        + K0 * n * (aa
            + (1.0 - tan2 + c2) * aa2 * aa / 6.0
            + (5.0 - 18.0 * tan2 + tan2 * tan2 + 72.0 * c2 - 58.0 * EP2)
                * aa2 * aa2 * aa / 120.0);
    let northing = K0
        * (m
            + n * t * (aa2 / 2.0
                + (5.0 - tan2 + 9.0 * c2 + 4.0 * c2 * c2) * aa2 * aa2 / 24.0
                + (61.0 - 58.0 * tan2 + tan2 * tan2 + 600.0 * c2 - 330.0 * EP2)
                    * aa2 * aa2 * aa2 / 720.0));
    Ok((easting, northing))
}

/// Inverse-project UTM 33N easting/northing (meters) to WGS84 lon/lat (degrees).
pub fn utm33n_to_wgs84(easting: f64, northing: f64) -> Result<(f64, f64), ProjError> {
    if !easting.is_finite() || !northing.is_finite() {
        return Err(ProjError::NonFinite);
    }
    if !(0.0..=10_000_000.0).contains(&northing) {
        return Err(ProjError::NorthingOutOfRange(northing));
    }
    let e4 = E2 * E2;
    let e6 = e4 * E2;

    let m = northing / K0;
    let mu = m / (A * (1.0 - E2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0));
    let e1 = (1.0 - (1.0 - E2).sqrt()) / (1.0 + (1.0 - E2).sqrt());

    let mut phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (15.0 * e1.powi(2) / 16.0 - 255.0 * e1.powi(4) / 256.0) * (4.0 * mu).sin()
        + (35.0 * e1.powi(3) / 32.0) * (6.0 * mu).sin()
        + (315.0 * e1.powi(4) / 512.0) * (8.0 * mu).sin();

    // Fixed-point refinement of phi1 for sub-centimeter accuracy.
    for _ in 0..2 {
        let m2 = A
            * ((1.0 - E2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * phi1
                - (3.0 * E2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * (2.0 * phi1).sin()
                + (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * (4.0 * phi1).sin()
                - (35.0 * e6 / 3072.0) * (6.0 * phi1).sin());
        // dM/dphi = a (1 - e²) / (1 - e² sin²phi)^3/2
        let denom = (1.0 - E2 * phi1.sin().powi(2)).powf(1.5);
        let dmdphi = A * (1.0 - E2) / denom;
        phi1 += (m - m2) / dmdphi;
    }

    let s1 = phi1.sin();
    let c1 = phi1.cos();
    let t1 = s1 / c1;
    let c1p = EP2 * c1 * c1;
    let n1 = A / (1.0 - E2 * s1 * s1).sqrt();
    let r1 = A * (1.0 - E2) / (1.0 - E2 * s1 * s1).powf(1.5);
    let d = (easting - FALSE_EASTING) / (n1 * K0);
    let d2 = d * d;

    let phi = phi1 - (n1 * t1 / r1)
        * (d2 / 2.0
            - (5.0 + 3.0 * t1 * t1 + 10.0 * c1p - 4.0 * c1p * c1p - 9.0 * EP2)
                * d2 * d2 / 24.0
            + (61.0 + 90.0 * t1 * t1 + 298.0 * c1p + 45.0 * t1 * t1 * t1 * t1
                - 252.0 * EP2
                - 3.0 * c1p * c1p)
                * d2 * d2 * d2 / 720.0);
    let lam = ZONE_LON0.to_radians()
        + (d - (1.0 + 2.0 * t1 * t1 + c1p) * d * d2 / 6.0
            + (5.0 - 2.0 * c1p + 28.0 * t1 * t1 - 3.0 * c1p * c1p + 8.0 * EP2
                + 24.0 * t1 * t1 * t1 * t1)
                * d * d2 * d2 / 120.0)
            / c1;

    Ok((lam.to_degrees(), phi.to_degrees()))
}

/// Direction of a dataset-wide transformation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformDirection {
    Wgs84ToUtm33n,
    Utm33nToWgs84,
}

/// Transform a raw GeoJSON geometry `Value` (any nesting depth, any geometry
/// type) by applying `project` to every coordinate position `[x, y(, z)]`.
/// Positions longer than two elements keep their remaining elements (e.g. z),
/// which are passed through unchanged. Returns a rebuilt `Value`.
pub fn transform_geometry(v: &Value, project: &dyn Fn(f64, f64) -> Result<(f64, f64), ProjError>) -> Result<Value, ProjError> {
    match v {
        Value::Object(obj) => {
            let mut out = serde_json::Map::new();
            for (k, val) in obj {
                out.insert(k.clone(), transform_geometry(val, project)?);
            }
            Ok(Value::Object(out))
        }
        Value::Array(arr) => {
            if !arr.is_empty() && arr.iter().all(|x| x.is_number()) && arr.len() >= 2 {
                let x = arr[0].as_f64().expect("checked is_number");
                let y = arr[1].as_f64().expect("checked is_number");
                let (tx, ty) = project(x, y)?;
                let mut out = vec![Value::from(tx), Value::from(ty)];
                for extra in arr.iter().skip(2) {
                    out.push(extra.clone());
                }
                Ok(Value::Array(out))
            } else {
                let mut out = Vec::with_capacity(arr.len());
                for item in arr {
                    out.push(transform_geometry(item, project)?);
                }
                Ok(Value::Array(out))
            }
        }
        other => Ok(other.clone()),
    }
}

/// Zone-33 validity check for a UI hint (EPSG:25833 covers lon 12°E–18°E;
/// points outside still project mathematically but distortion grows).
pub fn lon_in_zone33(lon: f64) -> bool {
    (12.0..=18.0).contains(&lon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn central_meridian_anchor() {
        // On the central meridian the easting is exactly the false easting.
        for lat in [0.0, 10.0, 48.0, 52.52] {
            let (e, _) = wgs84_to_utm33n(15.0, lat).unwrap();
            assert!((e - 500_000.0).abs() < 1e-6, "easting {e}");
        }
    }

    #[test]
    fn equator_anchors() {
        // On the equator the northing is exactly zero.
        for lon in [12.0, 15.0, 17.0] {
            let (_, n) = wgs84_to_utm33n(lon, 0.0).unwrap();
            assert!(n.abs() < 1e-6, "northing {n}");
        }
        // Easting scales with longitude offset: 3° at equator ≈ 334 km - but check
        // symmetric points around the central meridian mirror each other.
        let (e_minus, _) = wgs84_to_utm33n(13.5, 0.0).unwrap();
        let (e_plus, _) = wgs84_to_utm33n(16.5, 0.0).unwrap();
        assert!((e_minus - 2.0 * FALSE_EASTING + e_plus).abs() < 1e-6);
    }

    #[test]
    fn round_trip_is_accurate() {
        // Deterministic grid of points across Germany-ish area + edges of zone.
        for lon in [12.0, 13.4, 15.0, 16.75, 18.0] {
            for lat in [47.0, 50.0, 52.5, 55.0] {
                let (e, n) = wgs84_to_utm33n(lon, lat).unwrap();
                let (lon2, lat2) = utm33n_to_wgs84(e, n).unwrap();
                assert!(
                    (lon - lon2).abs() < 1e-7 && (lat - lat2).abs() < 1e-7,
                    "lon {lon}->{lon2}, lat {lat}->{lat2}"
                );
            }
        }
    }

    #[test]
    fn known_reference_point() {
        // UTM 33N of (15°E, 50°N): northing = meridian arc length to 50°N on GRS80,
        // independently computed with the meridian arc series = 5,538,630.7 m.
        let (_, n) = wgs84_to_utm33n(15.0, 50.0).unwrap();
        let expected = 5_538_630.70;
        assert!((n - expected).abs() < 0.5, "northing {n} vs {expected}");
    }

    #[test]
    fn latitude_out_of_range_is_an_error() {
        let err = wgs84_to_utm33n(15.0, 85.0).unwrap_err();
        assert!(matches!(err, ProjError::LatitudeOutOfRange(85.0)));
        let err = wgs84_to_utm33n(15.0, -81.0).unwrap_err();
        assert!(matches!(err, ProjError::LatitudeOutOfRange(-81.0)));
    }

    #[test]
    fn northing_out_of_range_is_an_error() {
        let err = utm33n_to_wgs84(500_000.0, -1.0).unwrap_err();
        assert!(matches!(err, ProjError::NorthingOutOfRange(-1.0)));
        assert!(utm33n_to_wgs84(500_000.0, 11_000_000.0).is_err());
    }

    #[test]
    fn non_finite_is_an_error() {
        assert_eq!(wgs84_to_utm33n(f64::NAN, 0.0).unwrap_err(), ProjError::NonFinite);
        assert_eq!(utm33n_to_wgs84(f64::INFINITY, 0.0).unwrap_err(), ProjError::NonFinite);
    }

    #[test]
    fn geometry_transformation_preserves_structure() {
        let geom: Value = serde_json::json!({
            "type": "Polygon",
            "coordinates": [[[13.5, 52.0, 100.0], [13.6, 52.1], [13.5, 52.2]], [[13.51, 52.01], [13.52, 52.02]]]
        });
        let out = transform_geometry(&geom, &wgs84_to_utm33n).unwrap();
        let f = &out["coordinates"][0][0];
        assert!(f[0].as_f64().unwrap() > 100_000.0, "easting {f}");
        assert!(f[1].as_f64().unwrap() > 5_000_000.0, "northing {f}");
        assert_eq!(f[2], 100.0, "z passes through unchanged");
        assert_eq!(out["type"], "Polygon");
        // ring structure preserved: 3 positions in ring 0, 2 in ring 1
        assert_eq!(out["coordinates"][0].as_array().unwrap().len(), 3);
        assert_eq!(out["coordinates"][1].as_array().unwrap().len(), 2);
    }

    #[test]
    fn geometry_transformation_round_trips() {
        let geom: Value = serde_json::json!({
            "type": "MultiPolygon",
            "coordinates": [[[[13.5, 52.0], [13.6, 52.1], [13.7, 52.0], [13.5, 52.0]]]]
        });
        let fwd = transform_geometry(&geom, &wgs84_to_utm33n).unwrap();
        // sort keys or compare positions numerically
        let (e, n) = wgs84_to_utm33n(13.5, 52.0).unwrap();
        assert_eq!(fwd["coordinates"][0][0][0][0].as_f64().unwrap(), e);
        assert_eq!(fwd["coordinates"][0][0][0][1].as_f64().unwrap(), n);
        let back = transform_geometry(&fwd, &utm33n_to_wgs84).unwrap();
        if let (Value::Array(a), Value::Array(b)) =
            (&geom["coordinates"][0][0], &back["coordinates"][0][0])
        {
            for (p, q) in a.iter().zip(b.iter()) {
                assert!((p[0].as_f64().unwrap() - q[0].as_f64().unwrap()).abs() < 1e-7);
                assert!((p[1].as_f64().unwrap() - q[1].as_f64().unwrap()).abs() < 1e-7);
            }
        } else {
            panic!("structure changed");
        }
    }

    #[test]
    fn non_position_arrays_are_recursed_not_projected() {
        // An array of numbers of length >= 2 nested as ring data is a position;
        // arrays containing arrays recurse. Check a GeometryCollection:
        let geom: Value = serde_json::json!({
            "type": "GeometryCollection",
            "geometries": [
                {"type": "Point", "coordinates": [15.0, 0.0]}
            ]
        });
        let out = transform_geometry(&geom, &wgs84_to_utm33n).unwrap();
        let coord = &out["geometries"][0]["coordinates"];
        assert!((coord[0].as_f64().unwrap() - 500_000.0).abs() < 1e-6);
        assert!(coord[1].as_f64().unwrap().abs() < 1e-6);
    }

    #[test]
    fn error_propagates_from_transform_geometry() {
        // Something with latitude > 84 must fail — never produce silent garbage.
        let geom: Value = serde_json::json!({"type": "Point", "coordinates": [15.0, 86.0]});
        let err = transform_geometry(&geom, &wgs84_to_utm33n).unwrap_err();
        assert!(matches!(err, ProjError::LatitudeOutOfRange(86.0)));
    }

    #[test]
    fn zone_check_helper() {
        assert!(lon_in_zone33(12.0));
        assert!(lon_in_zone33(18.0));
        assert!(lon_in_zone33(15.0));
        assert!(!lon_in_zone33(11.9));
        assert!(!lon_in_zone33(18.1));
    }
}
