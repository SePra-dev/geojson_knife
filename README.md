# GeoJSON Toolkit

A portable, single-binary **Swiss Army knife for GeoJSON files** — a Windows desktop
application (Rust + egui/eframe) for interactive map-symbol styling and exporting
QGIS style files (`.qml`).

## What it does (Phase 1 — MVP)

- **GeoJSON import & inspection**: open `FeatureCollection` / single `Feature` files
  (all RFC 7946 geometry types) via the file dialog or by drag & drop. Parsing runs
  on a background thread, so even 50k+ feature files never freeze the UI.
  The left panel reports feature count, geometry types present, and per-attribute
  detected type, distinct-value preview (or numeric min/max), and null count —
  in original field order.
- **Style designer**: per geometry family (fill / line / marker) design symbols with
  egui's built-in HSV color pickers:
  - **Fill**: flat color + opacity slider, optional hatching (angle 0–359°, spacing in
    mm, line width in mm, hatch line color), and outline color/width — with a live
    vector preview on an egui canvas.
  - **Line**: color and width in mm.
  - **Point**: marker size, color, outline.
- **Categorized rendering**: pick an attribute, build categories from its unique
  values, get an auto-assigned pleasant color ramp (golden-angle hue rotation),
  override any category by hand, and use bulk actions ("Apply hatch to all",
  "Apply outline to all").
- **Style projects**: every style state round-trips — save/load style as a small
  JSON project file.
- **QML export**: export the designed style as a QGIS 3.x `.qml` style file
  (single-symbol or categorized renderer; `SimpleFill`, `LinePatternFill`,
  `SimpleLine`, `SimpleMarker` symbol layers). Load it in QGIS via
  *Layer Properties → Style → Load Style…*.

## Build

Requires the Rust stable toolchain (MSVC on Windows).

```bash
cargo build --release
```

On Windows (`x86_64-pc-windows-msvc`) this produces the single self-contained
`target/release/geojson_knife.exe` (icon embedded via `winres`). No installation,
no admin rights, no external DLLs, no registry writes — all settings live in a
`config/` folder created next to the executable.

The release EXE is roughly **5 MB** (built with LTO and symbol stripping; verified
on the Windows CI runner — exact size depends on toolchain version).

## Usage

1. Start `geojson_knife.exe`.
2. Open a `.geojson` file (button or drag & drop) — try the files in `samples/`
   (`polygons.geojson`, `lines.geojson`, `points.geojson`, `mixed.geojson`,
   `malformed.geojson`, `large_points.geojson` with 60,000 features).
3. Design your symbol; for categorized rendering pick an attribute and click
   *Build categories*.
4. Click **Export .qml…** — the default filename is derived from the GeoJSON file.
5. In QGIS: open the GeoJSON as a vector layer, then
   *Layer Properties → Symbology → Style → Load Style…* and select the exported
   `.qml`.

- **Data-table editor (Phase 2)**: switch the center view to *Table editor* for a
  virtualized feature-by-attribute table (only visible rows are rendered, so 50k+
  rows stay smooth). Right-click a column header to rename it (written back as the
  property key on export), delete it, or set a per-column filter (contains / equals /
  numeric range; filters combine with AND logic, shown in the toolbar). Drag column
  headers to reorder. Structural edits (rename/delete/reorder) are undoable for the
  session (Ctrl+Z). **Export GeoJSON…** writes an RFC 7946, UTF-8 FeatureCollection
  that preserves geometries exactly, applies the new column order and renamed keys,
  and — with the *Export only filtered rows* checkbox — respects or ignores the
  active filters.

For the full manual QGIS verification procedure see
[docs/QGIS_TEST.md](docs/QGIS_TEST.md).

## Architecture

```
src/
├── main.rs      – eframe entry point, window + icon
├── model/
│   ├── geo.rs   – RFC 7946 parsing, field/type/distinct-value analysis (pure)
│   └── style.rs – style model: plain serde structs, color ramp (pure)
├── io/          – background loading thread, style project files, portable settings
├── qml/         – pure style → QGIS 3.x XML generation (golden-file tested)
└── ui/          – egui panels: file info, symbol designer, live preview, export
```

- The QML generator consumes only the style model — no UI dependency — and is
  covered by golden-file snapshot tests (`tests/golden/*.qml`).
- Malformed GeoJSON produces a friendly red error message in the UI, never a crash.
- Coordinates are WGS84 per RFC 7946 (shown in the info panel); no CRS handling is
  invented.

## Tests

```bash
cargo test          # 24 unit tests + golden-file QML snapshot tests
cargo test -- --ignored regenerate_golden   # regenerate golden files (review diff!)
```

- **Graduated (class-break) rendering (Phase 3)**: pick a numeric attribute,
  choose *Equal interval* or *Quantile* classification with 2–10 classes, and set
  the two ramp stop colors — class symbols interpolate between them (the base
  fill/hatch/outline is inherited). Breaks and symbols are listed live and exported
  as a QGIS `graduatedSymbol` renderer.
- **Geometry inspection & statistics (Phase 3)**: the file panel shows the dataset
  bounding box, total vertex count, and per-attribute numeric statistics
  (min/max, sum, mean, standard deviation) alongside distinct values and null counts.
- **Coordinate transformation (WGS84 ↔ ETRS89 UTM 33N ↔ Soldner Berlin)**:
  transform every geometry in the loaded dataset with one click —
  - WGS84 lon/lat (EPSG:4326) ↔ ETRS89 / UTM zone 33N (EPSG:25833): pure math
    (UTM series after Snyder/Krüger, mm–cm accurate). The sub-meter
    ETRS89↔WGS84 datum difference is treated as zero (common practice).
  - WGS84 ↔ DHDN / Soldner Berlin (EPSG:3068): Cassini-Soldner projection on
    Bessel 1841 with the official Berlin parameters (central point Rauenberg,
    false easting/northing 40 000/10 000 m), **including a real datum shift**
    DHDN↔WGS84 via the 7-parameter Helmert transformation (EPSG:1777,
    ~1 m accuracy; sub-centimetre would require the NTv2 BeTA grid). Round
    trips close to ~1 cm (2D chains cannot carry the ~40 m ellipsoid-height
    misclosure the datum shift induces — same behaviour as PROJ for 2D input).
  No PROJ dependency — the single-EXE portability is untouched. The current
  CRS (including EPSG code) and units-aware extent are shown, transformed data
  exports like any edited dataset, and warnings appear when data lies outside
  the 12°E–18°E UTM zone or the Berlin Soldner area of use.

## Roadmap

- **Phase 2 (done)** — data-table editor: virtualized table, rename/reorder/delete
  columns (with undo), per-column filters, GeoJSON export.
- **Phase 3 (partially done)** — graduated (class-break) styles with color ramps,
  geometry inspection, CRS info line, attribute statistics: **implemented**.
  Remaining backlog: GeoPackage/SHP import, batch conversion (deliberately
  deferred — they would need format libraries that threaten the portable
  single-EXE constraint).

The architecture keeps Phase 2/3 concerns separated (`model/`, `io/`, `qml/`, `ui/`)
so the table editor cannot break the styling/export pipeline.

## License

MIT — see [LICENSE](LICENSE).
