//! レポートフォーマッター
//!
//! JSON, CSV, HTML, Markdown形式でのレポート出力を行う。n
use serde::{Deserialize, Serialize};
use std::fs;

use crate::common::error::{NelstError, Result};

/// レポート形式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    /// JSON形式
    Json,
    /// CSV形式
    Csv,
    /// HTML形式
    Html,
    /// Markdown形式
    Markdown,
    /// テキスト形式
    Text,
}

impl ReportFormat {
    /// 文字列からフォーマットを解析
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "json" => Ok(ReportFormat::Json),
            "csv" => Ok(ReportFormat::Csv),
            "html" => Ok(ReportFormat::Html),
            "markdown" | "md" => Ok(ReportFormat::Markdown),
            "text" | "txt" => Ok(ReportFormat::Text),
            _ => Err(NelstError::config(format!(
                "Unknown report format: {}. Valid formats: json, csv, html, markdown, text",
                s
            ))),
        }
    }

    /// ファイル拡張子を取得
    #[allow(dead_code)]
    pub fn extension(&self) -> &'static str {
        match self {
            ReportFormat::Json => "json",
            ReportFormat::Csv => "csv",
            ReportFormat::Html => "html",
            ReportFormat::Markdown => "md",
            ReportFormat::Text => "txt",
        }
    }
}

/// レポートジェネレーター
#[derive(Debug)]
#[allow(dead_code)]
pub struct ReportGenerator {
    /// レポートタイトル
    title: String,
    /// レポート説明
    description: Option<String>,
    /// 生成日時
    generated_at: String,
}

#[allow(dead_code)]
impl ReportGenerator {
    /// 新しいレポートジェネレーターを作成
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            description: None,
            generated_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// 説明を設定
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    /// JSON形式でレポートを生成
    pub fn to_json<T: Serialize>(&self, data: &T) -> Result<String> {
        serde_json::to_string_pretty(data)
            .map_err(|e| NelstError::config(format!("Failed to serialize to JSON: {}", e)))
    }

    /// CSV形式でレポートを生成（テーブルデータ用）
    pub fn to_csv(&self, headers: &[&str], rows: &[Vec<String>]) -> Result<String> {
        let mut output = String::new();

        // ヘッダー行
        output.push_str(&headers.join(","));
        output.push('\n');

        // データ行
        for row in rows {
            let escaped: Vec<String> = row
                .iter()
                .map(|cell| {
                    if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                        format!("\"{}\"", cell.replace('"', "\"\""))
                    } else {
                        cell.clone()
                    }
                })
                .collect();
            output.push_str(&escaped.join(","));
            output.push('\n');
        }

