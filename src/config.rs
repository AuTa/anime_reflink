use std::fmt;

#[derive(Debug)]
pub struct Config {
    pub action: Action,
    pub mapfile_path: String,
    pub source_path: String,
    pub anime_path: String,
}

impl Config {
    pub fn new(args: impl Iterator<Item = String>) -> Config {
        let mut args = args.skip(1);
        let action: Action = match args.next() {
            Some(x) => Action::from(x.as_str()),
            None => Action::Test,
        };
        let mapfile_path = match args.next() {
            Some(arg) => arg,
            None => ".data/data.yaml".to_string(),
        };
        let source_path = args.next().unwrap_or("X:\\SOURCE".to_string());
        let anime_path = args.next().unwrap_or("X:\\ANIME".to_string());

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
    Renew,
    Reflink,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Action::*; // 非常方便的通配符用法，将枚举名称暂时放入方法上下文中.
        match self {
            Test => write!(f, "test"),
            Renew => write!(f, "renew"),
            Reflink => write!(f, "reflink"),
        }
    }
}

impl From<&str> for Action {
    fn from(s: &str) -> Self {
        match s {
            "test" => Action::Test,
            "renew" => Action::Renew,
            "reflink" => Action::Reflink,
            _ => Action::Test,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config() {
        let args: Vec<String> = vec![];
        let config = Config::new(args.into_iter());
        assert_eq!(config.action.to_string(), Action::Test.to_string());
        assert_eq!(
            config.mapfile_path, ".data/data.yaml",
            "mapfile_path {}",
            config.mapfile_path
        );
        assert_eq!(config.source_path, "X:\\SOURCE");
        assert_eq!(config.anime_path, "X:\\ANIME");

        let args = vec![
            "".to_string(),
            "renew".to_string(),
            ".data/data.1.yaml".to_string(),
            "./SOURCE".to_string(),
            "./ANIME".to_string(),
        ];
        let config = Config::new(args.into_iter());
        assert_eq!(config.action.to_string(), Action::Renew.to_string());
        assert_eq!(
            config.mapfile_path, ".data/data.1.yaml",
            "mapfile_path {}",
            config.mapfile_path
        );
        assert_eq!(config.source_path, "./SOURCE");
        assert_eq!(config.anime_path, "./ANIME");

        let args = vec!["".to_string(), "reflink".to_string()];
        let config = Config::new(args.into_iter());
        assert_eq!(config.action.to_string(), Action::Reflink.to_string());
    }

    #[test]
    fn action_from() {
        assert!(matches!(Action::from("test"), Action::Test));
        assert!(matches!(Action::from("renew"), Action::Renew));
        assert!(matches!(Action::from("reflink"), Action::Reflink));
        assert!(matches!(Action::from("nottest"), Action::Test));
    }
}
