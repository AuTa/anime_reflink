use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs::{self, DirEntry, File},
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    cache::{Cache, CacheMap},
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
    pub fn new(config: Config) -> Data {
        Data {
            data: RealData::default(),
            source_map: HashMap::default(),
            config,
        }
    }

    // 从 yaml 文件中读取数据.
    pub fn from_yaml(config: Config) -> Data {
        // 读取文件并且缓存已有数据.
        let Ok(file) = File::open(&config.mapfile_path) else {
            return Data::new(config);
        };
        let real_data = RealData::from_file(file);
        let mut data: Data = Data {
            data: real_data,
            source_map: HashMap::new(),
            config,
        };

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
            // 判断是否嵌套.
            // 先判断是否有其他文件,
            // 再生成子目录的 SourceAnimeMap.
            let (dir, other): (Vec<_>, Vec<_>) = fs::read_dir(dir_entry.path())
                .into_iter()
                .flatten()
                .flatten()
                .partition(|x| x.file_type().unwrap().is_dir());
            // FIXME: 优化这个逻辑.
            if other.is_empty() {
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
        file_type
    }

    pub fn push_anime_from_dir(&mut self) -> Result<(), Box<dyn Error>> {
        let anime_dir = &self.config.anime_path;
        fs::read_dir(anime_dir)?
            .flatten()
            .flat_map(|dir_entry| dir_entry.file_name().into_string())
            .for_each(|name| self.data.push_anime(name.to_string()));

        Ok(())
    }

    pub fn write_yaml(&self) -> Result<(), Box<dyn Error>> {
        let _ = fs::write(
            &self.config.mapfile_path,
            serde_yaml::to_string(&self.data).unwrap().as_str(),
        );
        Ok(())
    }

    pub fn map_animes(&mut self) -> Result<(), Box<dyn Error>> {
        let mut anime_cache = CacheMap::default();
        let maps = &self.data.source_anime_maps;
        let reflink_queue = self.need_reflink_anime_indexes(maps, &mut anime_cache);
        let Some(reflink_queue) = reflink_queue else {
            return Ok(());
        };
        self.data.set_anime_name(&reflink_queue);
        if let Action::Reflink = self.config.action {
            let successed_index = self.reflink(&reflink_queue);
            self.data.set_map_active(&successed_index);
        }
        Ok(())
    }

    // 获取需要 relink 的 anime index.
    // 因为无法同时更改 map 的 anime, 所以把 anime name 也存进去.
    fn need_reflink_anime_indexes(
        &self,
        source_anime_maps: &[SourceAnimeMap],
        anime_cache: &mut CacheMap,
    ) -> Option<Vec<(usize, usize, String)>> {
        let mut indexes = Vec::<(usize, usize, String)>::new();
        source_anime_maps
            .iter()
            .enumerate()
            .filter(|(_, map)| map.active())
            .for_each(|(i, map)| {
                if let FileType::Nesting(nesting) = &map.file_type {
                    let nesting_indexes = self.need_reflink_anime_indexes(nesting, anime_cache);
                    if let Some(nesting_indexes) = nesting_indexes {
                        indexes.extend(nesting_indexes.iter().map(|x| (i, x.0, x.2.clone())));
                    }
                    // TODO: 设置 Nesting 父级的 anime 名.
                } else if map.anime.is_empty() {
                    let source = map.source.clone();
                    let anime = self.find_exist_anime(source, anime_cache);
                    if let Some(anime) = anime {
                        indexes.push((i, 0, anime));
                    }
                } else {
                    indexes.push((i, 0, map.anime.clone()));
                }
            });
        if !indexes.is_empty() {
            Some(indexes)
        } else {
            None
        }
    }

    fn reflink(&self, reflink_queue: &Vec<(usize, usize, String)>) -> Vec<(usize, usize, bool)> {
        let mut successed_index: Vec<(usize, usize, bool)> = Vec::new();

        for i in reflink_queue {
            let map = self.data.get_map_at_indexes((i.0, i.1));
            match self.reflink_map(map) {
                Ok(_) => {
                    successed_index.push((i.0, i.1, false));
                }
                Err(e) => println!("reflink error:{}", e),
            }
        }
        successed_index
    }

    fn find_exist_anime(&self, source: String, anime_cache: &mut CacheMap) -> Option<String> {
        if let Some(anime) = anime_cache
            .iter()
            .find(|(_, c)| c.contains(&source))
            .map(|(a, _)| a)
        {
            return Some(anime.clone());
        }

        let source_path = Path::new(&self.config.source_path).join(&source);
        let mut source_set: HashSet<String> = HashSet::new();
        if let Ok(entries) = fs::read_dir(source_path) {
            entries
                .flatten()
                .filter_map(Self::filter_file_dir)
                .for_each(|(name, _)| {
                    source_set.insert(name);
                });
        };

        if let Some(anime) = anime_cache
            .iter()
            .find(|(_, c)| c.contains_set(&source_set))
            .map(|(a, _)| a)
        {
            return Some(anime.clone());
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

            if anime_cache.contains_key(anime) {
                continue;
            }
            let tree = self.fetch_anime_cache(anime, anime_cache);
            if tree.contains(&source) {
                return Some(anime.to_string());
            }
        }

        for (anime, set) in anime_cache {
            if set.contains_set(&source_set) {
                return Some(anime.clone());
            }
        }
        None
    }

    fn fetch_anime_cache<'a>(&'a self, anime: &str, anime_cache: &'a mut CacheMap) -> &Cache {
        let cache = anime_cache.entry(anime.to_string()).or_default();
        let anime_dir = Path::new(&self.config.anime_path).join(anime);

        Self::fetch_cache(cache, anime_dir);
        cache
    }

    // 递归获取文件.
    fn fetch_cache<P: AsRef<Path>>(cache: &mut Cache, dir_path: P) {
        let Ok(entries) = fs::read_dir(dir_path) else {
            return;
        };
        // HACK: 关于 flatten 和 filter_map 的讨论:
        // https://users.rust-lang.org/t/where-is-flatten-skipping-none-documented/89255/21
        // https://github.com/rust-lang/rust-clippy/issues/9377
        // https://github.com/rust-lang/rust/pull/99230
        // https://rust.godbolt.org/z/aG444qGdW
        // filter_map(|x| x), filter_map(identity), flatten.
        entries
            .flatten()
            .filter_map(Self::filter_file_dir)
            .for_each(|(name, path)| {
                if path.as_os_str() == "" {
                    cache.insert(name);
                } else if let Some(new) = cache.insert_cache(name) {
                    Self::fetch_cache(new, path);
                }
            });
    }

    // 过滤非视频文件或者特殊文件夹.
    // 返回文件名和文件夹路径.
    fn filter_file_dir(dir_entry: DirEntry) -> Option<(String, PathBuf)> {
        let Ok(file_type) = dir_entry.file_type() else {
            return None;
        };
        let Ok(name) = dir_entry.file_name().into_string() else {
            return None;
        };

        if file_type.is_file() {
            // 排除非视频文件.
            if [".mkv", ".mp4", ".avi"]
                .iter()
                .map(|suffixes| name.ends_with(suffixes))
                .any(|x| x)
            {
                return Some((name, "".into()));
            }
        } else if file_type.is_dir() {
            // 当文件夹名称超过 20 个字符，或者以 "Season" 开头时,
            // 返回文件夹路径备用.
            if name.len() > 20 || name.to_lowercase().starts_with("season") {
                return Some((name, dir_entry.path()));
            }
        }
        None
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

#[derive(Serialize, Deserialize, Default)]
pub struct RealData {
    pub source_anime_maps: Vec<SourceAnimeMap>, // 文件夹映射.
    pub animes: Vec<String>,                    // 动漫列表.
}

impl RealData {
    // 从文件中读取数据.
    fn from_file(file: File) -> RealData {
        let mut real_data = Self::default();
        // serde_yaml::from_reader::<&File, Data> 指定泛型类型.
        if let Ok(exist_data) = serde_yaml::from_reader::<&File, RealData>(&file) {
            let maps = exist_data.source_anime_maps;
            if !maps.is_empty() {
                real_data.source_anime_maps.extend(maps);
            }
        }
        real_data
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

    // 获取 map，可以嵌套.
    fn get_map_at_indexes(&self, i: (usize, usize)) -> &SourceAnimeMap {
        match &self.source_anime_maps[i.0].file_type {
            FileType::Nesting(maps) => &maps[i.1],
            _ => &self.source_anime_maps[i.0],
        }
    }

    // 批量设置 map 的值.
    fn set_batch_map<T: Clone>(
        &mut self,
        queue: &Vec<(usize, usize, T)>,
        f: fn(&mut SourceAnimeMap, T),
    ) {
        for i in queue {
            match &mut self.source_anime_maps[i.0].file_type {
                FileType::Nesting(maps) => {
                    f(&mut maps[i.1], i.2.clone());
                }
                _ => f(&mut self.source_anime_maps[i.0], i.2.clone()),
            }
        }
    }

    // 设置索引处的 map 的 anime name.
    fn set_anime_name(&mut self, reflink_queue: &Vec<(usize, usize, String)>) {
        // fn set<T: ToString>(map: &mut SourceAnimeMap, name: T) {
        //     map.anime = name.to_string();
        // }
        let set = |map: &mut SourceAnimeMap, name: String| map.anime = name;
        self.set_batch_map::<String>(reflink_queue, set)
    }

    fn set_map_active(&mut self, successed_index: &Vec<(usize, usize, bool)>) {
        // fn set(map: &mut SourceAnimeMap, i: bool) {
        //     map.active = i;
        // }
        let set = |map: &mut SourceAnimeMap, i: bool| map.active = i;
        self.set_batch_map::<bool>(successed_index, set)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::*;

    fn get_real_data() -> RealData {
        RealData {
            source_anime_maps: vec![
                SourceAnimeMap {
                    source: "file_source".to_string(),
                    anime: "file_anime".to_string(),
                    active: true,
                    file_type: FileType::File,
                },
                SourceAnimeMap {
                    source: "dir_source".to_string(),
                    anime: "dir_anime".to_string(),
                    active: true,
                    file_type: FileType::Dir,
                },
                SourceAnimeMap {
                    source: "nesting_source".to_string(),
                    anime: "nesting_anime".to_string(),
                    active: true,
                    file_type: FileType::Nesting(vec![
                        SourceAnimeMap {
                            source: "nesting_file_source".to_string(),
                            anime: "nesting_file_anime".to_string(),
                            active: true,
                            file_type: FileType::File,
                        },
                        SourceAnimeMap {
                            source: "nesting_dir_source".to_string(),
                            anime: "nesting_dir_anime".to_string(),
                            active: true,
                            file_type: FileType::Dir,
                        },
                    ]),
                },
            ],
            animes: Vec::new(),
        }
    }

    const ANIME_1: &str = "AIR [青空]";
    const ANIME_2: &str = "Just Because!";
    const ANIME_3: &str = "true tears [真实之泪]";
    const ANIME_4: &str = "君の名は。 [你的名字。]";
    const ANIME_4_SUB: [&str; 5] = [
        "SPs",
        "Season 01",
        "[VCB-Studio] Kimi no Na wa [Ma10p_1080p]",
        "[VCB-Studio] Kimi no Na wa [Ma10p_1080p][x265_flac].mkv",
        "readme about WebP.txt",
    ];

    fn temp_anime_dir(tep_dir: &TempDir) -> Result<&TempDir, Box<dyn Error>> {
        let binding = tep_dir.path().join("anime");
        let anime_path = binding.as_path();
        fs::create_dir(anime_path)?;
        for i in [ANIME_1, ANIME_2, ANIME_3] {
            fs::create_dir(anime_path.join(i))?;
        }
        let binding = anime_path.join(ANIME_4);
        let nesting_path = binding.as_path();
        fs::create_dir(nesting_path)?;
        for i in ANIME_4_SUB {
            if i.ends_with(".mkv") | i.ends_with(".txt") {
                File::create(nesting_path.join(i))?;
            } else {
                fs::create_dir(nesting_path.join(i))?;
            }
        }
        Ok(tep_dir)
    }

    mod data_tests {
        use super::*;

        fn create_data() -> Data {
            Data {
                data: get_real_data(),
                source_map: HashMap::new(),
                config: Config::new([].into_iter()),
            }
        }

        #[test]
        fn fetch_anime_cache() {
            let tep_dir = tempdir_in("./").unwrap();
            temp_anime_dir(&tep_dir).unwrap();
            let mut data = create_data();
            data.config.anime_path = tep_dir.path().join("anime").to_str().unwrap().to_string();
            let mut anime_cache = CacheMap::default();
            let set = data.fetch_anime_cache(ANIME_4, &mut anime_cache);
            assert_eq!(
                set,
                &Cache::Some(CacheMap::new(HashMap::from([
                    (ANIME_4_SUB[3].to_string(), Cache::None),
                    (ANIME_4_SUB[2].to_string(), Cache::default()),
                    (ANIME_4_SUB[1].to_string(), Cache::default()),
                ]))),
                "set is {:?}",
                set
            );
            assert_eq!(
                anime_cache,
                CacheMap::from([(
                    ANIME_4.to_string(),
                    Cache::Some(CacheMap::new(HashMap::from([
                        (ANIME_4_SUB[1].to_string(), Cache::default()),
                        (ANIME_4_SUB[2].to_string(), Cache::default()),
                        (ANIME_4_SUB[3].to_string(), Cache::None),
                    ])))
                )])
            );
        }
    }

    mod read_data_tests {
        use super::*;

        #[test]
        fn get_map_at_indexes() {
            let real_data = get_real_data();
            assert_eq!(
                real_data.get_map_at_indexes((0, 0)),
                &real_data.source_anime_maps[0]
            );
            let FileType::Nesting(nesting) = &real_data.source_anime_maps[2].file_type else {
                panic!("")
            };
            assert_eq!(real_data.get_map_at_indexes((2, 1)), &nesting[1]);
        }

        #[test]
        fn set_anime_name() {
            let mut real_data = get_real_data();
            real_data.set_anime_name(&vec![(0, 0, "new_anime".to_string())]);
            assert_eq!(
                real_data.source_anime_maps[0].anime,
                "new_anime".to_string()
            );
            real_data.set_anime_name(&vec![(2, 1, "new_nesting_anime".to_string())]);
            let FileType::Nesting(nesting) = &real_data.source_anime_maps[2].file_type else {
                panic!("")
            };
            assert_eq!(nesting[1].anime, "new_nesting_anime".to_string());
        }

        #[test]
        fn set_map_active() {
            let mut real_data = get_real_data();

            real_data.set_map_active(&vec![(0, 0, false)]);
            assert!(!real_data.source_anime_maps[0].active);

            real_data.set_map_active(&vec![(2, 1, false)]);
            let FileType::Nesting(nesting) = &real_data.source_anime_maps[2].file_type else {
                panic!("")
            };
            assert!(!nesting[1].active);

            real_data.set_map_active(&vec![(0, 0, true)]);
            assert!(real_data.source_anime_maps[0].active);
        }
    }
}
