mod app;
mod config;
mod i18n;
mod runner;

use eframe::egui::{FontFamily, ViewportBuilder};
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../icon.png"))
        .map_err(|e| eprintln!("icon load error: {e}"))
        .ok();

    let mut viewport = ViewportBuilder::default()
        .with_inner_size([900.0, 640.0])
        .with_title("Proton Launch Manager")
        .with_app_id("proton-launch");
    if let Some(ico) = icon {
        viewport.icon = Some(Arc::new(ico));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Proton Launch Manager",
        options,
        Box::new(|cc| {
            let mut fonts = eframe::egui::FontDefinitions::default();
            fonts.font_data.insert(
                "MiSans-Normal".to_owned(),
                Arc::new(eframe::egui::FontData::from_static(
                    include_bytes!("../MiSans-Normal.ttf"),
                )),
            );
            fonts
                .families
                .get_mut(&FontFamily::Proportional)
                .unwrap()
                .insert(0, "MiSans-Normal".to_owned());
            fonts
                .families
                .get_mut(&FontFamily::Monospace)
                .unwrap()
                .insert(0, "MiSans-Normal".to_owned());
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::<app::ProtonApp>::default())
        }),
    )
}
