use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs::{self, DirEntry, File},
    path::Path,
    process::Command,
    usize,
};

use serde::{Deserialize, Serialize};

use crate::{
    config::{Action, Config},
    source_anime_map::{FileType, SourceAnimeMap},
};

// data.yaml 的结构体.
pub struct Data {
    pub data: RealData,
    // 从序列化中跳过.
    source_map: HashMap<String, ()>,
    config: Config,
}

impl Data {
    // 从 yaml 文件中读取数据.
    pub fn from_yaml(config: Config) -> Data {
        let mut data: Data = Data {
            data: RealData {
                source_anime_maps: Vec::new(),
                animes: Vec::new(),
            },
            source_map: HashMap::new(),
            config,
        };

        // 读取文件并且缓存已有数据.
        if let Ok(file) = File::open(&data.config.mapfile_path) {
            data.data.from_file(file);
        }
        for i in &data.data.source_anime_maps {
            let name = i.source.clone();
            data.source_map.insert(name, ());
        }
        data
    }

    pub fn push_map_from_dir(&mut self) {
        let entries = fs::read_dir(&self.config.source_path);
        for dir_entry in entries.unwrap() {
            let dir_entry = dir_entry.unwrap();
            let name = dir_entry.file_name().into_string().unwrap();

            // 定义一个 push 函数, 根据不同的动作进行不同的处理.
            let mut push_fn: fn(&mut RealData, String, FileType) = RealData::push_new_map;

            if self.source_map.contains_key(&name) {
                match self.config.action {
                    Action::Renew => {
                        push_fn = RealData::push_renew_map;
                        println!("renew anime source: {}", name);
                    }
                    _ => continue,
                }
            } else {
                println!("new anime source: {}", name);
            }
            let file_type = self.get_map_file_type(&name, &dir_entry);
            push_fn(&mut self.data, name, file_type);
        }
    }

    // 获取文件类型.
    pub fn get_map_file_type(&self, name: &str, dir_entry: &DirEntry) -> FileType {
        let fs_file_type = dir_entry.file_type().unwrap();
        let mut file_type: FileType = FileType::File;
        if fs_file_type.is_file() {
            // 排除 .parts 文件.
            if name.ends_with(".parts") {
                file_type = FileType::Other
            }
        } else if fs_file_type.is_dir() {
            file_type = FileType::Dir;
            let entries = fs::read_dir(&dir_entry.path());
            if let Ok(entries) = entries {
                // 判断是否嵌套.
                // 先判断是否有其他文件,
                // 再生成子目录的 SourceAnimeMap.
                let (dir, other): (Vec<_>, Vec<_>) = entries
                    .into_iter()
                    .map(|x| x.unwrap())
                    .partition(|x| x.file_type().unwrap().is_dir());
                if other.len() == 0 {
                    let maps = dir
                        .iter()
                        .map(|x| SourceAnimeMap {
                            source: x.file_name().into_string().unwrap(),
                            anime: "".to_string(),
                            active: true,
                            file_type: FileType::Dir,
                        })
                        .collect();
                    file_type = FileType::Nesting(maps);
                }
            }
        }
        file_type
    }

    pub fn push_anime_from_dir(&mut self) -> Result<(), Box<dyn Error>> {
        let anime_dir = &self.config.anime_path;
        let entries = fs::read_dir(anime_dir);
        for dir_entry in entries? {
            let dir_entry = dir_entry?;
            let name = dir_entry.file_name().into_string().unwrap();

            self.data.push_anime(name);
        }
        Ok(())
    }

    pub fn write_yaml(&self) -> Result<(), Box<dyn Error>> {
        let _ = fs::write(
            &self.config.mapfile_path,
            serde_yaml::to_string(&self.data).unwrap().as_str(),
        );
        Ok(())
    }

    // 获取需要 relink 的 anime index.
    // 因为无法同时更改 map 的 anime, 所以把 anime name 也存进去.
    fn need_reflink_anime_indexes(
        &self,
        source_anime_maps: &Vec<SourceAnimeMap>,
        anime_set: &mut HashMap<String, HashSet<String>>,
    ) -> Option<Vec<(usize, usize, String)>> {
        let mut indexes = Vec::<(usize, usize, String)>::new();
        for i in 0..source_anime_maps.len() {
            let source_anime_map = &source_anime_maps[i];
            if !source_anime_map.active {
                continue;
            }
            if let FileType::Other = source_anime_map.file_type {
                continue;
            }
            if let FileType::Nesting(nesting) = &source_anime_map.file_type {
                let nesting_indexes = self.need_reflink_anime_indexes(nesting, anime_set);
                if let Some(nesting_indexes) = nesting_indexes {
                    indexes.extend(nesting_indexes.iter().map(|x| (i, x.0, x.2.clone())));
                }
            } else if source_anime_map.anime.is_empty() {
                let source = source_anime_map.source.clone();
                let anime = self.find_exist_anime(source, anime_set);
                if anime.is_some() {
                    let anime = anime.unwrap();
                    indexes.push((i, 0, anime));
                }
            } else {
                indexes.push((i, 0, source_anime_map.anime.clone()));
            }
        }
        if indexes.len() > 0 {
            Some(indexes)
        } else {
            None
        }
    }

