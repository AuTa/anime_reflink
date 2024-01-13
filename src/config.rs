use std::{borrow::Cow, fmt};

#[derive(Debug)]
pub struct Config {
    pub action: Action,
    pub mapfile_path: String,
    pub source_path: String,
    pub anime_path: String,
}

impl Config {
    pub fn new(args: &[String]) -> Config {
        let action: Action;

        match args.get(1) {
            Some(x) => match x.as_str() {
                "test" => action = Action::Test,
                "reflink" => action = Action::Reflink,
                _ => action = Action::Test,
            },
            _ => action = Action::Test,
        }
        let mapfile_path = args
            .get(2) // 存在所有权问题，不使用 `unwrap_or_else`.
            .unwrap_or(&"data.yaml".to_string())
            .to_string();
        let source_path = args
            .get(3)
            .map(Cow::Borrowed) // 用 Cow 解决 `unwrap_or_else` 的所有权问题.
            .unwrap_or_else(|| Cow::Owned(String::from("X:\\SOURCE")))
            .to_string();
        let anime_path = args
            .get(4)
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned("X:\\ANIME".to_string()))
            .to_string();

        Config {
            action,
            mapfile_path,
            source_path,
            anime_path,
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Test,
    Reflink,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Action::*; // 非常方便的通配符用法，将枚举名称暂时放入方法上下文中.
        match self {
            Test => write!(f, "test"),
            Reflink => write!(f, "reflink"),
        }
    }
}
