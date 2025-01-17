use std::{hash::{DefaultHasher, Hash, Hasher}, sync::{atomic::AtomicUsize, LazyLock, Mutex, RwLock}};

pub struct ConcurrentBSTMap<K, V>{
    inner: LazyLock<RwLock<ConcurrentBSTMapInternal<K, V>>>
}

struct ConcurrentBSTMapInternal<K, V>{
    root_node: Option<K>,
    no_elements: AtomicUsize,
    list: Vec<Mutex<Vec<ConcurrentBSTMapEntry<K, V>>>>
}

impl<K, V> ConcurrentBSTMapInternal<K, V>{

    fn new() -> Self{
        Self{
            root_node: None,
            no_elements: AtomicUsize::new(0),
            list: {
                let mut new_vec = Vec::new();
                for _ in 0..1024 {new_vec.push(Mutex::new(Vec::new()))}
                new_vec
            }
        }
    }
}

struct ConcurrentBSTMapEntry<K, V>{
    key: K,
    value: V,
    child_keys: [Option<K>; 2]
}

fn get_index<T: Hash>(item: T, max_index: usize) -> usize{
    let mut hasher = DefaultHasher::new();
    item.hash(&mut hasher);
    (hasher.finish() % (max_index as u64)) as usize
}

impl<K: Copy + Hash + Ord, V: Copy> ConcurrentBSTMap<K, V>{

    pub const fn new() -> Self{
        Self { 
            inner: LazyLock::new(|| RwLock::new(ConcurrentBSTMapInternal::new())) 
        }
    }

    fn get_child_index(target: K, current: K) -> usize{
        if target < current {0} else {1}
    }

    pub fn get(&self, key: K) -> Option<V>{
        self.inner.read().map(|read_lock| {
            let list_length = read_lock.list.len();
            let mut current_key = match read_lock.root_node{
                Some(result) => result,
                None => return None
            };
            let mut result = None;
            loop{
                match result{
                    Some(x) => return x,
                    None => ()
                }
                read_lock.list[get_index(current_key, list_length)].lock().map(|mutex_lock| {
                    match mutex_lock.iter().find(|x| x.key == current_key){
                        Some(entry) => {
                            if current_key == key {result = Some(Some(entry.value))}
                            else {
                                match entry.child_keys[Self::get_child_index(key, current_key)]{
                                    Some(next_key) => current_key = next_key,
                                    None => result = Some(None)
                                }
                            }
                        }
                        None => result = Some(None)
                    }
                }).unwrap();
            }
        }).unwrap()
    }
}