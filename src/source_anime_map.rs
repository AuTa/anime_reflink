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

    pub fn anime(&self) -> &str {
        &self.anime
    }

    pub fn set_anime(&mut self, anime: Value<&String>) -> Result<(), &'static str> {
        let f_index = |x: &mut Self, _: usize, v: &String| {
            let FileType::Nesting(_) = &x.file_type else {
                return;
            };
            if !x.anime.contains(v) {
                if !x.anime.is_empty() {
                    x.anime.push_str(", ")
                }
                x.anime.push_str(v);
            }
        };
        self.set_value(anime, |x, f| x.anime = f.to_owned(), f_index)
    }

    pub fn set_active(&mut self, active: Value<bool>) -> Result<(), &'static str> {
        let f_index = |x: &mut Self, _: usize, _: bool| {
            let FileType::Nesting(maps) = &x.file_type else {
                return;
            };
            x.active = maps.iter().any(|x| x.active);
        };
        self.set_value(active, |x, f| x.active = f, f_index)
    }

    // 给字段设置 value 的通用方法.
    // 可以使用泛型来实现.
    // f_base 是字段的基础设置方法.
    // f_index 是如果是嵌套字段, 需要额外的代码.
    fn set_value<T: Copy>(
        &mut self,
        value: Value<T>,
        f_base: fn(&mut Self, T),
        f_index: fn(&mut Self, usize, T),
    ) -> Result<(), &'static str> {
        match value {
            Value::Base(x) => f_base(self, x),
            Value::Index((i, x)) => match self.file_type {
                FileType::Nesting(ref mut maps) => {
                    maps[i].set_value(Value::Base(x), f_base, f_index)?;
                    f_index(self, i, x);
                }
                _ => {
                    if i == 0 {
                        f_base(self, x);
                    } else {
                        return Err("set value error. file_type isn't nesting.");
                    }
                }
            },
        }
        Ok(())
    }
}

pub enum Value<T> {
    Base(T),
    Index((usize, T)),
}
