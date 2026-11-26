//! Panneau de chat : liste défilante, défilement automatique, emotes, liens.
use crate::chat::client::{AuthState, ConnState};
use crate::chat::emotes::Emotes;
use crate::chat::parse::{ChatMessage, Segment};
use crate::chat::ChatState;
use eframe::egui::{self, Color32, RichText};

const EMOTE_SIZE: f32 = 26.0;
const BADGE_SIZE: f32 = 18.0;

/// Couleur de repli stable, dérivée du pseudo.
fn fallback_color(name: &str) -> Color32 {
    const PALETTE: [(u8, u8, u8); 8] = [
        (255, 99, 71),
        (30, 144, 255),
        (50, 205, 50),
        (218, 112, 214),
        (255, 165, 0),
        (0, 206, 209),
        (154, 205, 50),
        (255, 105, 180),
    ];
    let h = name
        .bytes()
        .fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    let (r, g, b) = PALETTE[h % PALETTE.len()];
    Color32::from_rgb(r, g, b)
}

/// Options d'affichage du chat (réglages).
pub struct ChatOpts {
    /// Échelle du texte et des emotes (1,0 = taille d'origine).
    pub scale: f32,
    pub timestamps: bool,
    /// Images des badges (option « Badges du chat ») ; `None` = pas de badges.
    pub badges: Option<std::sync::Arc<crate::chat::extra::ChatExtras>>,
    /// Chat followers-only et compte qui ne suit pas la chaîne : la saisie est désactivée.
    pub followers_locked: bool,
}

/// Image d'emote ou de badge ; une image animée visible redemande un affichage pour l'image suivante.
fn image(ui: &mut egui::Ui, tex: egui::TextureId, animated: bool, size: f32) -> egui::Response {
    let resp = ui.add(egui::Image::new((tex, egui::vec2(size, size))));
    if animated && ui.is_rect_visible(resp.rect) {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(40));
    }
    resp
}

fn message_ui(ui: &mut egui::Ui, m: &ChatMessage, emotes: &mut Emotes, opts: &ChatOpts) {
    let fill = if m.is_mention {
        Color32::from_rgba_unmultiplied(220, 60, 60, 50)
    } else if m.is_reply {
        Color32::from_rgba_unmultiplied(59, 130, 246, 45)
    } else if m.highlight {
        Color32::from_rgba_unmultiplied(240, 180, 40, 55)
    } else {
        Color32::TRANSPARENT
    };
    egui::Frame::new()
        .fill(fill)
        .inner_margin(2.0)
        .show(ui, |ui| {
            // Seuls textes de l'application à rester sélectionnables (copie d'un message ou d'un lien).
            ui.style_mut().interaction.selectable_labels = true;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                if opts.timestamps && !m.time.is_empty() {
                    ui.label(
                        RichText::new(&m.time)
                            .size(11.0 * opts.scale)
                            .color(crate::ui::theme::FAINT()),
                    );
                }
                let time = ui.input(|i| i.time);
                if let Some(x) = &opts.badges {
                    for b in &m.badges {
                        if let Some((tex, animated)) =
                            x.badges.get(b).and_then(|key| emotes.get(key, time))
                        {
                            image(ui, tex, animated, BADGE_SIZE * opts.scale);
                        }
                    }
                }
                // Couleur choisie sur Twitch, ajustée pour rester lisible (4,5:1) sur le fond du chat dans chaque thème.
                let color = m
                    .color
                    .map(|(r, g, b)| Color32::from_rgb(r, g, b))
                    .unwrap_or_else(|| fallback_color(&m.user));
                let color = crate::ui::theme::readable_on(color, crate::ui::theme::SURFACE_1());
                ui.label(RichText::new(&m.user).color(color).strong());
                ui.label(":");
                for seg in &m.segments {
                    match seg {
                        Segment::Text(t) => {
                            let mut rt = RichText::new(crate::twitch::strip_emoji(t));
                            if m.is_action {
                                rt = rt.italics().color(color);
                            }
                            ui.label(rt);
                        }
                        Segment::Emote { id, name } => match emotes.get(id, time) {
                            Some((tex, animated)) => {
                                let resp = image(ui, tex, animated, EMOTE_SIZE * opts.scale);
                                crate::ui::theme::show_tip(resp, name.as_str());
                            }
                            None => {
                                ui.label(name.as_str());
                            }
                        },
                        Segment::Link(url) => {
                            ui.hyperlink_to(url.as_str(), url.as_str());
                        }
                    }
                }
            });
        });
}

