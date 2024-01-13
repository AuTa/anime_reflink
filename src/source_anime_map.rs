use serde::{Deserialize, Serialize};

// 文件类型.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FileType {
    File,                         // 普通文件.
    Dir,                          // 文件夹.
    Other,                        // ".parts" 文件.
    Nesting(Vec<SourceAnimeMap>), // 文件夹里还是文件夹.
}

// 实现 PartialEq trait.
impl PartialEq for FileType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // 嵌套类型先比较长度，再比较内容.
            (FileType::Nesting(x), FileType::Nesting(y)) => {
                if x.len() != y.len() {
                    false
                } else {
                    x.iter().eq(y.iter()) // 迭代器比较.
                }
            }
            // 其它类型直接返回比较结果
            (x, y) => x == y,
        }
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
