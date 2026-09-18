//! 数据层：纯数据结构，不依赖任何 UI 框架。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

/// 全局自增计数器，配合时间戳生成唯一 ID。
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 生成一个在本应用内唯一的 ID。
pub fn generate_id() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let seq = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{ts:016x}{seq:04x}")
}

/// 单个游戏的启动配置。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GameConfig {
    /// 稳定唯一标识（P0-6: 替代数组下标做主键）
    #[serde(default = "generate_id")]
    pub id: String,
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
    /// 是否启用 MangoHud 性能监控 overlay
    #[serde(default)]
    pub enable_mango_hud: bool,
    /// MangoHud 配置项（逗号分隔，如 "fps,cpu,gpu,ram"）
    #[serde(default)]
    pub mango_hud_config: String,
}

/// 实现 Default 以兼容 serde 的 `#[serde(default)]`。
impl Default for GameConfig {
    fn default() -> Self {
        Self {
            id: generate_id(),
            name: String::new(),
            executable: String::new(),
            args: String::new(),
            work_dir: String::new(),
            renderer: String::new(),
            env_vars: HashMap::new(),
            enable_mango_hud: false,
            mango_hud_config: String::new(),
        }
    }
}

impl GameConfig {
    pub fn new(name: &str) -> Self {
        Self {
            id: generate_id(),
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// 侧边栏展示名称（未命名时回退到"未命名"）。
    pub fn display_name(&self) -> &str {
        if self.name.trim().is_empty() {
            "未命名"
        } else {
            &self.name
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
