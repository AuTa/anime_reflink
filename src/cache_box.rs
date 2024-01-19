use std::collections::{hash_map, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum CacheBox {
    None,
    Map(HashMap<String, Box<CacheBox>>),
}

impl CacheBox {
    // 嵌套的所有值都需要被查找.
    pub fn contains(&self, source: &str) -> bool {
        matches!(self, CacheBox::Map(map) if map.contains_key(source) || map.values().any(|cache| cache.contains(source)))
    }

    pub fn contains_set(&self, set: &HashSet<String>) -> bool {
        set.iter().map(|source| self.contains(source)).any(|b| b)
    }

    pub fn insert(&mut self, key: &str) {
        if let CacheBox::Map(map) = self {
            map.insert(key.to_owned(), Box::new(CacheBox::None));
        }
    }

    pub fn insert_cache(&mut self, key: &str) -> Option<&mut Self> {
        if let CacheBox::Map(map) = self {
            map.insert(key.to_owned(), Box::new(CacheBox::None));
            return map.get_mut(key).map(|cache| cache.as_mut()); // 重新绑定生命周期.
        }
        None
    }

    pub fn iter(&self) -> Iter {
        match self {
            CacheBox::None => Iter {
                inner: Item { opt: None },
            },
            CacheBox::Map(map) => Iter {
                inner: Item {
                    opt: Some(map.iter()),
                },
            },
        }
    }

    pub fn iter_mut(&mut self) -> IterMut {
        match self {
            CacheBox::None => IterMut {
                inner: ItemMut { opt: None },
            },
            CacheBox::Map(map) => IterMut {
                inner: ItemMut {
                    opt: Some(map.iter_mut()),
                },
            },
        }
    }
}

// 实现 From 特性.
impl<const N: usize> From<[(&str, CacheBox); N]> for CacheBox {
    fn from(arr: [(&str, CacheBox); N]) -> Self {
        if arr.is_empty() {
            return CacheBox::None;
        }
        CacheBox::Map(HashMap::<String, Box<CacheBox>>::from(
            arr.map(|(key, cache)| (key.to_owned(), Box::new(cache))),
        ))
    }
}

pub struct Iter<'a> {
    inner: Item<'a>,
}

pub struct Item<'a> {
    opt: Option<hash_map::Iter<'a, String, Box<CacheBox>>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a String, &'a CacheBox);

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
    opt: Option<hash_map::IterMut<'a, String, Box<CacheBox>>>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = (&'a String, &'a mut CacheBox);

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
    opt: Option<hash_map::IntoIter<String, Box<CacheBox>>>,
}

impl Iterator for IntoIter {
    type Item = (String, CacheBox);

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner.opt {
            None => None,
            Some(opt) => opt.next().map(|(key, cache)| (key, *cache)),
        }
    }
}

// 实现 for &CacheBox.
impl<'a> IntoIterator for &'a CacheBox {
    type Item = (&'a String, &'a CacheBox);
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// 实现 for &mut CacheBox.
impl<'a> IntoIterator for &'a mut CacheBox {
    type Item = (&'a String, &'a mut CacheBox);
    type IntoIter = IterMut<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

// 实现 for CacheBox.
impl IntoIterator for CacheBox {
    type Item = (String, CacheBox);
    type IntoIter = IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        match self {
            CacheBox::None => IntoIter {
                inner: IntoIterItem { opt: None },
            },
            CacheBox::Map(map) => IntoIter {
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

    fn get_cache() -> CacheBox {
        CacheBox::from([
            ("a", CacheBox::None),
            ("b", CacheBox::None),
            (
                "c",
                *Box::new(CacheBox::from([
                    ("c.0", CacheBox::None),
                    ("c.1", CacheBox::None),
                ])),
            ),
        ])
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
            CacheBox::from([("c.0", CacheBox::None), ("c.1", CacheBox::None)])
        );
    }

    #[test]
    fn iter_mut() {
        let mut cache = get_cache();
        let mut iter_mut = cache.iter_mut();
        let item = iter_mut.next().unwrap();
        assert!(["a", "b", "c"].contains(&item.0.as_str()));
        *item.1 = CacheBox::from([("d", CacheBox::None)]);
        assert_ne!(cache, get_cache());
    }
}
