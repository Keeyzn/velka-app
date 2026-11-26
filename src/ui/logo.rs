//! Logo de l'application : icône de fenêtre et logo teinté selon le thème.
use eframe::egui::{self, Color32, Rect, TextureHandle, Vec2};

const MARK_PNG: &[u8] = include_bytes!("../../assets/logo/mark-white.png");
pub const ICON_PNG: &[u8] = include_bytes!("../../assets/logo/app-icon-256.png");

/// Bordure transparente ajoutée autour de la texture du logo (en pixels).
const PAD: usize = 6;

/// Icône de la fenêtre et de la barre des tâches.
pub fn window_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn texture(ctx: &egui::Context) -> Option<TextureHandle> {
    let id = egui::Id::unique("logo_mark_texture");
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(tex);
    }
    let img = image::load_from_memory_with_format(MARK_PNG, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    // Bordure transparente : sans elle, le filtrage étire les pixels du bord quand le logo tourne.
    let (w, h) = (img.width() as usize, img.height() as usize);
    let (pw, ph) = (w + 2 * PAD, h + 2 * PAD);
    let mut padded = vec![0u8; pw * ph * 4];
    for y in 0..h {
        let src = y * w * 4;
        let dst = ((y + PAD) * pw + PAD) * 4;
        padded[dst..dst + w * 4].copy_from_slice(&img.as_raw()[src..src + w * 4]);
    }
    let color = egui::ColorImage::from_rgba_unmultiplied([pw, ph], &padded);
    let tex = ctx.load_texture("logo_mark", color, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

/// Rectangle agrandi pour que le logo garde sa taille malgré la bordure transparente de la texture.
fn padded(tex: &TextureHandle, rect: Rect) -> Rect {
    let w = tex.size()[0] as f32;
    let k = w / (w - 2.0 * PAD as f32);
    Rect::from_center_size(rect.center(), rect.size() * k)
}

/// Dessine le logo (deux triangles) dans `rect`, de la couleur donnée.
pub fn paint(ui: &egui::Ui, rect: Rect, color: Color32) {
    if let Some(tex) = texture(ui.ctx()) {
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(tex.id(), padded(&tex, rect), uv, color);
    }
}

/// Comme `paint`, tourné de `angle` radians autour du centre de `rect`.
pub fn paint_rotated(ui: &egui::Ui, rect: Rect, color: Color32, angle: f32) {
    if let Some(tex) = texture(ui.ctx()) {
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        let mut mesh = egui::epaint::Mesh::with_texture(tex.id());
        mesh.add_rect_with_uv(padded(&tex, rect), uv, color);
        mesh.rotate(egui::emath::Rot2::from_angle(angle), rect.center());
        ui.painter().add(egui::Shape::mesh(mesh));
    }
}

/// Réserve un carré et y dessine le logo ; renvoie la réponse de la zone.
pub fn show(ui: &mut egui::Ui, size: f32, color: Color32, sense: egui::Sense) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(size), sense);
    paint(ui, r, color);
    resp
}
