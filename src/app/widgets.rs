//! The controls the tabs are built from, drawn so screen readers and the
//! keyboard work with them, and the window's fonts and colours.

use super::*;

// ----- fonts and colours ---------------------------------------------------

/// Font family used for the tab labels (Google Sans Bold).
pub(super) fn tab_font() -> egui::FontFamily {
    egui::FontFamily::Name("tab".into())
}

/// Google Sans (SIL Open Font License, see assets/fonts/OFL.txt): Medium for
/// all body text, Bold for the tabs. egui's built-in fonts stay behind them
/// as fallbacks for symbols and scripts Google Sans does not cover.
fn setup_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};
    use std::sync::Arc;

    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "GoogleSans-Medium".into(),
        Arc::new(FontData::from_static(include_bytes!("../../assets/fonts/GoogleSans-Medium.ttf"))),
    );
    fonts.font_data.insert(
        "GoogleSans-Bold".into(),
        Arc::new(FontData::from_static(include_bytes!("../../assets/fonts/GoogleSans-Bold.ttf"))),
    );
    let fallbacks = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "GoogleSans-Medium".into());
    let mut tab = vec!["GoogleSans-Bold".to_owned()];
    tab.extend(fallbacks);
    fonts.families.insert(tab_font(), tab);
    ctx.set_fonts(fonts);
}

pub(super) fn setup_style(ctx: &egui::Context) {
    use egui::{FontId, TextStyle};
    setup_fonts(ctx);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(24.0)),
            (TextStyle::Body, FontId::proportional(17.0)),
            (TextStyle::Button, FontId::proportional(17.0)),
            (TextStyle::Monospace, FontId::monospace(16.0)),
            (TextStyle::Small, FontId::proportional(14.0)),
        ]
        .into();
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.interact_size.y = CONTROL_HEIGHT;
    });
}

pub(super) fn error_color(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode { Color32::from_rgb(255, 140, 140) } else { Color32::from_rgb(170, 0, 0) }
}

/// Spending within budget: green with enough contrast on either theme.
pub(super) fn good_color(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode { Color32::from_rgb(110, 200, 130) } else { Color32::from_rgb(20, 120, 50) }
}

// ----- controls ------------------------------------------------------------

/// A heading that screen readers list as a heading, so users can jump to it.
pub(super) fn heading(ui: &mut Ui, text: &str) {
    let resp = ui.heading(text);
    ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_role(Role::Heading);
        node.set_level(2);
        // egui stores a plain label's text as its value; headings need a name.
        node.set_label(text);
    });
}

pub(super) fn full_button(ui: &mut Ui, text: &str, enabled: bool) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| ui.add_sized([ui.available_width(), CONTROL_HEIGHT], Button::new(text)))
        .inner
}

/// Buttons side by side, sharing the width. Returns the one pressed.
pub(super) fn button_row(ui: &mut Ui, buttons: &[(&str, bool)]) -> Option<usize> {
    let mut pressed = None;
    ui.columns(buttons.len(), |cols| {
        for (i, (col, (text, enabled))) in cols.iter_mut().zip(buttons).enumerate() {
            if full_button(col, text, *enabled).clicked() {
                pressed = Some(i);
            }
        }
    });
    pressed
}

/// A text box with its label on the same line, filling the rest of the
/// width, with a fixed `id` so focus can be sent to it.
pub(super) fn text_field(ui: &mut Ui, id: Id, label: &str, value: &mut String, hint: &str) -> egui::Response {
    ui.horizontal(|ui| {
        let label = ui.label(label);
        let edit = egui::TextEdit::singleline(value).id(id).hint_text(hint).desired_width(f32::INFINITY);
        ui.add_sized([ui.available_width(), CONTROL_HEIGHT], edit).labelled_by(label.id)
    })
    .inner
}

/// A text box for a password: as `text_field`, with what is typed hidden.
pub(super) fn password_field(ui: &mut Ui, id: Id, label: &str, value: &mut String) -> egui::Response {
    ui.horizontal(|ui| {
        let label = ui.label(label);
        let edit = egui::TextEdit::singleline(value).id(id).password(true).desired_width(f32::INFINITY);
        ui.add_sized([ui.available_width(), CONTROL_HEIGHT], edit).labelled_by(label.id)
    })
    .inner
}

