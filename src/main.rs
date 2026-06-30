mod app;
mod config;
mod runner;

use eframe::egui::{FontFamily, ViewportBuilder};
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([900.0, 640.0])
            .with_title("Proton Launch Manager"),
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
