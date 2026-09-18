//! 配置读写：`games.json` 的加载与保存。

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::model::GameConfig;

#[derive(Serialize, Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    games: Vec<GameConfig>,
}

/// P2-2: ProjectDirs 不再 panic，返回 Result 由调用方处理。
fn project_dirs() -> Result<ProjectDirs, String> {
    ProjectDirs::from("com", "proton-launch", "proton-launch")
        .ok_or_else(|| "无法确定配置目录（$HOME 可能未设置）".to_string())
}

/// 游戏列表的内存副本 + 磁盘持久化。
pub struct ConfigStore {
    path: PathBuf,
    games: Vec<GameConfig>,
}

impl ConfigStore {
    pub fn new() -> (Self, Option<String>) {
        let (pd, load_error_init) = match project_dirs() {
            Ok(pd) => (pd, None),
            Err(e) => {
                // 无法确定配置目录，返回空配置 + 错误信息
                return (
                    ConfigStore {
                        path: std::path::PathBuf::from("games.json"),
                        games: Vec::new(),
                    },
                    Some(e),
                );
            }
        };
        let path = pd.config_dir().join("games.json");
        let mut error_msg = None;
        let games = match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<ConfigFile>(&content) {
                Ok(cf) => cf.games,
                Err(e) => {
                    // P0-4: 备份损坏文件并返回错误信息
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let backup = path.with_extension(format!("json.corrupt-{timestamp}"));
                    let backup_msg = if let Err(be) = fs::copy(&path, &backup) {
                        format!("（备份失败: {be}）")
                    } else {
                        format!(
                            "已备份为 {}",
                            backup.file_name().unwrap_or_default().to_string_lossy()
                        )
                    };
                    let msg = format!("配置文件解析失败: {e}。{backup_msg}，当前以空配置启动。");
                    eprintln!("{msg}");
                    error_msg = Some(msg);
                    Vec::new()
                }
            },
            Err(_) => Vec::new(),
        };
        (ConfigStore { path, games }, load_error_init.or(error_msg))
    }

    pub fn games(&self) -> &[GameConfig] {
        &self.games
    }

    /// 按 ID 查找游戏。
    pub fn game_by_id(&self, id: &str) -> Option<&GameConfig> {
        self.games.iter().find(|g| g.id == id)
    }

    /// 按 ID 查找索引位置。
    pub fn index_of_id(&self, id: &str) -> Option<usize> {
        self.games.iter().position(|g| g.id == id)
    }

    pub fn len(&self) -> usize {
        self.games.len()
    }

    pub fn is_empty(&self) -> bool {
        self.games.is_empty()
    }

    /// 追加游戏并返回其 ID。
    pub fn add_game(&mut self, game: GameConfig) -> String {
        let id = game.id.clone();
        self.games.push(game);
        id
    }

    /// 按 ID 删除游戏。
    pub fn remove_game_by_id(&mut self, id: &str) {
        self.games.retain(|g| g.id != id);
    }

    /// 按 ID 更新游戏。
    pub fn update_game_by_id(&mut self, id: &str, game: GameConfig) {
        if let Some(g) = self.games.iter_mut().find(|g| g.id == id) {
            *g = game;
        }
    }

    /// 写入磁盘（原子写：临时文件 + fsync + rename）。
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&ConfigFile {
            games: self.games.clone(),
        })
        .map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
            f.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())?;
        // 对目录做 fsync，确保 rename 元数据落盘
        if let Some(d) = self.path.parent() {
            let _ = fs::File::open(d).and_then(|f| f.sync_all());
        }
        Ok(())
    }

    /// 应用数据目录（umu-run 落盘位置）。
    pub fn data_dir() -> PathBuf {
        project_dirs()
            .map(|pd| pd.data_dir().to_path_buf())
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    }
}
