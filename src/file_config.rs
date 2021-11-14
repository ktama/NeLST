use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use lazy_static::lazy_static;
use log::debug;
use toml::Value;

lazy_static! {
    pub static ref CONFIG: Value = {
        return load_config();
    };
}

// toml形式の設定ファイルを読み込む
fn load_config() -> Value {
    // 当プログラムのディレクトリ
    let cur_path_str = env::current_exe().unwrap().clone();
    let cur_path = Path::new(&cur_path_str);
    let cur_dir = cur_path.parent().unwrap().display();

    let mut conf_toml_str;
    conf_toml_str = get_text_file(Path::new("config/config.toml"), "toml");
    // 文字列内に「{CUR}」が存在すれば、当プログラムが存在するディレクトリとみなして、カレントディレクトリに置換
    conf_toml_str = conf_toml_str.replace("{CUR}", &format!("{}", &cur_dir));

    // 設定をtoml形式に変換して返す
    return conf_toml_str.parse::<Value>().expect(&format!(
        "couldn't parse config file to toml format.{}",
        &conf_toml_str
    ));
}

fn get_text_file(path: &Path, extension: &'static str) -> String {
    let display = path.display();
    match path.extension() {
        None => return "".to_string(),
        Some(ext) => {
            if ext != extension {
                return "".to_string();
            }
        }
    };

    // pathを読み込み専用モードで開く
    let f = match File::open(&path) {
        Err(e) => panic!("couldn't open {}: {}", display, &e.to_string()),
        Ok(f) => f,
    };

    //  バッファリングされたストリーム入力とする
    let mut br = BufReader::new(f);

    // ファイルの中身を文字列に読み込み
    let mut conf_toml_str = String::new();
    match br.read_to_string(&mut conf_toml_str) {
        Err(e) => panic!("couldn't read {}: {}", display, &e.to_string()),
        Ok(_) => debug!("{} contains:\n{}", display, conf_toml_str),
    }
    conf_toml_str
}