        Ok(output)
    }

    /// HTML形式でレポートを生成
    pub fn to_html(&self, sections: &[ReportSection]) -> Result<String> {
        let mut html = String::new();

        // HTMLヘッダー
        html.push_str("<!DOCTYPE html>\n");
        html.push_str("<html lang=\"en\">\n");
        html.push_str("<head>\n");
        html.push_str("  <meta charset=\"UTF-8\">\n");
        html.push_str(
            "  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
        );
        html.push_str(&format!("  <title>{}</title>\n", escape_html(&self.title)));
        html.push_str("  <style>\n");
        html.push_str(CSS_STYLES);
        html.push_str("  </style>\n");
        html.push_str("</head>\n");
        html.push_str("<body>\n");

        // ヘッダー
        html.push_str("  <div class=\"container\">\n");
        html.push_str(&format!("    <h1>{}</h1>\n", escape_html(&self.title)));
        if let Some(ref desc) = self.description {
            html.push_str(&format!(
                "    <p class=\"description\">{}</p>\n",
                escape_html(desc)
            ));
        }
        html.push_str(&format!(
            "    <p class=\"meta\">Generated: {}</p>\n",
            &self.generated_at
        ));

        // セクション
        for section in sections {
            html.push_str(&format!("    <h2>{}</h2>\n", escape_html(&section.title)));

            match &section.content {
                SectionContent::KeyValue(items) => {
                    html.push_str("    <table class=\"kv-table\">\n");
                    for (key, value) in items {
                        html.push_str(&format!(
                            "      <tr><th>{}</th><td>{}</td></tr>\n",
                            escape_html(key),
                            escape_html(value)
                        ));
                    }
                    html.push_str("    </table>\n");
                }
                SectionContent::Table { headers, rows } => {
                    html.push_str("    <table class=\"data-table\">\n");
                    html.push_str("      <thead><tr>\n");
                    for header in headers {
                        html.push_str(&format!("        <th>{}</th>\n", escape_html(header)));
                    }
                    html.push_str("      </tr></thead>\n");
                    html.push_str("      <tbody>\n");
                    for row in rows {
                        html.push_str("      <tr>\n");
                        for cell in row {
                            html.push_str(&format!("        <td>{}</td>\n", escape_html(cell)));
                        }
                        html.push_str("      </tr>\n");
                    }
                    html.push_str("      </tbody>\n");
                    html.push_str("    </table>\n");
                }
                SectionContent::Text(text) => {
                    html.push_str(&format!("    <pre>{}</pre>\n", escape_html(text)));
                }
            }
        }

        html.push_str("  </div>\n");
        html.push_str("</body>\n");
        html.push_str("</html>\n");

        Ok(html)
    }

    /// Markdown形式でレポートを生成
    pub fn to_markdown(&self, sections: &[ReportSection]) -> Result<String> {
        let mut md = String::new();

        // タイトル
        md.push_str(&format!("# {}\n\n", &self.title));

        if let Some(ref desc) = self.description {
            md.push_str(&format!("{}\n\n", desc));
        }

        md.push_str(&format!("*Generated: {}*\n\n", &self.generated_at));

        // セクション
        for section in sections {
            md.push_str(&format!("## {}\n\n", &section.title));

            match &section.content {
                SectionContent::KeyValue(items) => {
                    for (key, value) in items {
                        md.push_str(&format!("- **{}**: {}\n", key, value));
                    }
                    md.push('\n');
                }
                SectionContent::Table { headers, rows } => {
                    // ヘッダー
                    md.push_str("| ");
                    md.push_str(&headers.join(" | "));
                    md.push_str(" |\n");

                    // セパレーター
                    md.push_str("| ");
                    md.push_str(
                        &headers
                            .iter()
                            .map(|_| "---")
                            .collect::<Vec<_>>()
                            .join(" | "),
                    );
                    md.push_str(" |\n");

                    // データ行
                    for row in rows {
                        md.push_str("| ");
                        md.push_str(&row.join(" | "));
                        md.push_str(" |\n");
                    }
                    md.push('\n');
                }
                SectionContent::Text(text) => {
                    md.push_str("```\n");
                    md.push_str(text);
                    md.push_str("\n```\n\n");
                }
            }
        }

        Ok(md)
    }

    /// テキスト形式でレポートを生成
    pub fn to_text(&self, sections: &[ReportSection]) -> Result<String> {
        let mut text = String::new();

        // タイトル
        let separator = "=".repeat(60);
        text.push_str(&separator);
        text.push('\n');
        text.push_str(&format!("  {}\n", &self.title));
        text.push_str(&separator);
        text.push_str("\n\n");

        if let Some(ref desc) = self.description {
            text.push_str(&format!("{}\n\n", desc));
        }

        text.push_str(&format!("Generated: {}\n\n", &self.generated_at));

        // セクション
        for section in sections {
            text.push_str(&format!("--- {} ---\n\n", &section.title));

            match &section.content {
                SectionContent::KeyValue(items) => {
                    let max_key_len = items.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
                    for (key, value) in items {
                        text.push_str(&format!(
                            "  {:width$}  {}\n",
                            key,
                            value,
                            width = max_key_len
                        ));
                    }
                    text.push('\n');
                }
                SectionContent::Table { headers, rows } => {
                    // カラム幅を計算
                    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
                    for row in rows {
                        for (i, cell) in row.iter().enumerate() {
                            if i < widths.len() && cell.len() > widths[i] {
                                widths[i] = cell.len();
                            }
                        }
                    }

                    // ヘッダー
                    let header_line: String = headers
                        .iter()
                        .zip(&widths)
                        .map(|(h, w)| format!("{:width$}", h, width = *w))
                        .collect::<Vec<_>>()
                        .join("  ");
                    text.push_str(&format!("  {}\n", header_line));

                    // セパレーター
                    let sep_line: String = widths
                        .iter()
                        .map(|w| "-".repeat(*w))
                        .collect::<Vec<_>>()
                        .join("  ");
                    text.push_str(&format!("  {}\n", sep_line));

                    // データ行
                    for row in rows {
                        let row_line: String = row
                            .iter()
                            .zip(&widths)
                            .map(|(c, w)| format!("{:width$}", c, width = *w))
                            .collect::<Vec<_>>()
                            .join("  ");
                        text.push_str(&format!("  {}\n", row_line));
                    }
                    text.push('\n');
                }
                SectionContent::Text(t) => {
                    for line in t.lines() {
                        text.push_str(&format!("  {}\n", line));
                    }
                    text.push('\n');
                }
            }
        }

        Ok(text)
    }

    /// 指定された形式でレポートを生成してファイルに保存
    pub fn save_to_file(
        &self,
        format: ReportFormat,
        sections: &[ReportSection],
        path: &str,
    ) -> Result<()> {
        let content = match format {
            ReportFormat::Html => self.to_html(sections)?,
            ReportFormat::Markdown => self.to_markdown(sections)?,
            ReportFormat::Text => self.to_text(sections)?,
            _ => {
                return Err(NelstError::config(
                    "Use to_json or to_csv for JSON/CSV formats",
                ));
            }
        };

        fs::write(path, content).map_err(|e| {
            NelstError::config(format!("Failed to write report to {}: {}", path, e))
        })?;

        Ok(())
    }
}