/// A read-only text box, labelled on the same line, that a screen reader can
/// still focus and review.
pub(super) fn read_only_field(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        let label = ui.label(label);
        let mut text = value;
        let edit = egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY);
        let resp = ui.add_sized([ui.available_width(), CONTROL_HEIGHT], edit).labelled_by(label.id);
        ui.ctx().accesskit_node_builder(resp.id, |node| node.set_read_only());
    });
}

/// Text that wraps, such as an explanation under a heading.
pub(super) fn note(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(text).wrap());
}

/// A labelled, full-width dropdown. While it has focus and is closed, the Up,
/// Down, Home and End keys change the choice directly, as in a native combo
/// box; Space, Enter or Alt+Down (Option+Down on a Mac) opens the list, and
/// Alt+Up or Escape closes it.
pub(super) fn dropdown(ui: &mut Ui, id_salt: &str, label: &str, options: &[String], selected: &mut usize, enabled: bool) -> bool {
    let label_resp = ui.label(label);
    let mut changed = false;
    let current = options.get(*selected).cloned().unwrap_or_default();
    let inner = ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(id_salt)
            .width(ui.available_width())
            .height(400.0)
            .selected_text(current.clone())
            .show_ui(ui, |ui| {
                for (i, option) in options.iter().enumerate() {
                    let resp = ui.add_sized(
                        [ui.available_width(), CONTROL_HEIGHT],
                        Button::selectable(i == *selected, option.as_str()),
                    );
                    if resp.clicked() {
                        *selected = i;
                        changed = true;
                    }
                }
            })
    });
    let combo = inner.inner;
    let resp = combo.response.labelled_by(label_resp.id);
    let popup_open = combo.inner.is_some();
    // Where egui keeps whether this dropdown's list is open.
    let popup_id = resp.id.with("popup");

    if enabled && popup_open && ui.input_mut(|i| consume_exact(i, Modifiers::ALT, Key::ArrowUp)) {
        egui::Popup::close_id(ui.ctx(), popup_id);
    }
    if enabled && resp.has_focus() && !popup_open && !options.is_empty() {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(resp.id, EventFilter { vertical_arrows: true, ..Default::default() })
        });
        if ui.input_mut(|i| consume_exact(i, Modifiers::ALT, Key::ArrowDown)) {
            egui::Popup::open_id(ui.ctx(), popup_id);
        }
        let last = options.len() - 1;
        let new = ui.input_mut(|i| {
            if consume_exact(i, Modifiers::NONE, Key::ArrowDown) {
                Some((*selected + 1).min(last))
            } else if consume_exact(i, Modifiers::NONE, Key::ArrowUp) {
                Some(selected.saturating_sub(1))
            } else if consume_exact(i, Modifiers::NONE, Key::Home) {
                Some(0)
            } else if consume_exact(i, Modifiers::NONE, Key::End) {
                Some(last)
            } else {
                None
            }
        });
        if let Some(new) = new
            && new != *selected
        {
            *selected = new;
            changed = true;
            ui.ctx().request_repaint();
        }
    }
    let value = options.get(*selected).cloned().unwrap_or_default();
    ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_role(Role::ComboBox);
        node.set_value(value);
    });
    changed
}

/// Takes a press of `key` with exactly `modifiers` out of the input. egui's
/// own `consume_key` ignores extra Shift and Alt, so Alt+Down would count as
/// Down.
pub(super) fn consume_exact(input: &mut egui::InputState, modifiers: Modifiers, key: Key) -> bool {
    let before = input.events.len();
    input.events.retain(|e| {
        !matches!(e, egui::Event::Key { key: k, pressed: true, modifiers: m, .. } if *k == key && m.matches_exact(modifiers))
    });
    input.events.len() != before
}

// ----- the list of entries -------------------------------------------------

/// The columns of an entry as drawn in the list.
pub(super) struct RowText<'a> {
    pub date: &'a str,
    pub name: &'a str,
    pub category: &'a str,
    pub price: &'a str,
    /// What a screen reader says for the row.
    pub spoken: &'a str,
}

