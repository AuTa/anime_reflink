use std::collections::{
    hash_map::{Entry, IntoIter, Iter, IterMut},
    HashMap, HashSet,
};

#[derive(Debug, PartialEq)]
pub enum Cache {
    None,
    Some(CacheMap),
}

impl Cache {
    pub fn contains(&self, source: &str) -> bool {
        matches!(self, Cache::Some(cache) if cache.contains(source))
    }

    pub fn contains_set(&self, set: &HashSet<String>) -> bool {
        set.iter().map(|source| self.contains(source)).any(|b| b)
    }

    pub fn insert(&mut self, key: String) {
        if let Self::Some(cache) = self {
            cache.map.insert(key, Self::None);
        };
    }

    pub fn insert_cache(&mut self, key: String) -> Option<&mut Self> {
        if let Self::Some(cache) = self {
            cache.map.insert(key.clone(), Self::default());
            return cache.map.get_mut(&key); // 重新绑定生命周期.
        }
        None
    }
}

impl Default for Cache {
    fn default() -> Self {
        Cache::Some(CacheMap::default())
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct CacheMap {
    map: HashMap<String, Cache>,
}

impl CacheMap {
    pub fn new(map: HashMap<String, Cache>) -> CacheMap {
        CacheMap { map }
    }

    // 嵌套的所有值都需要被查找.
    pub fn contains(&self, value: &str) -> bool {
        if self.map.contains_key(value) {
            true
        } else {
            self.map.values().any(|cache| cache.contains(value))
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    pub fn iter(&self) -> Iter<String, Cache> {
        self.map.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<String, Cache> {
        self.map.iter_mut()
    }

    pub fn entry(&mut self, key: String) -> Entry<'_, String, Cache> {
        self.map.entry(key)
    }

    pub fn insert(&mut self, key: String, value: Cache) {
        self.map.insert(key, value);
    }
}

// 实现 From 特性.
impl<const N: usize> From<[(String, Cache); N]> for CacheMap {
    fn from(arr: [(String, Cache); N]) -> Self {
        Self {
            map: HashMap::from(arr),
        }
    }
}

// 实现 for &CacheMap.
impl<'a> IntoIterator for &'a CacheMap {
    type Item = (&'a String, &'a Cache);
    type IntoIter = Iter<'a, String, Cache>;

    #[inline]
    fn into_iter(self) -> Iter<'a, String, Cache> {
        self.iter()
    }
}

// 实现 for &mut CacheMap.
impl<'a> IntoIterator for &'a mut CacheMap {
    type Item = (&'a String, &'a mut Cache);
    type IntoIter = IterMut<'a, String, Cache>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

// 实现 for CacheMap.
impl IntoIterator for CacheMap {
    type Item = (String, Cache);
    type IntoIter = IntoIter<String, Cache>;

    #[inline]
    fn into_iter(self) -> IntoIter<String, Cache> {
        self.map.into_iter()
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn contains_set() {
        let cache = Cache::Some(CacheMap::new(HashMap::from([
            ("a".to_string(), Cache::None),
            (
                "b".to_string(),
                Cache::Some(CacheMap::new(HashMap::from([(
                    "b.0".to_string(),
                    Cache::None,
                )]))),
            ),
        ])));

        assert!(cache.contains_set(&HashSet::from(["a".to_string()])));
        assert!(cache.contains_set(&HashSet::from(["b.0".to_string()])));
        assert!(!cache.contains_set(&HashSet::from(["c".to_string()])));
        assert!(cache.contains_set(&HashSet::from(["c".to_string(), "b.0".to_string()])));
    }

    #[test]
    fn contains() {
        let mut anime_cache = CacheMap::default();
        assert!(!anime_cache.contains("a"));

        anime_cache.map.insert("a".to_string(), Cache::None);
        anime_cache.map.insert(
            "b".to_string(),
            Cache::Some(CacheMap::new(HashMap::from([(
                "b.0".to_string(),
                Cache::None,
            )]))),
        );
        assert!(anime_cache.contains("a"));
        assert!(anime_cache.contains("b"));
        assert!(anime_cache.contains("b.0"));
        assert!(!anime_cache.contains("c"));
    }
}
