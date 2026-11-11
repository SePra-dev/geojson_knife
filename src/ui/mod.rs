//! egui/eframe UI: file info panel, style designer with live vector preview,
//! categorized rendering controls, and export.

use crate::io::{AppSettings, LoadHandle, LoadResult};
use crate::model::{Dataset, GeometryKind, StyleProject};
use crate::qml;
use eframe::egui;
use egui::{Align2, Color32, Pos2, Sense, Vec2};

use crate::model::{Category, Color, Fill, Hatch, LineSymbol, PointSymbol, RendererKind};

pub struct GeoJsonToolkitApp {
    settings: AppSettings,
    dataset: Option<(String, Dataset)>,
    load: Option<LoadHandle>,
    style: StyleProject,
    file_error: Option<String>,
    status: String,
    field_sort: FieldSort,
    /// Attribute chosen for categorized rendering (UI selection, applied on "Build categories").
    selected_attr: Option<String>,
    /// Geometry kind used for the preview/export symbol family.
    preview_kind: GeometryKind,
    show_about: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum FieldSort {
    NameAsc,
    NameDesc,
}

fn to_color32(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

fn from_color32(c: Color32) -> Color {
    let [r, g, b, a] = c.to_array();
    Color { r, g, b, a }
}

impl Default for GeoJsonToolkitApp {
    fn default() -> Self {
        Self {
            settings: AppSettings::load(),
            dataset: None,
            load: None,
            style: StyleProject::default(),
            file_error: None,
            status: "Open a GeoJSON file to begin.".into(),
            field_sort: FieldSort::NameAsc,
            selected_attr: None,
            preview_kind: GeometryKind::Polygon,
            show_about: false,
        }
    }
}

impl GeoJsonToolkitApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    fn open_path(&mut self, path: std::path::PathBuf) {
        self.file_error = None;
        self.dataset = None;
        self.status = format!("Loading {}…", path.display());
        self.load = Some(LoadHandle::spawn(path));
    }

    fn pick_file(&mut self) {
        let start_dir = self.settings.last_dir.clone();
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open GeoJSON file")
            .add_filter("GeoJSON", &["geojson", "json"])
            .set_directory(start_dir.unwrap_or_else(|| ".".into()))
            .pick_file()
        {
            if let Some(dir) = path.parent() {
                self.settings.last_dir = Some(dir.to_string_lossy().into_owned());
                let _ = self.settings.save();
            }
            self.open_path(path);
        }
    }

    fn apply_load_result(&mut self, result: LoadResult) {
        match result {
            Ok((name, ds)) => {
                self.preview_kind = ds
                    .geometry_kinds
                    .first()
                    .copied()
                    .unwrap_or(GeometryKind::Point);
                self.selected_attr = None;
                self.status = format!(
                    "Loaded {name}: {} features, {} attribute(s).",
                    ds.features.len(),
                    ds.fields.len()
                );
                self.dataset = Some((name, ds));
            }
            Err(e) => {
                self.file_error = Some(e.clone());
                self.status = format!("Failed to load file: {e}");
            }
        }
    }

    fn export_qml(&mut self) {
        let Some((name, _)) = &self.dataset else {
            self.status = "Open a GeoJSON file first.".into();
            return;
        };
        let stem = name.trim_end_matches(".geojson").trim_end_matches(".json");
        let default_name = format!("{stem}.qml");
        let start_dir = self.settings.last_dir.clone();
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export QGIS style file")
            .add_filter("QGIS style", &["qml"])
            .set_file_name(&default_name)
            .set_directory(start_dir.unwrap_or_else(|| ".".into()))
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, qml::generate_qml(&self.style, self.preview_kind)) {
            Ok(_) => self.status = format!("Exported {}.", path.display()),
            Err(e) => {
                self.file_error = Some(format!("could not write file: {e}"));
                self.status = format!("Export failed: {e}");
            }
        }
    }

    fn save_style(&mut self) {
        let default_name = match &self.dataset {
            Some((name, _)) => format!(
                "{}_style.json",
                name.trim_end_matches(".geojson").trim_end_matches(".json")
            ),
            None => "style.json".into(),
        };
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save style project")
            .add_filter("Style project", &["json"])
            .set_file_name(&default_name)
            .save_file()
        else {
            return;
        };
        match crate::io::save_style_project(&self.style, &path) {
            Ok(_) => self.status = format!("Saved {}.", path.display()),
            Err(e) => self.status = e,
        }
    }

