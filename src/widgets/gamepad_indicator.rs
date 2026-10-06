//! 展示层 · 手柄状态指示器：状态栏右下角计数 + 点击展开的详情卡片。
//!
//! 只做展示：数据来自 `utils::gamepad::GamepadInfo`，何时刷新、何时弹通知由
//! `app` 层决定。

use std::rc::Rc;

use adw::prelude::*;

use crate::utils::gamepad::GamepadInfo;

/// 右下角的手柄指示器（按钮 + 弹出卡片）。
pub struct GamepadIndicator {
    button: gtk::MenuButton,
    count: gtk::Label,
    header: gtk::Label,
    list: gtk::Box,
    empty: gtk::Label,
}

impl GamepadIndicator {
    /// 构建指示器；`button` 放进状态栏，卡片在点击时弹出。
    pub fn new() -> Rc<Self> {
        // ── 状态栏按钮：图标 + 计数 ─────────────────────────────────────
        let icon = gtk::Image::from_icon_name("input-gamepad-symbolic");
        let count = gtk::Label::new(Some("未连接手柄"));
        count.add_css_class("dim-label");

        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        inner.append(&icon);
        inner.append(&count);

        let button = gtk::MenuButton::builder()
            .child(&inner)
            .tooltip_text("已连接的手柄，点击查看详细信息")
            .build();
        button.add_css_class("flat");

        // ── 弹出卡片 ────────────────────────────────────────────────────
        let header = gtk::Label::new(Some("已连接的手柄（0）"));
        header.add_css_class("heading");
        header.set_xalign(0.0);
        header.set_hexpand(true);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 6);

        let empty = gtk::Label::new(Some("当前没有已连接的手柄"));
        empty.add_css_class("dim-label");
        empty.set_halign(gtk::Align::Center);
        empty.set_margin_top(8);
        empty.set_margin_bottom(8);

        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);

        let hint = gtk::Label::new(Some(
            "软件运行期间插入手柄会弹出系统通知；关闭软件期间插入手柄不会弹通知，\
             下次打开时会直接显示在这里。",
        ));
        hint.add_css_class("dim-label");
        hint.add_css_class("caption");
        hint.set_wrap(true);
        hint.set_xalign(0.0);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.append(&header);
        content.append(&list);
        content.append(&empty);
        content.append(&separator);
        content.append(&hint);

        let scrolled = gtk::ScrolledWindow::builder()
            .min_content_width(480)
            .max_content_height(440)
            .propagate_natural_height(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&content)
            .build();
        // 保证卡片宽度（Popover 只按子控件自然宽度定尺寸时也能到 480）
        scrolled.set_size_request(480, -1);

        let popover = gtk::Popover::new();
        popover.set_child(Some(&scrolled));
        button.set_popover(Some(&popover));

        Rc::new(Self {
            button,
            count,
            header,
            list,
            empty,
        })
    }

    /// 状态栏按钮（交给 `window` 摆到右下角）。
    pub fn button(&self) -> &gtk::MenuButton {
        &self.button
    }

    /// 用最新手柄列表刷新计数与卡片。
    pub fn update(&self, pads: &[GamepadInfo]) {
        let total = pads.len();
        let text = if total == 0 {
            "未连接手柄".to_string()
        } else {
            format!("已连接 {total} 个手柄")
        };
        self.count.set_text(&text);
        self.header.set_text(&format!("已连接的手柄（{total}）"));
        self.empty.set_visible(total == 0);

        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        for pad in pads {
            self.list.append(&device_row(pad));
        }
    }
}

/// 单个手柄的可展开卡片：标题为名称，展开后是完整字段。
fn device_row(pad: &GamepadInfo) -> adw::ExpanderRow {
    let row = adw::ExpanderRow::builder()
        .title(&pad.name)
        .subtitle(format!("{} · {}", pad.protocol.label(), pad.event_node))
        .enable_expansion(true)
        .build();
    row.add_prefix(&gtk::Image::from_icon_name("input-gamepad-symbolic"));
    for detail in detail_rows(pad) {
        row.add_row(&detail);
    }
    row
}

/// 卡片里每个字段渲染成的行（独立出来便于单测读取 title / subtitle）。
fn detail_rows(pad: &GamepadInfo) -> Vec<adw::ActionRow> {
    details(pad)
        .into_iter()
        .map(|(key, value)| {
            adw::ActionRow::builder()
                .title(key)
                .subtitle(value)
                .subtitle_lines(2)
                .build()
        })
        .collect()
}

