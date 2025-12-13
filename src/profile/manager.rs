//! プロファイルマネージャー
//!
//! プロファイルの作成、読み込み、一覧、削除、エクスポート/インポートを行う。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::common::config::Config;
use crate::common::error::{NelstError, Result};

/// プロファイル
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// プロファイル名
    pub name: String,

    /// 説明
    #[serde(default)]
    pub description: String,

    /// 作成日時
    pub created_at: String,

    /// 更新日時
    pub updated_at: String,

    /// コマンドタイプ (load, scan, diag, bench)
    pub command_type: String,

    /// サブコマンドタイプ (traffic, connection, http, port, ping, etc.)
    pub subcommand_type: String,

    /// オプション（キーバリュー形式）
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

impl Profile {
    /// 新しいプロファイルを作成
    pub fn new(
        name: &str,
        command_type: &str,
        subcommand_type: &str,
        description: Option<&str>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            name: name.to_string(),
            description: description.unwrap_or("").to_string(),
            created_at: now.clone(),
            updated_at: now,
            command_type: command_type.to_string(),
            subcommand_type: subcommand_type.to_string(),
            options: HashMap::new(),
        }
    }

    /// オプションを設定
    #[allow(dead_code)]
    pub fn set_option<T: Serialize>(&mut self, key: &str, value: T) -> Result<()> {
        let json_value = serde_json::to_value(value)
            .map_err(|e| NelstError::config(format!("Failed to serialize option: {}", e)))?;
        self.options.insert(key.to_string(), json_value);
        self.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }

    /// オプションを取得
    #[allow(dead_code)]
    pub fn get_option<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.options
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// オプションを文字列として取得
    #[allow(dead_code)]
    pub fn get_option_string(&self, key: &str) -> Option<String> {
        self.options.get(key).map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            _ => v.to_string(),
        })
    }
}

/// プロファイルマネージャー
#[derive(Debug)]
pub struct ProfileManager {
    /// プロファイル保存ディレクトリ
    profiles_dir: PathBuf,
}

impl ProfileManager {
    /// 新しいプロファイルマネージャーを作成
    pub fn new() -> Result<Self> {
        let profiles_dir = Config::profiles_dir()
            .ok_or_else(|| NelstError::config("Could not determine home directory"))?;

        // プロファイルディレクトリを作成
        if !profiles_dir.exists() {
            fs::create_dir_all(&profiles_dir).map_err(|e| {
                NelstError::config(format!(
                    "Failed to create profiles directory {:?}: {}",
                    profiles_dir, e
                ))
            })?;
        }

        Ok(Self { profiles_dir })
    }

    /// カスタムディレクトリでプロファイルマネージャーを作成
    #[allow(dead_code)]
    pub fn with_dir(profiles_dir: PathBuf) -> Result<Self> {
        if !profiles_dir.exists() {
            fs::create_dir_all(&profiles_dir).map_err(|e| {
                NelstError::config(format!(
                    "Failed to create profiles directory {:?}: {}",
                    profiles_dir, e
                ))
            })?;
        }

        Ok(Self { profiles_dir })
    }

    /// プロファイルのファイルパスを取得
    fn profile_path(&self, name: &str) -> PathBuf {
        self.profiles_dir.join(format!("{}.toml", name))
    }

    /// プロファイルを保存
    pub fn save(&self, profile: &Profile) -> Result<()> {
        let path = self.profile_path(&profile.name);
        let content = toml::to_string_pretty(profile)
            .map_err(|e| NelstError::config(format!("Failed to serialize profile: {}", e)))?;

        fs::write(&path, content).map_err(|e| {
            NelstError::config(format!("Failed to write profile to {:?}: {}", path, e))
        })?;

        Ok(())
    }

    /// プロファイルを読み込み
    pub fn load(&self, name: &str) -> Result<Profile> {
        let path = self.profile_path(name);
        if !path.exists() {
            return Err(NelstError::config(format!("Profile '{}' not found", name)));
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| NelstError::config(format!("Failed to read profile {:?}: {}", path, e)))?;

        toml::from_str(&content)
            .map_err(|e| NelstError::config(format!("Failed to parse profile: {}", e)))
    }