    fn load_style(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Load style project")
            .add_filter("Style project", &["json"])
            .pick_file()
        else {
            return;
        };
        match crate::io::load_style_project(&path) {
            Ok(style) => {
                self.selected_attr = style.attr.clone();
                self.style = style;
                self.status = format!("Loaded style from {}.", path.display());
            }
            Err(e) => self.status = e,
        }
    }
}

impl eframe::App for GeoJsonToolkitApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll background loader.
        if let Some(handle) = &self.load {
            if let Some(result) = handle.try_recv() {
                self.apply_load_result(result);
                self.load = None;
            }
        }

        // Drag & drop.
        let dropped: Vec<std::path::PathBuf> = ctx
            .input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .filter_map(|f| f.path.clone())
                    .collect()
            });
        for path in dropped {
            self.open_path(path);
        }

        // Keep polling while a file is loading so the UI stays responsive.
        if self.load.is_some() {
            ctx.request_repaint();
        }

        self.top_bar(ctx);
        self.left_panel(ctx);
        self.central_panel(ctx);
    }
}

impl GeoJsonToolkitApp {
    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("GeoJSON Toolkit");
                ui.separator();
                if ui.button("Open GeoJSON…").clicked() {
                    self.pick_file();
                }
                ui.separator();
                if ui.button("Save style").clicked() {
                    self.save_style();
                }
                if ui.button("Load style").clicked() {
                    self.load_style();
                }
                ui.separator();
                if ui.button("Export .qml…").clicked() {
                    self.export_qml();
                }
                with_dialog(ui, "About", &mut self.show_about, |ui| {
                    ui.heading("GeoJSON Toolkit");
                    ui.label("A portable Swiss Army knife for GeoJSON files.");
                    ui.label("MVP: map symbol styling and QGIS .qml export.");
                    ui.label(
                        "Coordinates are WGS84 (RFC 7946). Styles are QGIS 3.x compatible.",
                    );
                    ui.label("Settings are stored next to the executable (portable).");
                });
            });
        });
        egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = &self.file_error {
                    ui.colored_label(egui::Color32::RED, format!("⚠ {err}"));
                } else {
                    ui.label(&self.status);
                }
            });
        });
    }

    fn left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("left_panel")
            .default_width(340.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.file_info(ui);
                    ui.separator();
                    self.attribute_table(ui);
                });
            });
    }

    fn file_info(&mut self, ui: &mut egui::Ui) {
        ui.heading("File");
        match &self.dataset {
            Some((name, ds)) => {
                ui.label(format!("Name: {name}"));
                ui.label(format!("Features: {}", ds.features.len()));
                let kinds: Vec<&str> = ds
                    .geometry_kinds
                    .iter()
                    .map(|k| k.label())
                    .collect();
                ui.label(format!("Geometry: {}", kinds.join(", ")));
                ui.label("CRS: WGS84 (RFC 7946)");
            }
            None => {
                ui.label("No file loaded.");
                if self.load.is_some() {
                    ui.spinner();
                    ui.label("Parsing…");
                }
                ui.label("Drag & drop a .geojson file onto this window.");
            }
        }
    }

    fn attribute_table(&mut self, ui: &mut egui::Ui) {
        let Some((_, ds)) = &self.dataset else {
            return;
        };
        ui.heading("Attributes");
        ui.horizontal(|ui| {
            ui.label("Sort:");
            if ui
                .selectable_value(&mut self.field_sort, FieldSort::NameAsc, "A→Z")
                .clicked()
            {}
            if ui
                .selectable_value(&mut self.field_sort, FieldSort::NameDesc, "Z→A")
                .clicked()
            {}
        });
        egui::Grid::new("fields")
            .striped(true)
            .num_columns(4)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.strong("Field");
                ui.strong("Type");
                ui.strong("Values");
                ui.strong("Nulls");
                ui.end_row();
                let mut fields: Vec<_> = ds.fields.iter().collect();
                match self.field_sort {
                    FieldSort::NameAsc => fields.sort_by(|a, b| a.name.cmp(&b.name)),
                    FieldSort::NameDesc => fields.sort_by(|a, b| b.name.cmp(&a.name)),
                }
                for f in fields {
                    ui.label(&f.name);
                    ui.label(f.ty.label());
                    match (f.ty, f.num_min, f.num_max) {
                        (crate::model::AttrType::Number, Some(min), Some(max)) => {
                            ui.label(format!("min {min}, max {max}"));
                        }
                        _ => {
                            let preview = if f.distinct.len() < f.distinct_count {
                                format!(
                                    "{} … ({} distinct)",
                                    f.distinct.join(", "),
                                    f.distinct_count
                                )
                            } else {
                                f.distinct.join(", ")
                            };
                            ui.label(shorten(&preview, 60));
                        }
                    }
                    ui.label(f.null_count.to_string());
                    ui.end_row();
                }
            });
    }

    fn central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.dataset.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.heading(
                        "Open a GeoJSON file (button or drag & drop) to start styling.",
                    );
                });
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.renderer_controls(ui);
                ui.separator();
                match self.preview_kind.symbol_family() {
                    "fill" => self.fill_designer(ui),
                    "line" => self.line_designer(ui),
                    _ => self.point_designer(ui),
                }
                ui.separator();
                self.preview(ui);
                if self.style.renderer == RendererKind::Categorized {
                    ui.separator();
                    self.category_designer(ui);
                }
            });
        });
    }

    fn renderer_controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Renderer");
        ui.horizontal(|ui| {
            ui.label("Type:");
            if ui
                .selectable_value(
                    &mut self.style.renderer,
                    RendererKind::SingleSymbol,
                    "Single symbol",
                )
                .clicked()
            {}
            if ui
                .selectable_value(
                    &mut self.style.renderer,
                    RendererKind::Categorized,
                    "Categorized",
                )
                .clicked()
            {}
            ui.separator();
            ui.label("Geometry:");
            let kinds = self
                .dataset
                .as_ref()
                .map(|(_, ds)| ds.geometry_kinds.clone())
                .unwrap_or_default();
            for k in kinds {
                if ui
                    .selectable_value(&mut self.preview_kind, k, k.label())
                    .clicked()
                {}
            }
        });
        if self.style.renderer == RendererKind::Categorized {
            ui.horizontal(|ui| {
                ui.label("Attribute:");
                let fields = self
                    .dataset
                    .as_ref()
                    .map(|(_, ds)| ds.fields.clone())
                    .unwrap_or_default();
                let selected = self.selected_attr.clone().unwrap_or_default();
                let mut changed = false;
                egui::ComboBox::from_id_source("attr_combo")
                    .selected_text(if selected.is_empty() { "– pick –" } else { &selected })
                    .show_ui(ui, |ui| {
                        for f in &fields {
                            changed |= ui
                                .selectable_value(&mut self.selected_attr, Some(f.name.clone()), &f.name)
                                .clicked();
                        }
                    });
                let _ = changed;
                if ui.button("Build categories").clicked() {
                    if let Some(attr) = self.selected_attr.clone() {
                        if let Some((_, ds)) = &self.dataset {
                            if let Some(f) = ds.fields.iter().find(|f| f.name == attr) {
                                self.style.build_categories(&attr, &f.distinct);
                                self.style.attr = Some(attr.clone());
                                self.status = format!(
                                    "Built {} categories from '{attr}'.",
                                    self.style.categories.len()
                                );
                            }
                        }
                    }
                }
            });
        }
    }

    fn fill_designer(&mut self, ui: &mut egui::Ui) {
        ui.heading("Fill");
        let mut fill = self.style.fill.clone();
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Fill color:");
            let mut c = to_color32(fill.color);
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut c,
                egui::color_picker::Alpha::OnlyBlend,
            )
            .changed()
            {
                fill.color = from_color32(c);
                changed = true;
            }
            let mut opacity = fill.color.a as f32 / 255.0;
            ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0).text("opacity"));
            if opacity != fill.color.a as f32 / 255.0 {
                fill.color.a = (opacity * 255.0).round() as u8;
                changed = true;
            }
        });
        let mut hatch_on = fill.hatch.is_some();
        if ui.checkbox(&mut hatch_on, "Hatching").changed() {
            fill.hatch = if hatch_on {
                Some(Hatch {
                    angle: 45,
                    distance_mm: 2.0,
                    line_width_mm: 0.25,
                    color: Color::BLACK,
                })
            } else {
                None
            };
            changed = true;
        }
        if let Some(h) = &mut fill.hatch {
            ui.horizontal(|ui| {
                let mut angle = h.angle as f32;
                ui.add(egui::Slider::new(&mut angle, 0.0..=359.0).text("angle (°)"));
                h.angle = angle.round() as u16;
                let mut dist = h.distance_mm;
                ui.add(egui::Slider::new(&mut dist, 0.5..=10.0).text("spacing (mm)"));
                h.distance_mm = dist;
                let mut lw = h.line_width_mm;
                ui.add(egui::Slider::new(&mut lw, 0.05..=2.0).text("line width (mm)"));
                h.line_width_mm = lw;
            });
            ui.horizontal(|ui| {
                ui.label("Hatch color:");
                let mut c = to_color32(h.color);
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut c,
                    egui::color_picker::Alpha::OnlyBlend,
                );
                h.color = from_color32(c);
            });
            changed = true;
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Outline color:");
            let mut c = to_color32(fill.outline.color);
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut c,
                egui::color_picker::Alpha::OnlyBlend,
            )
            .changed()
            {
                fill.outline.color = from_color32(c);
                changed = true;
            }
            let mut w = fill.outline.width_mm;
            ui.add(egui::Slider::new(&mut w, 0.0..=5.0).text("outline width (mm)"));
            if w != fill.outline.width_mm {
                fill.outline.width_mm = w;
                changed = true;
            }
        });
        let _ = changed;
        self.style.fill = fill;
    }

    fn line_designer(&mut self, ui: &mut egui::Ui) {
        ui.heading("Line");
        let mut line = self.style.line.clone();
        ui.horizontal(|ui| {
            ui.label("Color:");
            let mut c = to_color32(line.color);
            egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::OnlyBlend);
            line.color = from_color32(c);
            let mut w = line.width_mm;
            ui.add(egui::Slider::new(&mut w, 0.1..=10.0).text("width (mm)"));
            line.width_mm = w;
        });
        self.style.line = line;
    }

    fn point_designer(&mut self, ui: &mut egui::Ui) {
        ui.heading("Point");
        let mut point = self.style.point.clone();
        ui.horizontal(|ui| {
            ui.label("Color:");
            let mut c = to_color32(point.color);
            egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::OnlyBlend);
            point.color = from_color32(c);
            let mut s = point.size_mm;
            ui.add(egui::Slider::new(&mut s, 0.5..=20.0).text("size (mm)"));
            point.size_mm = s;
        });
        ui.horizontal(|ui| {
            ui.label("Outline color:");
            let mut c = to_color32(point.outline.color);
            egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::OnlyBlend);
            point.outline.color = from_color32(c);
            let mut w = point.outline.width_mm;
            ui.add(egui::Slider::new(&mut w, 0.0..=2.0).text("outline width (mm)"));
            point.outline.width_mm = w;
        });
        self.style.point = point;
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        ui.heading("Preview");
        let size = Vec2::new(ui.available_width().min(600.0), 220.0);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, Color32::from_gray(250));

        let mm = 4.0; // preview scale: mm -> px
        match self.preview_kind.symbol_family() {
            "fill" => draw_fill_preview(&painter, rect, &self.style.fill, mm),
            "line" => draw_line_preview(&painter, rect, &self.style.line, mm),
            _ => draw_point_preview(&painter, rect, &self.style.point, mm),
        }
    }

    fn category_designer(&mut self, ui: &mut egui::Ui) {
        ui.heading("Categories");
        ui.horizontal(|ui| {
            if ui.button("Apply hatch to all").clicked() {
                let hatch = self.style.categories.first().and_then(|c| c.fill.hatch.clone());
                self.style.apply_hatch_to_all(hatch);
            }
            if ui.button("Apply outline to all").clicked() {
                let outline = self
                    .style
                    .categories
                    .first()
                    .map(|c| c.fill.outline.clone())
                    .unwrap_or_default();
                self.style.apply_outline_to_all(outline);
            }
        });
        ui.add_space(4.0);
        let mut new_cats: Vec<Category> = Vec::new();
        let mut rebuild = false;
        egui::Grid::new("categories")
            .striped(true)
            .num_columns(5)
            .show(ui, |ui| {
                ui.strong("Value");
                ui.strong("Label");
                ui.strong("Color");
                ui.strong("Hatch angle");
                ui.strong("Render");
                ui.end_row();
                for cat in &mut self.style.categories {
                    let mut c = cat.clone();
                    ui.label(&c.value);
                    let mut label = c.label.clone();
                    if ui.text_edit_singleline(&mut label).changed() {
                        c.label = label;
                        rebuild = true;
                    }
                    let mut col = to_color32(c.fill.color);
                    if egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut col,
                        egui::color_picker::Alpha::OnlyBlend,
                    )
                    .changed()
                    {
                        c.fill.color = from_color32(col);
                        rebuild = true;
                    }
                    if let Some(h) = &mut c.fill.hatch {
                        let mut angle = h.angle;
                        ui.add(egui::DragValue::new(&mut angle).clamp_range(0..=359).suffix("°"));
                        if angle != h.angle {
                            h.angle = angle;
                            rebuild = true;
                        }
                    } else {
                        ui.label("–");
                    }
                    if ui.checkbox(&mut c.render, "").changed() {
                        rebuild = true;
                    }
                    ui.end_row();
                    new_cats.push(c);
                }
            });
        if rebuild {
            self.style.categories = new_cats;
        }
    }
}

fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max - 1).collect();
        format!("{cut}…")
    }
}

fn with_dialog(ui: &mut egui::Ui, label: &str, open: &mut bool, body: impl FnOnce(&mut egui::Ui)) {
    if ui.small_button(label).clicked() {
        *open = true;
    }
    egui::Window::new("About")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .open(open)
        .show(ui.ctx(), |ui| {
            body(ui);
        });
}

fn draw_fill_preview(painter: &egui::Painter, rect: egui::Rect, fill: &Fill, mm: f32) {
    let shape = egui::Rect {
        min: Pos2::new(rect.min.x + 40.0, rect.min.y + 40.0),
        max: Pos2::new(rect.max.x - 40.0, rect.max.y - 40.0),
    };
    painter.rect_filled(shape, 6.0, to_color32(fill.color));

    if let Some(h) = &fill.hatch {
        let dir = Vec2::new((h.angle as f32).to_radians().cos(), (h.angle as f32).to_radians().sin());
        let normal = Vec2::new(-dir.y, dir.x);
        let spacing = (h.distance_mm * mm).max(2.0);
        let center = shape.center();
        let half_diag = shape.size().length() / 2.0 + spacing;
        let half_len = half_diag;
        let n = (half_diag / spacing).ceil() as i32;
        let line_color = to_color32(h.color);
        for k in -n..=n {
            let p = center + normal * (k as f32 * spacing);
            let a = p + dir * half_len;
            let b = p - dir * half_len;
            painter.line_segment(
                [clip_to_rect(a, shape), clip_to_rect(b, shape)],
                egui::Stroke::new((h.line_width_mm * mm).max(0.5), line_color),
            );
        }
    }

    painter.rect_stroke(
        shape,
        6.0,
        egui::Stroke::new((fill.outline.width_mm * mm).max(0.5), to_color32(fill.outline.color)),
    );
}