    pub fn map_animes(&mut self) -> Result<(), Box<dyn Error>> {
        let mut anime_set = HashMap::<String, HashSet<String>>::new();
        let maps = &self.data.source_anime_maps;
        let reflink_queue = self.need_reflink_anime_indexes(maps, &mut anime_set);
        if let Some(reflink_queue) = reflink_queue {
            self.set_anime_name(&reflink_queue);
            if let Action::Reflink = self.config.action {
                let successed_index: Vec<(usize, usize)> = self.reflink(reflink_queue);
                for i in successed_index {
                    match &mut self.data.source_anime_maps[i.0].file_type {
                        FileType::Nesting(maps) => {
                            maps[i.1].active = false;
                        }
                        _ => self.data.source_anime_maps[i.0].active = false,
                    }
                }
            }
        }
        Ok(())
    }

    // 设置索引处的 map 的 anime name.
    fn set_anime_name(&mut self, reflink_queue: &Vec<(usize, usize, String)>) {
        for i in reflink_queue {
            match &mut self.data.source_anime_maps[i.0].file_type {
                FileType::Nesting(maps) => {
                    maps[i.1].anime = i.2.clone();
                }
                _ => self.data.source_anime_maps[i.0].anime = i.2.clone(),
            }
        }
    }

    fn reflink(&self, reflink_queue: Vec<(usize, usize, String)>) -> Vec<(usize, usize)> {
        let mut successed_index: Vec<(usize, usize)> = Vec::new();
        for i in reflink_queue {
            let result: Result<(), _>;
            match &self.data.source_anime_maps[i.0].file_type {
                FileType::Nesting(maps) => {
                    result = self.reflink_map(&maps[i.1]);
                }
                _ => result = self.reflink_map(&self.data.source_anime_maps[i.0]),
            }

            match result {
                Ok(_) => {
                    successed_index.push((i.0, i.1));
                }
                Err(e) => println!("reflink error:{}", e),
            }
        }
        successed_index
    }

    fn find_exist_anime(
        &self,
        source: String,
        anime_set: &mut HashMap<String, HashSet<String>>,
    ) -> Option<String> {
        for (anime, set) in anime_set.iter() {
            if set.contains(&source) {
                return Some(anime.clone());
            }
        }

        let source_path = &format!("{}/{}", self.config.source_path, source);
        let mut source_set: HashSet<String> = HashSet::new();
        if let Ok(entries) = fs::read_dir(source_path) {
            for dir_entry in entries {
                if let Ok(dir_entry) = dir_entry {
                    let name = dir_entry.file_name().into_string().unwrap();
                    if let Ok(file_type) = dir_entry.file_type() {
                        if file_type.is_file() {
                            // 排除非视频文件.
                            if [".mkv", ".mp4", ".avi"]
                                .iter()
                                .map(|suffixes| name.ends_with(suffixes))
                                .any(|x| x)
                            {
                                source_set.insert(name);
                            }
                        } else if file_type.is_dir() {
                            // 当文件夹名称超过 20 个字符，或者以 "Season" 开头时,
                            // 递归获取该文件夹下的文件.
                            if name.len() > 20 || name.to_lowercase().starts_with("season") {
                                source_set.insert(name);
                            }
                        }
                    }
                }
            }
        }
        for (anime, set) in &mut *anime_set {
            for _ in set.intersection(&source_set) {
                return Some(anime.clone());
            }
        }

        // cannot borrow `*self` as mutable more than once at a time
        // second mutable borrow occurs here
        //
        // closure requires unique access to `*self`
        // but it is already borrowed closure construction occurs her
        //
        // let animes = self.data.animes.clone();
        // for anime in &animes {
        //     if self.anime_tree.contains_key(anime) {
        //         continue;
        //     }
        //     let tree = self.fetch_anime_tree( anime);
        //     if tree.contains(&source) {
        //         return Some(anime.clone());
        //     }
        // }
        for i in 0..self.data.animes.len() {
            let anime = &self.data.animes[i].clone();

            if anime_set.contains_key(anime) {
                continue;
            }
            let tree = self.fetch_anime_set(anime, anime_set);
            if tree.contains(&source) {
                return Some(anime.to_string());
            }
        }

        for (anime, set) in anime_set {
            for _ in set.intersection(&source_set) {
                return Some(anime.clone());
            }
        }
        None
    }

