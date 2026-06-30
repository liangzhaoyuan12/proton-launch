use std::collections::HashMap;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use crate::config::{ConfigStore, GameConfig};

const UMU_RUN: &[u8] = include_bytes!("../umu-run");

pub struct Runner {
    umu_path: PathBuf,
    python_check: bool,
}

impl Runner {
    pub fn new() -> Self {
        let data_dir = ConfigStore::data_dir();
        let umu_path = data_dir.join("umu-run");
        Runner { umu_path, python_check: false }
    }

    pub fn extract_umu(&self) -> Result<(), String> {
        if self.umu_path.exists() {
            return Ok(());
        }
        let parent = self.umu_path.parent().unwrap();
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;

        let mut umu_zip = Vec::with_capacity(UMU_RUN.len());
        umu_zip.extend_from_slice(UMU_RUN);
        // Write Python zipapp as-is (it has the shebang inside)
        fs::write(&self.umu_path, &umu_zip).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o755);
            fs::set_permissions(&self.umu_path, perms).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub fn is_python_available(&mut self) -> bool {
        if self.python_check {
            return true;
        }
        let result = Command::new("python3").arg("--version").output();
        match result {
            Ok(output) if output.status.success() => {
                self.python_check = true;
                true
            }
            _ => {
                // try python
                match Command::new("python").arg("--version").output() {
                    Ok(o) if o.status.success() => {
                        self.python_check = true;
                        true
                    }
                    _ => false,
                }
            }
        }
    }

    pub fn run_game(&mut self, config: &GameConfig) -> Result<std::process::Child, String> {
        self.extract_umu()?;

        if !self.umu_path.exists() {
            return Err("umu-run not extracted".to_string());
        }

        let mut envs: HashMap<String, String> = HashMap::new();
        // Merge GAMEID from env_vars, or default
        let game_id = config
            .env_vars
            .get("GAMEID")
            .cloned()
            .unwrap_or_else(|| "umu-default".to_string());
        let store = config
            .env_vars
            .get("STORE")
            .cloned()
            .unwrap_or_else(|| "none".to_string());

        envs.insert("GAMEID".to_string(), game_id);
        envs.insert("STORE".to_string(), store);

        // Copy all user env vars
        for (k, v) in &config.env_vars {
            if k != "GAMEID" && k != "STORE" {
                envs.insert(k.clone(), v.clone());
            }
        }

        // Build command
        let mut cmd = if self.is_python_available() {
            let mut c = Command::new("python3");
            c.arg(&self.umu_path);
            c
        } else {
            let c = Command::new(&self.umu_path);
            c
        };

        // Add executable and args
        if !config.executable.is_empty() {
            cmd.arg(&config.executable);
        }
        if !config.renderer.is_empty() {
            cmd.arg(&config.renderer);
        }
        if !config.args.is_empty() {
            // Split args by whitespace
            for arg in config.args.split_whitespace() {
                cmd.arg(arg);
            }
        }

        // Set work dir — 默认取可执行文件所在目录
        if !config.work_dir.is_empty() {
            cmd.current_dir(&config.work_dir);
        } else if !config.executable.is_empty() {
            if let Some(parent) = std::path::Path::new(&config.executable).parent() {
                cmd.current_dir(parent);
            }
        }

        // Set environment variables
        for (k, v) in &envs {
            cmd.env(k, v);
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        // Create own process group so we can kill the whole tree
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }

        cmd.spawn().map_err(|e| format!("failed to launch: {e}"))
    }

    #[allow(dead_code)]
    pub fn umu_path(&self) -> &PathBuf {
        &self.umu_path
    }
}
