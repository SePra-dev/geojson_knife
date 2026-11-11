# Manual test script — verifying exported `.qml` in QGIS 3.x

This is the acceptance procedure for the QML export milestone (M3).
It must be run on a Windows machine with QGIS 3.x (tested target: 3.28, any
3.x should behave the same) before a release is considered done.

## Setup

1. Build the app: `cargo build --release` (MSVC toolchain).
2. Start `target/release/geojson_knife.exe`.
3. Open QGIS 3.x (no special plugins needed).

## Test 1 — single-symbol polygon style (flat fill + hatching + outline)

1. In GeoJSON Toolkit, open `samples/polygons.geojson`.
2. Keep renderer **Single symbol**, geometry **Polygon**.
3. Fill color: pick a blue, opacity ~50 %, enable **Hatching** and set
   angle 45°, spacing 2 mm, line width 0.25 mm, hatch color black.
4. Outline: dark color, width 0.26 mm.
5. Click **Export .qml…** → save as `polygons.qml`.
6. In QGIS: *Layer → Add Layer → Add Vector Layer…* → select
   `samples/polygons.geojson`.
7. *Layer Properties → Symbology → Style (button at bottom) → Load Style…*
   → select `polygons.qml` → *Apply*.

**Expected:**
- QGIS loads the style **without any warning or error** (the style log
  *View → Panels → Log Messages* stays clean).
- The layer renders as a semi-transparent blue fill with black 45° hatching
  at ~2 mm spacing and a thin dark outline.
- The QGIS style dialog shows one symbol with two layers:
  `LinePatternFill` (top) and `SimpleFill` (bottom).
- The rendering matches the live preview in GeoJSON Toolkit (same colors,
  angle, spacing).

## Test 2 — single-symbol line style

1. Open `samples/lines.geojson`, geometry **LineString**.
2. Pick a line color and width (e.g. 1.5 mm), export `lines.qml`.
3. Load it in QGIS on the `lines.geojson` layer as above.

**Expected:** all lines render in the chosen color/width; QGIS shows a single
`SimpleLine` layer; no warnings.

## Test 3 — single-symbol point style

1. Open `samples/points.geojson`, geometry **Point**.
2. Pick marker size (e.g. 4 mm), color, outline; export `points.qml`.
3. Load it in QGIS on the `points.geojson` layer.

**Expected:** points render as circles of the chosen size/color with outline;
QGIS shows a single `SimpleMarker` layer; no warnings.

## Test 4 — categorized renderer

1. Open `samples/polygons.geojson`.
2. Renderer → **Categorized**, attribute → `type`, click **Build categories**
   (3 categories: residential / industrial / park).
3. Optionally override one category's color and one hatch angle; use
   *Apply hatch to all* to sync hatching.
4. Export `polygons_categorized.qml`.
5. Load it in QGIS.

**Expected:**
- QGIS shows a *Categorized* renderer with one category per distinct value of
  the `type` attribute, each with its own color.
- Each feature is colored by its `type` value and the colors match the app.
- Category labels appear in the layer legend.
- No warnings.

## Test 5 — attribute names with spaces

1. Open any sample, rename... (Phase 1 note: pick a file whose attribute name
   contains a space, or create one) and export a categorized style for it.

**Expected:** the `.qml` contains the attribute as `"my attr"` (quoted inside
`attr="…"`), and QGIS resolves it correctly.

## Test 6 — robustness

1. Drag `samples/malformed.geojson` onto the app window.

**Expected:** a red error message appears in the status bar; the app keeps
running; previously loaded data stays usable.

2. Open `samples/large_points.geojson` (60,000 features).

**Expected:** the UI stays responsive (spinner in the left panel), and the
file info shows 60,000 features and attributes id/kind/score with correct
types (number/text/number).

## Test 7 — style project round-trip

1. Design any style, click **Save style** → `my_style.json`.
2. Restart the app, click **Load style** → `my_style.json`.

**Expected:** all colors, hatch parameters and categories are exactly as saved.

## Result recording

Record pass/fail per test with the QGIS version used (e.g. "QGIS 3.28.9-Firenze,
all tests passed 2024-XX-XX"). Any warning emitted by QGIS when loading a
`.qml` is a **failure** — file it as a bug against `src/qml/mod.rs`.