/// 一张卡片里要展示的全部字段。
fn details(pad: &GamepadInfo) -> Vec<(&'static str, String)> {
    let or_none = |value: &str| {
        if value.is_empty() {
            "无".to_string()
        } else {
            value.to_string()
        }
    };
    vec![
        ("协议", pad.protocol.label().to_string()),
        ("协议标识", pad.protocol.code().to_string()),
        ("设备节点", pad.event_node.clone()),
        (
            "字符设备",
            pad.js_node
                .clone()
                .unwrap_or_else(|| "未生成（内核 joydev 未创建 js 节点）".to_string()),
        ),
        ("厂商:产品 ID", pad.vendor_product()),
        ("输入 ID", pad.input_id()),
        ("连接总线", pad.bus_label()),
        (
            "内核驱动",
            pad.driver.clone().unwrap_or_else(|| "未知".to_string()),
        ),
        ("串号 / MAC", or_none(&pad.uniq)),
        ("物理路径", or_none(&pad.phys)),
        ("能力", pad.capabilities()),
        ("系统路径", or_none(&pad.sysfs)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::gamepad::parse;

    /// 一个有 js 节点、有震动的 Xbox One，一个没有 js 节点、无 sysfs 的 DualSense。
    const FIXTURE: &str = r#"
I: Bus=0003 Vendor=045e Product=02dd Version=0114
N: Name="Microsoft X-Box One pad"
P: Phys=usb-0000:00:14.0-3/input0
S: Sysfs=/devices/virtual/proton-launch-test/input/input30
U: Uniq=
H: Handlers=js1 event30
B: PROP=0
B: EV=30001f
B: KEY=7fff000000000000 0 0 0 0
B: ABS=3003f

I: Bus=0005 Vendor=054c Product=0ce6 Version=8111
N: Name="Sony Interactive Entertainment DualSense Wireless Controller"
P: Phys=
S: Sysfs=
U: Uniq=ac:83:e3:aa:bb:cc
H: Handlers=event50
B: PROP=0
B: EV=10001f
B: KEY=7fff000000000000 0 0 0 0
B: ABS=3003f
"#;

    fn field(rows: &[(&'static str, String)], key: &str) -> String {
        rows.iter()
            .find(|(name, _)| *name == key)
            .unwrap_or_else(|| panic!("卡片缺少字段：{key}"))
            .1
            .clone()
    }

    #[test]
    fn 卡片字段齐全且取值正确() {
        let pads = parse(FIXTURE);
        assert_eq!(pads.len(), 2);
        // 按名称排序：Microsoft … 在 Sony … 之前
        let rows = details(&pads[0]);
        assert_eq!(rows.len(), 12, "卡片共 12 个字段");
        assert_eq!(field(&rows, "协议"), "Xbox One");
        assert_eq!(field(&rows, "协议标识"), "xbox-one");
        assert_eq!(field(&rows, "设备节点"), "/dev/input/event30");
        assert_eq!(field(&rows, "字符设备"), "/dev/input/js1");
        assert_eq!(field(&rows, "厂商:产品 ID"), "045e:02dd");
        assert_eq!(
            field(&rows, "输入 ID"),
            "bus=0x0003 vendor=0x045e product=0x02dd version=0x0114"
        );
        assert_eq!(field(&rows, "连接总线"), "USB（0x0003）");
        assert_eq!(
            field(&rows, "系统路径"),
            "/devices/virtual/proton-launch-test/input/input30"
        );
        assert_eq!(field(&rows, "能力"), "按键 15 个 · 轴 8 个 · 力反馈支持");
    }

    #[test]
    fn 缺失的信息显示为无或未知() {
        let pads = parse(FIXTURE);
        let rows = details(&pads[1]);
        assert_eq!(field(&rows, "协议"), "PS5（DualSense）");
        assert_eq!(field(&rows, "协议标识"), "ps5");
        assert_eq!(
            field(&rows, "字符设备"),
            "未生成（内核 joydev 未创建 js 节点）"
        );
        assert_eq!(field(&rows, "串号 / MAC"), "ac:83:e3:aa:bb:cc");
        assert_eq!(field(&rows, "物理路径"), "无");
        assert_eq!(field(&rows, "系统路径"), "无");
        assert_eq!(field(&rows, "内核驱动"), "未知");
        assert_eq!(field(&rows, "能力"), "按键 15 个 · 轴 8 个 · 力反馈不支持");
        assert_eq!(field(&rows, "连接总线"), "蓝牙（0x0005）");
    }

    /// 需要图形显示环境：`cargo test -- --ignored` 单独跑。
    #[test]
    #[ignore = "需要可用的显示环境（DISPLAY/Wayland）"]
    fn 指示器渲染计数与手柄行() {
        adw::init().expect("初始化 libadwaita 失败");
        let pads = parse(FIXTURE);
        let indicator = GamepadIndicator::new();
        indicator.update(&pads);
        assert_eq!(indicator.count.text(), "已连接 2 个手柄");
        assert_eq!(indicator.header.text(), "已连接的手柄（2）");
        assert!(!indicator.empty.get_visible());

        let mut rows = 0;
        let mut child = indicator.list.first_child();
        while let Some(widget) = child {
            let next = widget.next_sibling();
            let row: adw::ExpanderRow = widget
                .downcast()
                .expect("卡片列表的直接子项必须是 ExpanderRow");
            assert!(!row.title().is_empty(), "手柄行必须有名称");
            assert!(row.subtitle().contains("·"), "副标题应含协议与节点");
            rows += 1;
            child = next;
        }
        assert_eq!(rows, 2, "两个手柄各一行");

        // 展开后的字段行：标题与取值都要落到控件上
        let xbox_rows = detail_rows(&pads[0]);
        assert_eq!(xbox_rows.len(), 12);
        assert_eq!(xbox_rows[0].title(), "协议");
        assert_eq!(xbox_rows[0].subtitle().as_deref(), Some("Xbox One"));
        assert_eq!(xbox_rows[4].title(), "厂商:产品 ID");
        assert_eq!(xbox_rows[4].subtitle().as_deref(), Some("045e:02dd"));
        assert_eq!(xbox_rows[11].title(), "系统路径");
        assert_eq!(
            xbox_rows[11].subtitle().as_deref(),
            Some("/devices/virtual/proton-launch-test/input/input30")
        );

        indicator.update(&[]);
        assert_eq!(indicator.count.text(), "未连接手柄");
        assert_eq!(indicator.header.text(), "已连接的手柄（0）");
        assert!(indicator.empty.get_visible(), "空状态文案应恢复显示");
        assert!(indicator.list.first_child().is_none(), "空列表不残留旧条目");
    }
}