    /// プロファイル一覧を取得
    pub fn list(&self) -> Result<Vec<ProfileInfo>> {
        let mut profiles = Vec::new();

        let entries = fs::read_dir(&self.profiles_dir).map_err(|e| {
            NelstError::config(format!(
                "Failed to read profiles directory {:?}: {}",
                self.profiles_dir, e
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                NelstError::config(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                    match self.load(name) {
                        Ok(profile) => {
                            profiles.push(ProfileInfo {
                                name: profile.name,
                                description: profile.description,
                                command_type: profile.command_type,
                                subcommand_type: profile.subcommand_type,
                                updated_at: profile.updated_at,
                            });
                        }
                        Err(_) => {
                            // 破損したプロファイルはスキップ
                            continue;
                        }
                    }
                }
            }
        }

        // 名前でソート
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    /// プロファイルを削除
    pub fn delete(&self, name: &str) -> Result<()> {
        let path = self.profile_path(name);
        if !path.exists() {
            return Err(NelstError::config(format!("Profile '{}' not found", name)));
        }

        fs::remove_file(&path).map_err(|e| {
            NelstError::config(format!("Failed to delete profile {:?}: {}", path, e))
        })?;

        Ok(())
    }

    /// プロファイルが存在するか確認
    pub fn exists(&self, name: &str) -> bool {
        self.profile_path(name).exists()
    }

    /// プロファイルをファイルにエクスポート
    pub fn export(&self, name: &str, output_path: &str) -> Result<()> {
        let profile = self.load(name)?;
        let content = toml::to_string_pretty(&profile)
            .map_err(|e| NelstError::config(format!("Failed to serialize profile: {}", e)))?;

        fs::write(output_path, content).map_err(|e| {
            NelstError::config(format!("Failed to write to {}: {}", output_path, e))
        })?;

        Ok(())
    }

    /// ファイルからプロファイルをインポート
    pub fn import(&self, input_path: &str, new_name: Option<&str>) -> Result<Profile> {
        let content = fs::read_to_string(input_path)
            .map_err(|e| NelstError::config(format!("Failed to read {}: {}", input_path, e)))?;

        let mut profile: Profile = toml::from_str(&content)
            .map_err(|e| NelstError::config(format!("Failed to parse profile: {}", e)))?;

        // 新しい名前が指定された場合は上書き
        if let Some(name) = new_name {
            profile.name = name.to_string();
        }

        // 保存
        self.save(&profile)?;

        Ok(profile)
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new().expect("Failed to create ProfileManager")
    }
}

/// プロファイル情報（一覧表示用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    /// プロファイル名
    pub name: String,
    /// 説明
    pub description: String,
    /// コマンドタイプ
    pub command_type: String,
    /// サブコマンドタイプ
    pub subcommand_type: String,
    /// 更新日時
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_profiles_dir() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        env::temp_dir().join(format!("nelst_test_{}_{}", std::process::id(), id))
    }

    fn cleanup_temp_dir(path: &PathBuf) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn test_profile_new() {
        let profile = Profile::new("test", "load", "traffic", Some("Test profile"));
        assert_eq!(profile.name, "test");
        assert_eq!(profile.command_type, "load");
        assert_eq!(profile.subcommand_type, "traffic");
        assert_eq!(profile.description, "Test profile");
    }

    #[test]
    fn test_profile_options() {
        let mut profile = Profile::new("test", "load", "traffic", None);

        profile.set_option("target", "127.0.0.1:8080").unwrap();
        profile.set_option("duration", 60u64).unwrap();
        profile.set_option("concurrency", 10usize).unwrap();

        assert_eq!(
            profile.get_option_string("target"),
            Some("127.0.0.1:8080".to_string())
        );
        assert_eq!(profile.get_option::<u64>("duration"), Some(60));
        assert_eq!(profile.get_option::<usize>("concurrency"), Some(10));
    }

    #[test]
    fn test_profile_manager_save_load() {
        let dir = temp_profiles_dir();
        let manager = ProfileManager::with_dir(dir.clone()).unwrap();

        let mut profile = Profile::new("test_profile", "scan", "port", Some("Port scan"));
        profile.set_option("target", "192.168.1.1").unwrap();
        profile.set_option("ports", "1-1024").unwrap();

        manager.save(&profile).unwrap();
        assert!(manager.exists("test_profile"));

        let loaded = manager.load("test_profile").unwrap();
        assert_eq!(loaded.name, "test_profile");
        assert_eq!(loaded.command_type, "scan");
        assert_eq!(
            loaded.get_option_string("target"),
            Some("192.168.1.1".to_string())
        );

        cleanup_temp_dir(&dir);
    }

    #[test]
    fn test_profile_manager_list() {
        let dir = temp_profiles_dir();
        let manager = ProfileManager::with_dir(dir.clone()).unwrap();

        let profile1 = Profile::new("alpha", "load", "traffic", None);
        let profile2 = Profile::new("beta", "scan", "port", None);
        let profile3 = Profile::new("gamma", "diag", "ping", None);

        manager.save(&profile1).unwrap();
        manager.save(&profile2).unwrap();
        manager.save(&profile3).unwrap();

        let list = manager.list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].name, "alpha");
        assert_eq!(list[1].name, "beta");
        assert_eq!(list[2].name, "gamma");

        cleanup_temp_dir(&dir);
    }

    #[test]
    fn test_profile_manager_delete() {
        let dir = temp_profiles_dir();
        let manager = ProfileManager::with_dir(dir.clone()).unwrap();

        let profile = Profile::new("to_delete", "load", "http", None);
        manager.save(&profile).unwrap();
        assert!(manager.exists("to_delete"));

        manager.delete("to_delete").unwrap();
        assert!(!manager.exists("to_delete"));

        cleanup_temp_dir(&dir);
    }

    #[test]
    fn test_profile_manager_export_import() {
        let dir = temp_profiles_dir();
        let export_path = dir.join("exported.toml");
        let manager = ProfileManager::with_dir(dir.clone()).unwrap();

        let mut profile = Profile::new("original", "bench", "bandwidth", Some("Bandwidth test"));
        profile.set_option("duration", 30u64).unwrap();
        manager.save(&profile).unwrap();

        // エクスポート
        manager
            .export("original", export_path.to_str().unwrap())
            .unwrap();
        assert!(export_path.exists());

        // 別名でインポート
        let imported = manager
            .import(export_path.to_str().unwrap(), Some("imported"))
            .unwrap();
        assert_eq!(imported.name, "imported");
        assert_eq!(imported.description, "Bandwidth test");

        cleanup_temp_dir(&dir);
    }

    #[test]
    fn test_load_nonexistent_profile() {
        let dir = temp_profiles_dir();
        let manager = ProfileManager::with_dir(dir.clone()).unwrap();

        let result = manager.load("nonexistent");
        assert!(result.is_err());

        cleanup_temp_dir(&dir);
    }
}
