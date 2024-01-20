use std::collections::{hash_map, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum Cache {
    None,
    Map(HashMap<String, Box<Cache>>),
}

impl Cache {
    // 嵌套的所有值都需要被查找.
    pub fn contains(&self, source: &str) -> bool {
        matches!(self, Cache::Map(map) if map.contains_key(source) || map.values().any(|cache| cache.contains(source)))
    }

    pub fn contains_set(&self, set: &HashSet<String>) -> bool {
        set.iter().map(|source| self.contains(source)).any(|b| b)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        matches!(self, Cache::Map(map) if map.contains_key(key))
    }

    pub fn insert(&mut self, key: &str, value: Cache) {
        if let Cache::Map(map) = self {
            map.insert(key.to_owned(), Box::new(value));
        }
    }

    pub fn insert_none(&mut self, key: &str) {
        if let Cache::Map(map) = self {
            map.insert(key.to_owned(), Box::new(Cache::None));
        }
    }

    pub fn insert_default(&mut self, key: &str) -> Option<&mut Self> {
        if let Cache::Map(map) = self {
            map.insert(key.to_owned(), Box::<Cache>::default());
            return map.get_mut(key).map(|cache| cache.as_mut()); // 重新绑定生命周期.
        }
        None
    }

    pub fn iter(&self) -> Iter {
        match self {
            Cache::None => Iter {
                inner: Item { opt: None },
            },
            Cache::Map(map) => Iter {
                inner: Item {
                    opt: Some(map.iter()),
                },
            },
        }
    }

    pub fn iter_mut(&mut self) -> IterMut {
        match self {
            Cache::None => IterMut {
                inner: ItemMut { opt: None },
            },
            Cache::Map(map) => IterMut {
                inner: ItemMut {
                    opt: Some(map.iter_mut()),
                },
            },
        }
    }

    pub fn entry(&mut self, key: &str) -> Option<hash_map::Entry<'_, String, Box<Cache>>> {
        match self {
            Cache::None => None,
            Cache::Map(map) => Some(map.entry(key.to_owned())),
        }
    }
}

// 实现 From 特性.
impl<const N: usize> From<[(&str, Cache); N]> for Cache {
    fn from(arr: [(&str, Cache); N]) -> Self {
        if arr.is_empty() {
            return Cache::None;
        }
        Cache::Map(HashMap::<String, Box<Cache>>::from(
            arr.map(|(key, cache)| (key.to_owned(), Box::new(cache))),
        ))
    }
}

impl Default for Cache {
    fn default() -> Self {
        Cache::Map(HashMap::default())
    }
}
pub struct Iter<'a> {
    inner: Item<'a>,
}

pub struct Item<'a> {
    opt: Option<hash_map::Iter<'a, String, Box<Cache>>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a String, &'a Cache);

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner.opt {
            None => None,
            Some(opt) => opt.next().map(|(key, cache)| (key, cache.as_ref())),
        }
    }
}

pub struct IterMut<'a> {
    inner: ItemMut<'a>,
}

pub struct ItemMut<'a> {
    opt: Option<hash_map::IterMut<'a, String, Box<Cache>>>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = (&'a String, &'a mut Cache);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner.opt {
            None => None,
            Some(opt) => opt.next().map(|(key, cache)| (key, cache.as_mut())),
        }
    }
}

pub struct IntoIter {
    inner: IntoIterItem,
}

pub struct IntoIterItem {
    opt: Option<hash_map::IntoIter<String, Box<Cache>>>,
}

impl Iterator for IntoIter {
    type Item = (String, Cache);

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner.opt {
            None => None,
            Some(opt) => opt.next().map(|(key, cache)| (key, *cache)),
        }
    }
}

// 实现 for &Cache.
impl<'a> IntoIterator for &'a Cache {
    type Item = (&'a String, &'a Cache);
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// 实现 for &mut Cache.
impl<'a> IntoIterator for &'a mut Cache {
    type Item = (&'a String, &'a mut Cache);
    type IntoIter = IterMut<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

// 实现 for Cache.
impl IntoIterator for Cache {
    type Item = (String, Cache);
    type IntoIter = IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        match self {
            Cache::None => IntoIter {
                inner: IntoIterItem { opt: None },
            },
            Cache::Map(map) => IntoIter {
                inner: IntoIterItem {
                    opt: Some(map.into_iter()),
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_cache() -> Cache {
        Cache::from([
            ("a", Cache::None),
            ("b", Cache::None),
            (
                "c",
                *Box::new(Cache::from([("c.0", Cache::None), ("c.1", Cache::None)])),
            ),
        ])
    }

    #[test]
    fn contains() {
        let mut cache = Cache::default();
        assert!(!cache.contains("a"));

        cache.insert_none("a");
        cache.insert("b", Cache::from([("b.0", Cache::None)]));
        assert!(cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("b.0"));
        assert!(!cache.contains("c"));
    }

    #[test]
    fn contains_set() {
        let cache = Cache::from([
            ("a", Cache::None),
            ("b", Cache::from([("b.0", Cache::None)])),
        ]);

        assert!(cache.contains_set(&HashSet::from(["a".to_string()])));
        assert!(cache.contains_set(&HashSet::from(["b.0".to_string()])));
        assert!(!cache.contains_set(&HashSet::from(["c".to_string()])));
        assert!(cache.contains_set(&HashSet::from(["c".to_string(), "b.0".to_string()])));
    }

    #[test]
    fn iter() {
        let cache = get_cache();
        let iter = cache.iter();
        let mut sort: Vec<_> = iter.map(|(k, _)| k).collect();
        sort.sort();
        assert_eq!(sort, ["a", "b", "c"]);
        let mut iter = cache.iter();
        let cb = iter.find_map(|(k, v)| if k == "c" { Some(v) } else { None });
        assert_eq!(
            *cb.unwrap(),
            Cache::from([("c.0", Cache::None), ("c.1", Cache::None)])
        );
    }

    #[test]
    fn iter_mut() {
        let mut cache = get_cache();
        let mut iter_mut = cache.iter_mut();
        let item = iter_mut.next().unwrap();
        assert!(["a", "b", "c"].contains(&item.0.as_str()));
        *item.1 = Cache::from([("d", Cache::None)]);
        assert_ne!(cache, get_cache());
    }
}
