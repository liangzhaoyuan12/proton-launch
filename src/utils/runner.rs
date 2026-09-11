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
    python_check: bool,
}

impl Runner {
    pub fn new() -> Self {
        let umu_path = ConfigStore::data_dir().join("umu-run");
        Runner {
            umu_path,
            python_check: false,
        }
    }

    /// 首次运行时把内嵌的 umu-run 释放到数据目录。
    pub fn extract_umu(&self) -> Result<(), String> {
        if self.umu_path.exists() {
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
        Ok(())
    }

    /// 是否有可用的 python3（umu-run 是 python zipapp）。
    pub fn is_python_available(&mut self) -> bool {
        if self.python_check {
            return true;
        }
        let ok = Command::new("python3")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
            || Command::new("python")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
        self.python_check = ok;
        ok
    }

    /// 启动游戏，返回子进程句柄（stdout/stderr 已接管道）。
    pub fn run_game(&mut self, config: &GameConfig) -> Result<Child, String> {
        self.extract_umu()?;
        if !self.umu_path.exists() {
            return Err("umu-run 释放失败".to_string());
        }

        let mut envs: HashMap<String, String> = HashMap::new();
        envs.insert(
            "GAMEID".to_string(),
            config
                .env_vars
                .get("GAMEID")
                .cloned()
                .unwrap_or_else(|| "umu-default".to_string()),
        );
        envs.insert(
            "STORE".to_string(),
            config
                .env_vars
                .get("STORE")
                .cloned()
                .unwrap_or_else(|| "none".to_string()),
        );
        for (k, v) in &config.env_vars {
            if k != "GAMEID" && k != "STORE" {
                envs.insert(k.clone(), v.clone());
            }
        }

        let mut cmd = if self.is_python_available() {
            let mut c = Command::new("python3");
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
        for arg in config.args.split_whitespace() {
            cmd.arg(arg);
        }

        if !config.work_dir.trim().is_empty() {
            cmd.current_dir(config.work_dir.trim());
        } else if !config.executable.trim().is_empty() {
            if let Some(parent) = std::path::Path::new(config.executable.trim()).parent() {
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
