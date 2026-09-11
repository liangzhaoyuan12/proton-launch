//! 展示层 · 可复用复合控件（图标按钮、文件选择、环境变量行）。

use std::rc::Rc;

use gtk::prelude::*;
use adw::prelude::*;

/// 扁平图标按钮（行尾操作统一用它）。
pub fn icon_button(icon_name: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon_name)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .build();
    button.add_css_class("flat");
    button
}

/// 控件所属窗口（作为文件对话框的父窗口）。
fn parent_window(widget: &impl IsA<gtk::Widget>) -> Option<gtk::Window> {
    widget.root().and_then(|r| r.downcast::<gtk::Window>().ok())
}

/// 弹出文件 / 目录选择框，选中后写回 `row`（`set_text` 会触发 changed 信号）。
pub fn pick_path(row: &adw::EntryRow, title: &str, folder: bool, on_done: Rc<dyn Fn()>) {
    let dialog = gtk::FileDialog::builder().title(title).modal(true).build();
    let parent = parent_window(row);
    let target = row.clone();
    let keep_alive = dialog.clone();
    if folder {
        dialog.select_folder(
            parent.as_ref(),
            None::<&gtk::gio::Cancellable>,
            move |res| {
                let _ = &keep_alive;
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        target.set_text(&path.display().to_string());
                        on_done();
                    }
                }
            },
        );
    } else {
        dialog.open(parent.as_ref(), None::<&gtk::gio::Cancellable>, move |res| {
            let _ = &keep_alive;
            if let Ok(file) = res {
                if let Some(path) = file.path() {
                    target.set_text(&path.display().to_string());
                    on_done();
                }
            }
        });
    }
}

/// 开关型环境变量行：勾选即写入 "1"。
pub fn bool_env_row(label: &str, key: &str, active: bool) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder().title(label).subtitle(key).build();
    row.set_tooltip_text(Some(key));
    row.set_active(active);
    row
}

/// 文本型环境变量行：外层开关表示「是否写入该变量」，内层输入值。
///
/// `browse` 为 `Some((提示文本, 是否目录))` 时在值行尾追加浏览按钮。
pub fn text_env_row(
    label: &str,
    key: &str,
    value: &str,
    browse: Option<(&'static str, bool)>,
    on_done: Rc<dyn Fn()>,
) -> (adw::ExpanderRow, adw::EntryRow) {
    let entry = adw::EntryRow::builder().title("值").build();
    entry.set_text(value);
    entry.set_tooltip_text(Some(key));

    if let Some((tooltip, folder)) = browse {
        let icon = if folder {
            "folder-open-symbolic"
        } else {
            "document-open-symbolic"
        };
        let button = icon_button(icon, tooltip);
        let target = entry.clone();
        button.connect_clicked(move |_| {
            pick_path(&target, tooltip, folder, on_done.clone());
        });
        entry.add_suffix(&button);
    }

    let row = adw::ExpanderRow::builder()
        .title(label)
        .subtitle(key)
        .show_enable_switch(true)
        .build();
    row.add_row(&entry);
    let filled = !value.is_empty();
    row.set_enable_expansion(filled);
    row.set_expanded(filled);
    (row, entry)
}
