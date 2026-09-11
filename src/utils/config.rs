//! 配置读写：`games.json` 的加载与保存。

use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::model::GameConfig;

#[derive(Serialize, Deserialize, Default)]
struct ConfigFile {
    games: Vec<GameConfig>,
}

fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("com", "proton-launch", "proton-launch")
        .expect("无法确定配置目录")
}

/// 游戏列表的内存副本 + 磁盘持久化。
pub struct ConfigStore {
    path: PathBuf,
    games: Vec<GameConfig>,
}

impl ConfigStore {
    pub fn new() -> Self {
        let path = project_dirs().config_dir().join("games.json");
        let games = match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<ConfigFile>(&content) {
                Ok(cf) => cf.games,
                Err(e) => {
                    eprintln!("配置解析失败: {e}");
                    Vec::new()
                }
            },
            Err(_) => Vec::new(),
        };
        ConfigStore { path, games }
    }

    pub fn games(&self) -> &[GameConfig] {
        &self.games
    }

    pub fn game(&self, idx: usize) -> Option<&GameConfig> {
        self.games.get(idx)
    }

    pub fn len(&self) -> usize {
        self.games.len()
    }

    pub fn is_empty(&self) -> bool {
        self.games.is_empty()
    }

    /// 追加游戏并返回其索引。
    pub fn add_game(&mut self, game: GameConfig) -> usize {
        self.games.push(game);
        self.games.len() - 1
    }

    pub fn remove_game(&mut self, idx: usize) {
        if idx < self.games.len() {
            self.games.remove(idx);
        }
    }

    pub fn update_game(&mut self, idx: usize, game: GameConfig) {
        if idx < self.games.len() {
            self.games[idx] = game;
        }
    }

    /// 写入磁盘。
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&ConfigFile {
            games: self.games.clone(),
        })
        .map_err(|e| e.to_string())?;
        fs::write(&self.path, json).map_err(|e| e.to_string())
    }

    /// 应用数据目录（umu-run 落盘位置）。
    pub fn data_dir() -> PathBuf {
        project_dirs().data_dir().to_path_buf()
    }
}