fn clip_to_rect(p: Pos2, rect: egui::Rect) -> Pos2 {
    Pos2::new(
        p.x.clamp(rect.min.x, rect.max.x),
        p.y.clamp(rect.min.y, rect.max.y),
    )
}

fn draw_line_preview(painter: &egui::Painter, rect: egui::Rect, line: &LineSymbol, mm: f32) {
    let mid = rect.center_bottom() - Vec2::new(0.0, rect.height() / 2.0);
    let mut points = Vec::new();
    for i in 0..60 {
        let t = i as f32 / 59.0;
        let x = rect.min.x + 30.0 + t * (rect.width() - 60.0);
        let y = mid.y + (t * std::f32::consts::TAU * 1.5).sin() * 30.0;
        points.push(Pos2::new(x, y));
    }
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new((line.width_mm * mm).max(0.5), to_color32(line.color)),
    ));
}

fn draw_point_preview(painter: &egui::Painter, rect: egui::Rect, point: &PointSymbol, mm: f32) {
    let r = (point.size_mm * mm / 2.0).max(2.0);
    for i in 0..5 {
        let x = rect.min.x + 50.0 + i as f32 * (rect.width() - 100.0) / 4.0;
        let y = rect.center().y;
        painter.circle_filled(Pos2::new(x, y), r, to_color32(point.color));
        painter.circle_stroke(
            Pos2::new(x, y),
            r,
            egui::Stroke::new(
                (point.outline.width_mm * mm).max(0.5),
                to_color32(point.outline.color),
            ),
        );
    }
}