/// One entry in the list: a full-width row that can be focused and pressed,
/// with the date, name, category and amount in columns. Screen readers get
/// it as a list option, selected or not, named with the whole entry.
pub(super) fn entry_row(ui: &mut Ui, id: Id, text: &RowText<'_>, selected: bool) -> egui::Response {
    // Laid out with an automatic id, but pressed and focused with `id`, so
    // focus can be sent to a row by its entry's line number.
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), CONTROL_HEIGHT), egui::Sense::hover());
    let resp = ui.interact(rect, id, egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, text.spoken));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&resp, selected);
        if selected || resp.hovered() || resp.has_focus() {
            ui.painter().rect_filled(rect, 4.0, visuals.weak_bg_fill);
        }
        let colour = visuals.text_color();
        let inner = rect.shrink2(egui::vec2(8.0, 0.0));
        let width = inner.width();
        // Date, name and category from the left; the amount on the right.
        let columns = [(0.0, 0.22, text.date), (0.22, 0.42, text.name), (0.64, 0.2, text.category)];
        let painter = ui.painter().with_clip_rect(rect);
        for (start, share, value) in columns {
            let galley = egui::WidgetText::from(value).into_galley(
                ui,
                Some(egui::TextWrapMode::Truncate),
                width * share - 8.0,
                egui::TextStyle::Body,
            );
            let pos = egui::pos2(inner.left() + width * start, rect.center().y - galley.size().y / 2.0);
            painter.galley(pos, galley, colour);
        }
        let font = egui::TextStyle::Body.resolve(ui.style());
        painter.text(egui::pos2(inner.right(), rect.center().y), egui::Align2::RIGHT_CENTER, text.price, font, colour);
    }
    ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_role(Role::ListBoxOption);
        node.set_label(text.spoken);
        node.set_selected(selected);
    });
    resp
}

// ----- charts ----------------------------------------------------------------

/// A category's spending this month as a labelled bar: the name and amounts
/// above, the bar below. With a budget, the bar's length is the budget and
/// it fills as money is spent, turning red when over. Without one, bars are
/// scaled to the largest category's spending. It can be focused with Tab,
/// and screen readers read `spoken`.
pub(super) fn spending_bar(
    ui: &mut Ui,
    name: &str,
    amounts: &str,
    fraction: f32,
    over: bool,
    has_budget: bool,
    spoken: &str,
) {
    let height = 56.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::focusable_noninteractive());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, spoken));
    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        let painter = ui.painter_at(rect);
        let font = egui::TextStyle::Body.resolve(ui.style());
        let text_y = rect.top() + 12.0;
        painter.text(egui::pos2(rect.left() + 2.0, text_y), egui::Align2::LEFT_CENTER, name, font.clone(), visuals.text_color());
        let amount_colour = if over { error_color(ui) } else { visuals.text_color() };
        painter.text(egui::pos2(rect.right() - 2.0, text_y), egui::Align2::RIGHT_CENTER, amounts, font, amount_colour);
        let track = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 2.0, rect.top() + 28.0),
            egui::pos2(rect.right() - 2.0, rect.top() + 46.0),
        );
        painter.rect_filled(track, 4.0, visuals.extreme_bg_color);
        painter.rect_stroke(track, 4.0, egui::Stroke::new(1.0, visuals.weak_text_color()), egui::StrokeKind::Inside);
        let fill = if over {
            error_color(ui)
        } else if has_budget {
            good_color(ui)
        } else {
            visuals.selection.bg_fill
        };
        let filled = egui::Rect::from_min_size(track.min, egui::vec2(track.width() * fraction.clamp(0.0, 1.0), track.height()));
        if filled.width() > 0.5 {
            painter.rect_filled(filled, 4.0, fill);
        }
    }
}

/// A column chart of the totals for several months, with each month's name
/// beneath its column (every few months' when there are many) and
/// `highlight` drawn in the selection colour. Screen readers get it as an
/// image named `description`.
pub(super) fn month_chart(ui: &mut Ui, months: &[(String, i64)], highlight: Option<usize>, top_label: &str, description: &str) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 220.0), egui::Sense::focusable_noninteractive());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, description));
    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, visuals.extreme_bg_color);
        let small = egui::TextStyle::Small.resolve(ui.style());
        let plot = egui::Rect::from_min_max(rect.min + egui::vec2(8.0, 24.0), rect.max - egui::vec2(8.0, 24.0));
        let largest = months.iter().map(|(_, t)| *t).max().unwrap_or(0).max(1);
        painter.text(rect.left_top() + egui::vec2(8.0, 4.0), egui::Align2::LEFT_TOP, top_label, small.clone(), visuals.weak_text_color());
        let baseline = [egui::pos2(plot.left(), plot.bottom()), egui::pos2(plot.right(), plot.bottom())];
        painter.line_segment(baseline, egui::Stroke::new(1.0, visuals.weak_text_color()));
        let slot = plot.width() / months.len().max(1) as f32;
        // With many months, only every few are named, so the names don't run
        // into each other.
        let named_every = (40.0 / slot).ceil().max(1.0) as usize;
        let last = months.len().saturating_sub(1);
        for (i, (label, total)) in months.iter().enumerate() {
            let x = plot.left() + slot * (i as f32 + 0.5);
            let height = plot.height() * (*total).max(0) as f32 / largest as f32;
            let column = egui::Rect::from_min_max(
                egui::pos2(x - slot * 0.32, plot.bottom() - height),
                egui::pos2(x + slot * 0.32, plot.bottom()),
            );
            let colour = if Some(i) == highlight { visuals.selection.bg_fill } else { visuals.weak_text_color() };
            painter.rect_filled(column, 2.0, colour);
            let text_colour = if Some(i) == highlight { visuals.strong_text_color() } else { visuals.text_color() };
            if (last - i).is_multiple_of(named_every) {
                painter.text(egui::pos2(x, rect.bottom() - 4.0), egui::Align2::CENTER_BOTTOM, label, small.clone(), text_colour);
            }
        }
    }
    ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_role(Role::Image);
        node.set_label(description);
    });
}

