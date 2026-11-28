use eframe::egui::{self, Align2, Margin, Order, RichText, Rounding, Sense, Stroke, Vec2};

use crate::ui::state::ProfileFormState;
use crate::ui::theme::{
    destructive_button, modal_card_frame, primary_button, responsive_modal_size, secondary_button,
    semibold, Palette,
};
use crate::ui::KrakenApp;

pub fn paint_backdrop(ctx: &egui::Context, p: &Palette, id: &str) {
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new(id))
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, Rounding::ZERO, p.backdrop);
        });
}

impl KrakenApp {
    pub(super) fn render_cert_pin_modal(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.manager.pending_cert_prompt() else {
            return;
        };
        let p = self.palette();
        let screen = ctx.screen_rect();
        let size = responsive_modal_size(screen, 480.0, 340.0);

        paint_backdrop(ctx, &p, "cert_pin_backdrop");

        egui::Area::new(egui::Id::new("cert_pin_modal"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                modal_card_frame(&p, Margin::same(22.0)).show(ui, |ui| {
                    ui.set_width(size.x - 44.0);
                    ui.label(semibold("Verify server certificate", 17.0, p.on_surface));
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!(
                                "OpenConnect could not verify the TLS certificate for '{}' ({}).",
                                prompt.profile_name, prompt.server
                            ))
                            .size(13.0)
                            .color(p.on_surface_variant),
                        )
                        .wrap(),
                    );
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("SHA-256 certificate pin")
                            .size(12.0)
                            .color(p.on_surface_variant),
                    );
                    ui.add_space(4.0);
                    egui::Frame::none()
                        .fill(p.surface_low)
                        .rounding(Rounding::same(8.0))
                        .stroke(Stroke::new(1.0_f32, p.outline_variant))
                        .inner_margin(Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&prompt.fingerprint)
                                        .monospace()
                                        .size(12.0)
                                        .color(p.on_surface),
                                )
                                .wrap(),
                            );
                        });
                    ui.add_space(10.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(
                                "Only trust this fingerprint if it matches your organization's VPN gateway.",
                            )
                            .size(12.0)
                            .color(p.outline),
                        )
                        .wrap(),
                    );
                    ui.add_space(16.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, &p, "Trust, pin & reconnect").clicked() {
                            if let Some(prof) =
                                self.profiles.iter_mut().find(|x| x.id == prompt.profile_id)
                            {
                                prof.servercert = Some(prompt.fingerprint.clone());
                                let cloned = prof.clone();
                                self.save_profiles_to_disk();
                                if let Err(e) = self.manager.trust_cert_and_connect(
                                    cloned,
                                    &self.settings,
                                    prompt.fingerprint,
                                ) {
                                    self.notify(e, true);
                                }
                            } else {
                                self.manager.dismiss_cert_prompt();
                            }
                        }
                        if secondary_button(ui, &p, "Cancel").clicked() {
                            self.manager.dismiss_cert_prompt();
                        }
                    });
                });
            });
    }

    pub(super) fn render_delete_confirm_modal(&mut self, ctx: &egui::Context) {
        let Some(target_id) = self.delete_confirm_id else {
            return;
        };
        let Some(target_name) = self
            .profiles
            .iter()
            .find(|x| x.id == target_id)
            .map(|x| x.name.clone())
        else {
            self.delete_confirm_id = None;
            return;
        };

        let p = self.palette();
        let size = responsive_modal_size(ctx.screen_rect(), 420.0, 220.0);
        paint_backdrop(ctx, &p, "delete_confirm_backdrop");

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.delete_confirm_id = None;
            return;
        }

        egui::Area::new(egui::Id::new("delete_confirm_modal"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                modal_card_frame(&p, Margin::same(22.0)).show(ui, |ui| {
                    ui.set_width(size.x - 44.0);
                    ui.label(semibold("Delete VPN profile?", 17.0, p.on_surface));
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!(
                                "Remove '{}' and its stored credentials from this computer? If it is currently connected, the tunnel will be closed first.",
                                target_name
                            ))
                            .size(13.0)
                            .color(p.on_surface_variant),
                        )
                        .wrap(),
                    );
                    ui.add_space(18.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if destructive_button(ui, &p, "Delete profile").clicked() {
                            if self.manager.state().active_profile_id() == Some(target_id) {
                                self.manager.disconnect_sync("Deleted active profile");
                            }
                            self.profiles.retain(|x| x.id != target_id);
                            if self.profile_form.editing_id == Some(target_id) {
                                self.profile_form = ProfileFormState::default();
                                self.profile_modal_open = false;
                            }
                            self.save_profiles_to_disk();
                            self.delete_confirm_id = None;
                            self.notify(format!("Deleted profile '{target_name}'."), false);
                        }
                        if secondary_button(ui, &p, "Cancel").clicked() {
                            self.delete_confirm_id = None;
                        }
                    });
                });
            });
    }
}
