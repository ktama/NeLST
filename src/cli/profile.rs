//! プロファイル管理サブコマンドの定義

use clap::{Args, Subcommand};

/// プロファイル管理のサブコマンド
#[derive(Subcommand, Debug)]
pub enum ProfileCommands {
    /// プロファイル一覧を表示
    List,
    /// プロファイル詳細を表示
    Show(ShowArgs),
    /// プロファイルを削除
    Delete(DeleteArgs),
    /// プロファイルをエクスポート
    Export(ExportArgs),
    /// プロファイルをインポート
    Import(ImportArgs),
}

/// プロファイル表示の引数
#[derive(Args, Debug)]
pub struct ShowArgs {
    /// プロファイル名
    pub name: String,
}

/// プロファイル削除の引数
#[derive(Args, Debug)]
pub struct DeleteArgs {
    /// プロファイル名
    pub name: String,

    /// 確認なしで削除
    #[arg(short, long)]
    pub force: bool,
}

/// プロファイルエクスポートの引数
#[derive(Args, Debug)]
pub struct ExportArgs {
    /// プロファイル名
    pub name: String,

    /// 出力ファイル
    #[arg(short, long)]
    pub output: Option<String>,
}

/// プロファイルインポートの引数
#[derive(Args, Debug)]
pub struct ImportArgs {
    /// インポートするファイル
    pub file: String,

    /// 新しいプロファイル名（省略時はファイル内の名前を使用）
    #[arg(short, long)]
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_args() {
        let args = ShowArgs {
            name: "test-profile".to_string(),
        };
        assert_eq!(args.name, "test-profile");
    }

    #[test]
    fn test_delete_args() {
        let args = DeleteArgs {
            name: "old-profile".to_string(),
            force: true,
        };
        assert!(args.force);
    }

    #[test]
    fn test_export_args() {
        let args = ExportArgs {
            name: "my-profile".to_string(),
            output: Some("profile.toml".to_string()),
        };
        assert!(args.output.is_some());
    }

    #[test]
    fn test_import_args() {
        let args = ImportArgs {
            file: "shared-profile.toml".to_string(),
            name: Some("imported-profile".to_string()),
        };
        assert!(args.name.is_some());
    }
}
