use serde::{Deserialize, Serialize};

// 文件类型.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum FileType {
    File,                         // 普通文件.
    Dir,                          // 文件夹.
    Other,                        // ".parts" 文件.
    Nesting(Vec<SourceAnimeMap>), // 文件夹里还是文件夹.
}

impl FileType {
    pub fn is_other(&self) -> bool {
        matches!(self, FileType::Other)
    }
}

// 文件夹映射.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SourceAnimeMap {
    pub source: String,      // 源文件夹地址.
    pub anime: String,       // 目标文件夹地址.
    pub active: bool,        // 是否激活.
    pub file_type: FileType, // 文件类型.
}

impl SourceAnimeMap {
    pub fn active(&self) -> bool {
        self.active && !self.file_type.is_other()
    }
}