/// Draws a thick, high-contrast outline around the focused control.
pub(super) fn draw_focus_ring(ctx: &egui::Context) {
    let Some(id) = ctx.memory(|m| m.focused()) else { return };
    let Some(resp) = ctx.read_response(id) else { return };
    let color = if ctx.global_style().visuals.dark_mode {
        Color32::from_rgb(255, 215, 0)
    } else {
        Color32::from_rgb(0, 70, 200)
    };
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("focus_ring")));
    painter.rect_stroke(resp.rect.expand(3.0), 4.0, egui::Stroke::new(3.0, color), egui::StrokeKind::Outside);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, RawInput};

    fn press(key: Key, modifiers: Modifiers) -> Event {
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    /// Runs one frame of `ui_fn` with `events` as the input.
    fn frame(ctx: &egui::Context, events: Vec<Event>, ui_fn: &mut impl FnMut(&mut Ui)) {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let input = RawInput { events, screen_rect: Some(screen), ..Default::default() };
        let mut output = ctx.run_ui(input, |ui| ui_fn(ui));
        output.textures_delta.clear();
    }

    fn focused(ctx: &egui::Context) -> Option<Id> {
        ctx.memory(|m| m.focused())
    }

    fn tab(ctx: &egui::Context, ui_fn: &mut impl FnMut(&mut Ui)) {
        frame(ctx, vec![press(Key::Tab, Modifiers::NONE)], ui_fn);
        frame(ctx, vec![], ui_fn);
    }

    #[test]
    fn dropdown_keys() {
        let ctx = egui::Context::default();
        let options: Vec<String> = ["One", "Two", "Three"].map(String::from).to_vec();
        let mut selected = 0;
        let mut changes = 0;
        let mut ui_fn = |ui: &mut Ui| {
            if dropdown(ui, "test", "Number", &options, &mut selected, true) {
                changes += 1;
            }
        };
        frame(&ctx, vec![], &mut ui_fn);
        tab(&ctx, &mut ui_fn);
        assert!(focused(&ctx).is_some(), "Tab moves focus to the dropdown");
        frame(&ctx, vec![press(Key::ArrowDown, Modifiers::NONE)], &mut ui_fn);
        frame(&ctx, vec![press(Key::End, Modifiers::NONE)], &mut ui_fn);
        frame(&ctx, vec![press(Key::ArrowUp, Modifiers::NONE)], &mut ui_fn);
        frame(&ctx, vec![press(Key::ArrowDown, Modifiers::SHIFT)], &mut ui_fn);
        assert_eq!((selected, changes), (1, 3));
    }

    #[test]
    fn rows_and_bars_take_focus_from_tab() {
        let ctx = egui::Context::default();
        let row = RowText { date: "Fri 30 Oct 2026", name: "Lunch", category: "Food", price: "£8.50", spoken: "Lunch" };
        let row_id = Id::new("row");
        let mut ui_fn = |ui: &mut Ui| {
            entry_row(ui, row_id, &row, false);
            spending_bar(ui, "Food", "£8.50", 0.5, false, true, "Food: £8.50");
        };
        frame(&ctx, vec![], &mut ui_fn);
        tab(&ctx, &mut ui_fn);
        assert_eq!(focused(&ctx), Some(row_id));
        tab(&ctx, &mut ui_fn);
        assert!(focused(&ctx).is_some_and(|id| id != row_id), "the bar can be focused too");
    }
}