    fn fetch_anime_set<'a>(
        &'a self,
        anime: &str,
        anime_set: &'a mut HashMap<String, HashSet<String>>,
    ) -> &HashSet<String> {
        let set = anime_set.entry(anime.to_string()).or_insert(HashSet::new());
        let anime_dir = &format!("{}/{}", &self.config.anime_path, anime);

        // 递归获取文件.
        fn fetch_set<P: AsRef<Path>>(set: &mut HashSet<String>, dir_path: P) {
            if let Ok(entries) = fs::read_dir(dir_path) {
                for dir_entry in entries {
                    if let Ok(dir_entry) = dir_entry {
                        let name = dir_entry.file_name().into_string().unwrap();

                        if let Ok(file_type) = dir_entry.file_type() {
                            if file_type.is_file() {
                                // 排除非视频文件.
                                if [".mkv", ".mp4", ".avi"]
                                    .iter()
                                    .map(|suffixes| name.ends_with(suffixes))
                                    .any(|x| x)
                                {
                                    set.insert(name);
                                }
                            } else if file_type.is_dir() {
                                // 当文件夹名称超过 20 个字符，或者以 "Season" 开头时,
                                // 递归获取该文件夹下的文件.
                                if name.len() > 20 || name.to_lowercase().starts_with("season") {
                                    set.insert(name);
                                    fetch_set(set, &dir_entry.path());
                                }
                            }
                        }
                    }
                }
            }
        }

        fetch_set(set, anime_dir);
        set
    }

    fn reflink_map(&self, source_anime_map: &SourceAnimeMap) -> Result<(), Box<dyn Error>> {
        let source = &format!("{}/{}", self.config.source_path, source_anime_map.source);
        let anime = &format!("{}/{}", self.config.anime_path, source_anime_map.anime);

        let os_type = std::env::consts::OS;
        match os_type {
            "linux" => {
                println!("Source: {}, Anime: {}.", source, anime);
                if fs::read_dir(anime).is_err() {
                    fs::create_dir(anime).unwrap();
                }
                let output = Command::new("cp")
                    .arg("--archive")
                    .arg("-r")
                    .arg("--reflink=always")
                    .arg(source)
                    .arg(anime)
                    .output();
                match output {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        println!("Error: {}", e);
                        Err(Box::new(e))
                    }
                }
            }
            _ => {
                let err = format!(
                    "Only support linux system. Source: {}, Anime: {}.",
                    source, anime
                );
                Err(err.into())
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RealData {
    pub source_anime_maps: Vec<SourceAnimeMap>, // 文件夹映射.
    pub animes: Vec<String>,                    // 动漫列表.
}

impl RealData {
    // 从文件中读取数据.
    fn from_file(&mut self, file: File) {
        // serde_yaml::from_reader::<&File, Data> 指定泛型类型.
        if let Ok(exist_data) = serde_yaml::from_reader::<&File, RealData>(&file) {
            let maps = exist_data.source_anime_maps;
            if maps.len() > 0 {
                self.source_anime_maps.extend(maps);
            }
        }
    }

    // 添加文件夹映射.
    pub fn push_new_map(&mut self, name: String, file_type: FileType) {
        let anime_map = bulid_anime_map(name, "".to_string(), file_type);
        self.source_anime_maps.push(anime_map);
    }

    // 更新文件夹映射.
    pub fn push_renew_map(&mut self, name: String, file_type: FileType) {
        let anime_map = self.source_anime_maps.iter_mut().find(|x| x.source == name);
        let anime_map = anime_map.unwrap();
        let file_type: FileType = match &file_type {
            FileType::Nesting(x) => {
                // 嵌套文件夹要继承父文件夹对应的 anime.
                // 这里所有权复杂，直接 clone 后修改.
                let y = x
                    .iter()
                    .map(|y| {
                        let mut z = y.clone();
                        z.anime = anime_map.anime.clone();
                        z
                    })
                    .collect();
                FileType::Nesting(y)
            }
            _ => file_type,
        };
        if anime_map.file_type != file_type {
            anime_map.file_type = file_type;
        }
    }

    pub fn push_anime(&mut self, name: String) {
        if !self.animes.contains(&name) {
            self.animes.push(name)
        }
    }
}
// 构建文件夹映射
fn bulid_anime_map(source: String, anime: String, file_type: FileType) -> SourceAnimeMap {
    SourceAnimeMap {
        source,
        anime,
        active: true,
        file_type,
    }
}
