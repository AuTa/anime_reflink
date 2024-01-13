use serde::{Deserialize, Serialize};

// 文件类型.
#[derive(Serialize, Deserialize, Debug)]
pub enum FileType {
    File,  // 普通文件.
    Dir,   // 文件夹.
    Other, // ".parts" 文件.
}

// 文件夹映射.
#[derive(Serialize, Deserialize, Debug)]
pub struct SourceAnimeMap {
    pub source: String,      // 源文件夹地址.
    pub anime: String,       // 目标文件夹地址.
    pub active: bool,        // 是否激活.
    pub file_type: FileType, // 文件类型.
}