/// レポートセクション
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ReportSection {
    /// セクションタイトル
    pub title: String,
    /// セクション内容
    pub content: SectionContent,
}

#[allow(dead_code)]
impl ReportSection {
    /// キーバリュー形式のセクションを作成
    pub fn key_value(title: &str, items: Vec<(&str, &str)>) -> Self {
        Self {
            title: title.to_string(),
            content: SectionContent::KeyValue(
                items
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ),
        }
    }

    /// テーブル形式のセクションを作成
    pub fn table(title: &str, headers: Vec<&str>, rows: Vec<Vec<String>>) -> Self {
        Self {
            title: title.to_string(),
            content: SectionContent::Table {
                headers: headers.into_iter().map(|h| h.to_string()).collect(),
                rows,
            },
        }
    }

    /// テキスト形式のセクションを作成
    pub fn text(title: &str, content: &str) -> Self {
        Self {
            title: title.to_string(),
            content: SectionContent::Text(content.to_string()),
        }
    }
}

/// セクション内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum SectionContent {
    /// キーバリュー形式
    KeyValue(Vec<(String, String)>),
    /// テーブル形式
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// テキスト形式
    Text(String),
}

/// HTMLエスケープ
#[allow(dead_code)]
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// CSSスタイル
#[allow(dead_code)]
const CSS_STYLES: &str = r#"
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
      line-height: 1.6;
      color: #333;
      background: #f5f5f5;
      margin: 0;
      padding: 20px;
    }
    .container {
      max-width: 1000px;
      margin: 0 auto;
      background: white;
      padding: 30px;
      border-radius: 8px;
      box-shadow: 0 2px 4px rgba(0,0,0,0.1);
    }
    h1 {
      color: #2c3e50;
      border-bottom: 3px solid #3498db;
      padding-bottom: 10px;
    }
    h2 {
      color: #34495e;
      margin-top: 30px;
      border-bottom: 1px solid #ecf0f1;
      padding-bottom: 5px;
    }
    .description {
      color: #7f8c8d;
      font-size: 1.1em;
    }
    .meta {
      color: #95a5a6;
      font-size: 0.9em;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      margin: 15px 0;
    }
    .kv-table th {
      text-align: left;
      width: 200px;
      padding: 8px 12px;
      background: #ecf0f1;
      border: 1px solid #bdc3c7;
    }
    .kv-table td {
      padding: 8px 12px;
      border: 1px solid #bdc3c7;
    }
    .data-table th {
      background: #3498db;
      color: white;
      padding: 10px;
      text-align: left;
    }
    .data-table td {
      padding: 10px;
      border-bottom: 1px solid #ecf0f1;
    }
    .data-table tbody tr:hover {
      background: #f8f9fa;
    }
    pre {
      background: #2c3e50;
      color: #ecf0f1;
      padding: 15px;
      border-radius: 4px;
      overflow-x: auto;
    }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_format_from_str() {
        assert_eq!(ReportFormat::from_str("json").unwrap(), ReportFormat::Json);
        assert_eq!(ReportFormat::from_str("CSV").unwrap(), ReportFormat::Csv);
        assert_eq!(ReportFormat::from_str("html").unwrap(), ReportFormat::Html);
        assert_eq!(
            ReportFormat::from_str("markdown").unwrap(),
            ReportFormat::Markdown
        );
        assert_eq!(
            ReportFormat::from_str("md").unwrap(),
            ReportFormat::Markdown
        );
        assert_eq!(ReportFormat::from_str("text").unwrap(), ReportFormat::Text);
        assert!(ReportFormat::from_str("invalid").is_err());
    }

    #[test]
    fn test_report_format_extension() {
        assert_eq!(ReportFormat::Json.extension(), "json");
        assert_eq!(ReportFormat::Csv.extension(), "csv");
        assert_eq!(ReportFormat::Html.extension(), "html");
        assert_eq!(ReportFormat::Markdown.extension(), "md");
        assert_eq!(ReportFormat::Text.extension(), "txt");
    }

    #[test]
    fn test_to_csv() {
        let generator = ReportGenerator::new("Test Report");
        let headers = vec!["Port", "State", "Service"];
        let rows = vec![
            vec!["22".to_string(), "open".to_string(), "ssh".to_string()],
            vec!["80".to_string(), "open".to_string(), "http".to_string()],
        ];

        let csv = generator.to_csv(&headers, &rows).unwrap();
        assert!(csv.contains("Port,State,Service"));
        assert!(csv.contains("22,open,ssh"));
        assert!(csv.contains("80,open,http"));
    }

    #[test]
    fn test_to_csv_escape() {
        let generator = ReportGenerator::new("Test");
        let headers = vec!["Name", "Value"];
        let rows = vec![vec![
            "test,with,commas".to_string(),
            "has \"quotes\"".to_string(),
        ]];

        let csv = generator.to_csv(&headers, &rows).unwrap();
        assert!(csv.contains("\"test,with,commas\""));
        assert!(csv.contains("\"has \"\"quotes\"\"\""));
    }

    #[test]
    fn test_to_html() {
        let generator = ReportGenerator::new("Test Report").with_description("Test description");

        let sections = vec![
            ReportSection::key_value("Summary", vec![("Total", "100"), ("Success", "95")]),
            ReportSection::table(
                "Results",
                vec!["Port", "State"],
                vec![vec!["22".to_string(), "open".to_string()]],
            ),
        ];

        let html = generator.to_html(&sections).unwrap();
        assert!(html.contains("<title>Test Report</title>"));
        assert!(html.contains("Test description"));
        assert!(html.contains("Summary"));
        assert!(html.contains("Total"));
        assert!(html.contains("22"));
    }

    #[test]
    fn test_to_markdown() {
        let generator = ReportGenerator::new("Test Report");

        let sections = vec![
            ReportSection::key_value("Info", vec![("Target", "192.168.1.1")]),
            ReportSection::table(
                "Ports",
                vec!["Port", "Service"],
                vec![vec!["80".to_string(), "http".to_string()]],
            ),
        ];

        let md = generator.to_markdown(&sections).unwrap();
        assert!(md.contains("# Test Report"));
        assert!(md.contains("## Info"));
        assert!(md.contains("**Target**: 192.168.1.1"));
        assert!(md.contains("| Port | Service |"));
        assert!(md.contains("| 80 | http |"));
    }

    #[test]
    fn test_to_text() {
        let generator = ReportGenerator::new("Test Report");

        let sections = vec![ReportSection::text("Output", "Line 1\nLine 2")];

        let text = generator.to_text(&sections).unwrap();
        assert!(text.contains("Test Report"));
        assert!(text.contains("--- Output ---"));
        assert!(text.contains("Line 1"));
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("a & b"), "a &amp; b");
        assert_eq!(escape_html("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_escape_html_single_quote() {
        assert_eq!(escape_html("it's"), "it&#x27;s");
    }

    #[test]
    fn test_report_generator_with_description() {
        let generator = ReportGenerator::new("My Report").with_description("Description text");
        let sections = vec![];
        let html = generator.to_html(&sections).unwrap();
        assert!(html.contains("Description text"));
    }

    #[test]
    fn test_to_json() {
        use serde_json::json;
        let generator = ReportGenerator::new("JSON Test");
        let data = json!({
            "port": 80,
            "state": "open",
            "service": "http"
        });
        let json_str = generator.to_json(&data).unwrap();
        assert!(json_str.contains("\"port\": 80"));
        assert!(json_str.contains("\"state\": \"open\""));
    }

    #[test]
    fn test_report_section_key_value() {
        let section =
            ReportSection::key_value("Test", vec![("key1", "value1"), ("key2", "value2")]);
        assert_eq!(section.title, "Test");
        match section.content {
            SectionContent::KeyValue(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], ("key1".to_string(), "value1".to_string()));
            }
            _ => panic!("Expected KeyValue content"),
        }
    }

    #[test]
    fn test_report_section_table() {
        let section = ReportSection::table(
            "Table",
            vec!["H1", "H2"],
            vec![vec!["a".to_string(), "b".to_string()]],
        );
        assert_eq!(section.title, "Table");
        match section.content {
            SectionContent::Table { headers, rows } => {
                assert_eq!(headers, vec!["H1", "H2"]);
                assert_eq!(rows.len(), 1);
            }
            _ => panic!("Expected Table content"),
        }
    }

    #[test]
    fn test_report_section_text() {
        let section = ReportSection::text("Text", "Some content");
        assert_eq!(section.title, "Text");
        match section.content {
            SectionContent::Text(content) => {
                assert_eq!(content, "Some content");
            }
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_to_csv_empty() {
        let generator = ReportGenerator::new("Empty");
        let headers: Vec<&str> = vec![];
        let rows: Vec<Vec<String>> = vec![];
        let csv = generator.to_csv(&headers, &rows).unwrap();
        assert_eq!(csv.trim(), "");
    }

    #[test]
    fn test_to_csv_with_newlines() {
        let generator = ReportGenerator::new("Test");
        let headers = vec!["Data"];
        let rows = vec![vec!["line1\nline2".to_string()]];
        let csv = generator.to_csv(&headers, &rows).unwrap();
        assert!(csv.contains("\"line1\nline2\""));
    }

    #[test]
    fn test_to_html_empty_sections() {
        let generator = ReportGenerator::new("Empty Report");
        let sections: Vec<ReportSection> = vec![];
        let html = generator.to_html(&sections).unwrap();
        assert!(html.contains("<title>Empty Report</title>"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn test_to_markdown_empty_sections() {
        let generator = ReportGenerator::new("Empty Report");
        let sections: Vec<ReportSection> = vec![];
        let md = generator.to_markdown(&sections).unwrap();
        assert!(md.contains("# Empty Report"));
    }

    #[test]
    fn test_to_text_empty_sections() {
        let generator = ReportGenerator::new("Empty Report");
        let sections: Vec<ReportSection> = vec![];
        let text = generator.to_text(&sections).unwrap();
        assert!(text.contains("Empty Report"));
    }

    #[test]
    fn test_to_html_with_all_section_types() {
        let generator = ReportGenerator::new("Full Report").with_description("Complete test");

        let sections = vec![
            ReportSection::key_value("Info", vec![("Key", "Value")]),
            ReportSection::table(
                "Data",
                vec!["Col1", "Col2"],
                vec![vec!["A".to_string(), "B".to_string()]],
            ),
            ReportSection::text("Log", "Log content here"),
        ];

        let html = generator.to_html(&sections).unwrap();
        assert!(html.contains("Info"));
        assert!(html.contains("Key"));
        assert!(html.contains("Data"));
        assert!(html.contains("Col1"));
        assert!(html.contains("Log"));
        assert!(html.contains("Log content here"));
    }

    #[test]
    fn test_report_format_all_values() {
        let formats = vec![
            ("json", ReportFormat::Json),
            ("JSON", ReportFormat::Json),
            ("csv", ReportFormat::Csv),
            ("html", ReportFormat::Html),
            ("HTML", ReportFormat::Html),
            ("markdown", ReportFormat::Markdown),
            ("MARKDOWN", ReportFormat::Markdown),
            ("md", ReportFormat::Markdown),
            ("MD", ReportFormat::Markdown),
            ("text", ReportFormat::Text),
            ("txt", ReportFormat::Text),
            ("TXT", ReportFormat::Text),
        ];

        for (input, expected) in formats {
            assert_eq!(
                ReportFormat::from_str(input).unwrap(),
                expected,
                "Failed for input: {}",
                input
            );
        }
    }

    #[test]
    fn test_save_to_file() {
        use std::env;

        let generator = ReportGenerator::new("Save Test");
        let temp_dir = env::temp_dir().join(format!("nelst_report_test_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).unwrap();
        let output_path = temp_dir.join("test_report.html");

        let sections = vec![ReportSection::text("Content", "Test content")];

        generator
            .save_to_file(ReportFormat::Html, &sections, output_path.to_str().unwrap())
            .unwrap();

        assert!(output_path.exists());
        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("Save Test"));
        assert!(content.contains("Test content"));

        // クリーンアップ
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
