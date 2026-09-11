//! 逻辑层：处理配置读写、环境变量清单、进程启动。
//!
//! 硬性约束：本层禁止 `use gtk` / `use adw` / `use glib`，不持有任何控件引用，
//! 输入输出均为纯数据，可脱离显示器直接单测。

pub mod catalog;
pub mod config;
pub mod runner;
