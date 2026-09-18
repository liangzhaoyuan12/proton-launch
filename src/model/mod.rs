//! 数据层：纯数据结构，不依赖任何 UI 框架。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 单个游戏的启动配置。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct GameConfig {
    /// 展示名称
    #[serde(default)]
    pub name: String,
    /// 可执行文件路径
    #[serde(default)]
    pub executable: String,
    /// 命令行参数（按空白拆分后逐个传给 umu-run）
    #[serde(default)]
    pub args: String,
    /// 工作目录（留空则取可执行文件所在目录）
    #[serde(default)]
    pub work_dir: String,
    /// 渲染器启动参数（"" / -dx11 / -dx12 / -opengl / -vulkan）
    #[serde(default)]
    pub renderer: String,
    /// 传给游戏的环境变量
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
}

impl GameConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// 侧边栏展示名称（未命名时回退到编号）。
    pub fn display_name(&self, idx: usize) -> String {
        if self.name.trim().is_empty() {
            format!("未命名 #{}", idx + 1)
        } else {
            self.name.clone()
        }
    }

    /// 侧边栏副标题：可执行文件信息。
    pub fn subtitle(&self) -> String {
        let exe = self.executable.trim();
        if exe.is_empty() {
            "未设置可执行文件".to_string()
        } else {
            exe.to_string()
        }
    }
}
