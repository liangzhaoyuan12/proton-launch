//! 通过内嵌的 umu-run 启动游戏。

use std::collections::HashMap;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command};

use crate::model::GameConfig;
use crate::utils::config::ConfigStore;

const UMU_RUN: &[u8] = include_bytes!("../../umu-run");

pub struct Runner {
    umu_path: PathBuf,
    python_check: Option<bool>,
    python_cmd: Option<String>,
}

impl Runner {
    pub fn new() -> Self {
        let umu_path = ConfigStore::data_dir().join("umu-run");
        Runner {
            umu_path,
            python_check: None,
            python_cmd: None,
        }
    }

    /// 首次运行时把内嵌的 umu-run 释放到数据目录。
    /// P2-3: 版本检查 — 内嵌内容变化时覆盖旧文件。
    pub fn extract_umu(&self) -> Result<(), String> {
        let version_path = self.umu_path.with_extension("version");
        let current_ver = UMU_RUN.len().to_string();
        // 比较版本戳：一致则跳过
        if let Ok(ver) = fs::read_to_string(&version_path)
            && ver.trim() == current_ver
        {
            return Ok(());
        }
        let parent = self.umu_path.parent().ok_or("无效的 umu-run 路径")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        fs::write(&self.umu_path, UMU_RUN).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.umu_path, fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        // 写入版本戳
        let _ = fs::write(&version_path, &current_ver);
        Ok(())
    }

    /// 是否有可用的 python3（umu-run 是 python zipapp）。
    /// P1-13: 缓存实际探测到的解释器路径。
    pub fn is_python_available(&mut self) -> bool {
        if let Some(ok) = self.python_check {
            return ok;
        }
        // 先尝试 python3
        let ok = Command::new("python3")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            self.python_check = Some(true);
            self.python_cmd = Some("python3".to_string());
            return true;
        }
        // 再尝试 python
        let ok = Command::new("python")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            self.python_check = Some(true);
            self.python_cmd = Some("python".to_string());
            return true;
        }
        self.python_check = Some(false);
        false
    }

    /// 启动游戏，返回子进程句柄（stdout/stderr 已接管道）。
    pub fn run_game(&mut self, config: &GameConfig) -> Result<Child, String> {
        self.extract_umu()?;
        if !self.umu_path.exists() {
            return Err("umu-run 释放失败".to_string());
        }

        let mut envs: HashMap<String, String> = HashMap::new();
        // P1-12: 空字符串回退默认值
        envs.insert(
            "GAMEID".to_string(),
            config
                .env_vars
                .get("GAMEID")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or("umu-default")
                .to_string(),
        );
        envs.insert(
            "STORE".to_string(),
            config
                .env_vars
                .get("STORE")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or("none")
                .to_string(),
        );
        for (k, v) in &config.env_vars {
            if k != "GAMEID" && k != "STORE" {
                envs.insert(k.clone(), v.clone());
            }
        }

        // P1-13: 使用实际探测到的解释器
        let mut cmd = if self.is_python_available() {
            let python_cmd = self.python_cmd.as_deref().unwrap_or("python3");
            let mut c = Command::new(python_cmd);
            c.arg(&self.umu_path);
            c
        } else {
            Command::new(&self.umu_path)
        };

        if !config.executable.trim().is_empty() {
            cmd.arg(config.executable.trim());
        }
        if !config.renderer.trim().is_empty() {
            cmd.arg(config.renderer.trim());
        }
        // P1-14: 支持引号的参数分词
        for arg in split_args(&config.args) {
            cmd.arg(arg);
        }

        if !config.work_dir.trim().is_empty() {
            cmd.current_dir(config.work_dir.trim());
        } else if !config.executable.trim().is_empty() {
            // P2-15: 空 parent 时跳过 current_dir
            if let Some(parent) = std::path::Path::new(config.executable.trim()).parent()
                && !parent.as_os_str().is_empty()
            {
                cmd.current_dir(parent);
            }
        }

        for (k, v) in &envs {
            cmd.env(k, v);
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        // 独立进程组，便于整棵进程树一起终止
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }

        cmd.spawn().map_err(|e| format!("启动进程失败: {e}"))
    }
}

/// P1-14: 支持引号的参数分词。
/// 比 `split_whitespace` 多支持 `"..."` 和 `'...'` 包裹的含空格参数。
fn split_args(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();
    while let Some(&ch) = chars.peek() {
        match ch {
            '"' | '\'' => {
                let quote = ch;
                chars.next(); // consume quote
                while let Some(&c) = chars.peek() {
                    if c == quote {
                        chars.next(); // consume closing quote
                        break;
                    }
                    current.push(c);
                    chars.next();
                }
            }
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    result.push(std::mem::take(&mut current));
                }
                chars.next();
            }
            _ => {
                current.push(ch);
                chars.next();
            }
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}