/// Dessine le chat. `jump_to_bottom` force le retour en bas (puis repasse à `false`).
/// Renvoie le message à envoyer si l'utilisateur vient de valider le champ de saisie.
pub fn show(
    ui: &mut egui::Ui,
    chat: &ChatState,
    emotes: &mut Emotes,
    jump_to_bottom: &mut bool,
    input: &mut String,
    opts: &ChatOpts,
) -> Option<String> {
    let mut to_send = None;
    let status = match &chat.conn {
        ConnState::Connecting => crate::i18n::tr("Connexion au chat…").to_string(),
        ConnState::Connected => match &chat.auth {
            AuthState::Authenticated(n) => format!("{} · {n}", crate::i18n::tr("Chat connecté")),
            _ => crate::i18n::tr("Chat connecté").to_string(),
        },
        ConnState::Reconnecting(s) => {
            format!("{} ({s} s)…", crate::i18n::tr("Reconnexion du chat"))
        }
    };
    ui.label(
        egui::RichText::new(status)
            .size(12.0)
            .color(crate::ui::theme::MUTED()),
    );
    ui.separator();

    // Champ de saisie en bas : actif seulement avec un jeton valide.
    egui::Panel::bottom("chat_input_panel")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: 8,
            bottom: 0,
        }))
        .show_separator_line(false)
        .show(ui, |ui| {
            let can_send =
                matches!(chat.auth, AuthState::Authenticated(_)) && !opts.followers_locked;
            let hint = match &chat.auth {
                AuthState::Authenticated(_) if opts.followers_locked => {
                    crate::i18n::tr("Chat réservé aux followers")
                }
                AuthState::Authenticated(_) => crate::i18n::tr("Envoyer un message"),
                AuthState::Invalid(m) => m.as_str(),
                AuthState::Anonymous => crate::i18n::tr("Jeton requis pour écrire (Réglages)"),
            };
            let edit = ui.add_enabled(
                can_send,
                egui::TextEdit::singleline(input)
                    .hint_text(hint)
                    .char_limit(crate::chat::client::MAX_LEN)
                    .desired_width(f32::INFINITY),
            );
            if can_send && edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if !input.trim().is_empty() {
                    to_send = Some(std::mem::take(input));
                }
                edit.request_focus();
            }
        });

    let mut area = egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false, false]);
    if *jump_to_bottom {
        area = area.vertical_scroll_offset(1.0e9);
        *jump_to_bottom = false;
    }
    let out = area.show(ui, |ui| {
        if (opts.scale - 1.0).abs() > f32::EPSILON {
            ui.style_mut().override_font_id = Some(egui::FontId::proportional(14.0 * opts.scale));
        }
        // Connecté mais aucun message encore : on le dit, sinon la zone vide laisse croire à une panne.
        if chat.messages.is_empty() && matches!(chat.conn, ConnState::Connected) {
            ui.add_space(crate::ui::theme::SP_M);
            ui.label(
                egui::RichText::new(crate::i18n::tr("En attente de messages…"))
                    .size(12.5)
                    .color(crate::ui::theme::FAINT()),
            );
        }
        for m in &chat.messages {
            message_ui(ui, m, emotes, opts);
        }
    });
    let at_bottom =
        out.state.clamped_offset().y + out.inner_rect.height() >= out.content_size.y - 8.0;
    if !at_bottom {
        let rect = egui::Rect::from_center_size(
            egui::pos2(out.inner_rect.center().x, out.inner_rect.bottom() - 22.0),
            egui::vec2(150.0, 26.0),
        );
        // Bouton dessiné et sensible sur toute sa surface (posé après la liste : il est prioritaire sur les messages
        // dessous, dont les textes sélectionnables donneraient le curseur « texte »).
        let label = crate::i18n::tr("Revenir en bas");
        let resp = ui.interact(
            rect,
            egui::Id::unique("chat_jump_button"),
            egui::Sense::click(),
        );
        let k = crate::ui::theme::hover_k(ui, resp.id, resp.hovered());
        let fill = crate::ui::theme::SURFACE_2().lerp_to_gamma(crate::ui::theme::ACCENT(), k);
        let stroke = egui::Stroke::new(1.0, crate::ui::theme::BORDER().gamma_multiply(1.0 - k));
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(crate::ui::theme::RADIUS_INNER),
            fill,
            stroke,
            egui::StrokeKind::Inside,
        );
        crate::ui::theme::paint_pressed(ui, &resp, rect, crate::ui::theme::RADIUS_INNER);
        let fg = crate::ui::theme::TEXT().lerp_to_gamma(Color32::WHITE, k);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(14.0),
            fg,
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        crate::ui::theme::focus_ring(ui, &resp, rect, crate::ui::theme::RADIUS_INNER);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        if resp.clicked() {
            // Retour direct au dernier message (décalage posé d'un coup, sans refaire le défilement).
            *jump_to_bottom = true;
            ui.ctx().request_repaint();
        }
    }
    to_send
}
