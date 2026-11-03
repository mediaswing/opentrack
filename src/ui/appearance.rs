//! The two appearances: a light colour scheme set in Ubuntu Medium, and a
//! dark, high-contrast one set in Ubuntu Bold.

use egui::style::{Selection, WidgetVisuals};
use egui::{Color32, FontData, FontDefinitions, FontFamily, Stroke, Theme, Visuals};

use crate::config::Appearance;

const MEDIUM: &str = "Ubuntu-Medium";
const BOLD: &str = "Ubuntu-Bold";

/// High contrast's highlight: selection, focus and hovered outlines.
const YELLOW: Color32 = Color32::from_rgb(255, 212, 0);
/// Behind selected things in high contrast, with yellow on top.
const NAVY: Color32 = Color32::from_rgb(0, 40, 120);

impl Appearance {
    pub fn label(self) -> &'static str {
        match self {
            Appearance::Standard => "Standard",
            Appearance::HighContrast => "High contrast",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Appearance::Standard => "Light colours, medium-weight text",
            Appearance::HighContrast => "White and yellow on black, bold text",
        }
    }
}

/// Bars on the dashboard's month chart.
pub fn chart_colour(appearance: Appearance, visuals: &Visuals) -> Color32 {
    match appearance {
        Appearance::Standard => visuals.selection.bg_fill,
        // The selection fill is navy there, which barely shows on black.
        Appearance::HighContrast => YELLOW,
    }
}

/// Switch the window to `appearance`'s colours and font.
pub fn apply(ctx: &egui::Context, appearance: Appearance) {
    log::debug!("Using the {} appearance", appearance.label());
    let mut fonts = FontDefinitions::default();
    for (name, data) in [
        (MEDIUM, &include_bytes!("../../assets/fonts/Ubuntu-Medium.ttf")[..]),
        (BOLD, &include_bytes!("../../assets/fonts/Ubuntu-Bold.ttf")[..]),
    ] {
        fonts.font_data.insert(name.to_owned(), std::sync::Arc::new(FontData::from_static(data)));
    }
    // egui's own fonts stay behind it, for the characters it lacks.
    let primary = match appearance {
        Appearance::Standard => MEDIUM,
        Appearance::HighContrast => BOLD,
    };
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, primary.to_owned());
    ctx.set_fonts(fonts);

    // Each appearance fixes its colours, rather than following the system's
    // light or dark mode.
    match appearance {
        Appearance::Standard => {
            ctx.set_visuals_of(Theme::Light, Visuals::light());
            ctx.set_theme(Theme::Light);
        }
        Appearance::HighContrast => {
            ctx.set_visuals_of(Theme::Dark, high_contrast());
            ctx.set_theme(Theme::Dark);
        }
    }
}

/// White text and outlines on black, with yellow for whatever is selected,
/// focused or under the pointer.
///
/// egui draws a focused text field's outline in the selection stroke, and
/// selected text and buttons in the selection stroke on the selection fill.
/// So the stroke must stand out against the black background as well as the
/// fill: yellow on navy.
fn high_contrast() -> Visuals {
    let mut v = Visuals::dark();
    let grey = Color32::from_gray;
    v.panel_fill = Color32::BLACK;
    v.window_fill = Color32::BLACK;
    v.extreme_bg_color = Color32::BLACK;
    v.text_edit_bg_color = Some(Color32::BLACK);
    v.faint_bg_color = grey(32);
    v.code_bg_color = grey(32);
    v.window_stroke = Stroke::new(2.0, Color32::WHITE);
    v.weak_text_color = Some(grey(210));
    v.hyperlink_color = Color32::from_rgb(110, 200, 255);
    v.warn_fg_color = YELLOW;
    v.error_fg_color = Color32::from_rgb(255, 120, 120);
    v.selection = Selection { bg_fill: NAVY, stroke: Stroke::new(2.0, YELLOW) };

    let w = &mut v.widgets;
    // Labels, separators and group frames.
    w.noninteractive.bg_stroke = Stroke::new(1.0, grey(170));
    w.noninteractive.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    let style = |wv: &mut WidgetVisuals, fill: u8, border: Stroke| {
        wv.bg_fill = grey(fill);
        wv.weak_bg_fill = grey(fill);
        wv.bg_stroke = border;
        wv.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    };
    style(&mut w.inactive, 16, Stroke::new(1.0, Color32::WHITE));
    style(&mut w.hovered, 40, Stroke::new(2.0, YELLOW));
    style(&mut w.active, 64, Stroke::new(2.0, YELLOW));
    style(&mut w.open, 32, Stroke::new(1.0, YELLOW));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG contrast ratio between two colours.
    fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let ch = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
        };
        let (hi, lo) = if lum(a) > lum(b) { (lum(a), lum(b)) } else { (lum(b), lum(a)) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// High contrast meets WCAG AAA (7:1) for its text, and the error and
    /// link colours still pass AA.
    #[test]
    fn high_contrast_is_high_contrast() {
        let v = high_contrast();
        let bg = v.panel_fill;
        assert!(contrast(v.widgets.noninteractive.fg_stroke.color, bg) >= 7.0);
        assert!(contrast(v.weak_text_color.unwrap(), bg) >= 7.0);
        assert!(contrast(v.weak_text_color.unwrap(), v.faint_bg_color) >= 7.0, "striped rows");
        assert!(contrast(v.selection.stroke.color, v.selection.bg_fill) >= 7.0, "selected text");
        assert!(contrast(v.selection.stroke.color, v.text_edit_bg_color.unwrap()) >= 7.0, "focused field outline");
        assert!(contrast(v.widgets.inactive.fg_stroke.color, v.widgets.inactive.bg_fill) >= 7.0);
        assert!(contrast(v.widgets.active.fg_stroke.color, v.widgets.active.bg_fill) >= 7.0);
        assert!(contrast(v.warn_fg_color, bg) >= 7.0);
        assert!(contrast(v.error_fg_color, bg) >= 4.5);
        assert!(contrast(v.hyperlink_color, bg) >= 4.5);
        assert!(contrast(chart_colour(Appearance::HighContrast, &v), bg) >= 7.0, "chart bars");
    }
}
