use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, RwLock, RwLockReadGuard, RwLockWriteGuard};
use rand::random;

#[derive(Debug)]
enum LockGuard<'a, K, V>{
    Read(RwLockReadGuard<'a, ConcurrentBSTMapInternal<K, V>>),
    Write(RwLockWriteGuard<'a, ConcurrentBSTMapInternal<K, V>>)
}

impl<'a, K, V> Deref for LockGuard<'a, K, V>{
    type Target = ConcurrentBSTMapInternal<K, V>;

    fn deref(&self) -> &Self::Target {
        match self{
            LockGuard::Read(lock) => lock,
            LockGuard::Write(lock) => lock
        }
    }
}

const MIN_LIST_LENGTH: usize = 1024;

#[derive(Debug)]
pub struct ConcurrentBSTMap<K, V>(LazyLock<RwLock<ConcurrentBSTMapInternal<K, V>>>);

#[derive(Debug)]
struct ConcurrentBSTMapInternal<K, V>{
    random_bytes: [u8; 32],
    no_elements: AtomicUsize,
    root_node_key: Option<K>,
    list: Vec<RwLock<Vec<ConcurrentBSTMapNode<K, V>>>>
}

#[derive(Debug, Clone, Copy)]
struct ConcurrentBSTMapNode<K, V>{
    key: K,
    value: V,
    child_keys: [Option<(K, bool)>; 2]
}

impl<K: Copy, V> ConcurrentBSTMapNode<K, V>{
    const fn new(key: K, value: V) -> Self{
        Self{
            key,
            value,
            child_keys: [None; 2]
        }
    }
}

impl<K: Copy + Ord + Hash, V: Copy> ConcurrentBSTMapInternal<K, V>{
    fn new() -> Self{
        Self{
            random_bytes: random(),
            no_elements: AtomicUsize::new(0),
            root_node_key: None,
            list: {
                let mut new_vec = Vec::new();
                for _ in 0..MIN_LIST_LENGTH {new_vec.push(RwLock::new(Vec::new()))}
                new_vec
            }
        }
    }

    fn get_index(&self, key: K) -> usize{
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        self.random_bytes.hash(&mut hasher);
        (hasher.finish() % (self.list.len() as u64)) as usize
    }
}

impl<K: Copy + Ord + Hash, V: Copy> ConcurrentBSTMap<K, V>{

    fn child_index(current: &K, target: &K) -> usize{
        if current < target {0} else {1}
    }
    
    pub fn clear(&self){
        *self.0.write().unwrap() = ConcurrentBSTMapInternal::new();
    }
    
    pub fn contains_key(&self, key: K) -> bool{
        self.0.read().map(|read_lock| {
            read_lock.list[read_lock.get_index(key)].read().unwrap().iter().position(|x| x.key == key).is_some()
        }).unwrap()
    }
    
    pub fn get(&self, key: K) -> Option<V>{
        self.0.read().map(|read_lock| {
            read_lock.list[read_lock.get_index(key)].read().unwrap().iter().find(|x| x.key == key).map(|x| x.value)
        }).unwrap()
    }

    pub fn get_min(&self) -> Option<(K,V)>{
        self.get_min_or_max(0)
    }

    pub fn get_max(&self) -> Option<(K,V)>{
        self.get_min_or_max(1)
    }

    fn get_min_or_max(&self, child_index: usize) -> Option<(K, V)>{
        self.0.read().map(|read_lock| {

            let mut current_key = match read_lock.root_node_key {
                Some(root_node_key) => root_node_key,
                None => return None
            };
            let mut current_key_hash = read_lock.get_index(current_key);
            let mut current_best = None;

            loop{
                match read_lock.list[current_key_hash].read().map(|inner_read_lock| {
                    match inner_read_lock.iter().position(|x| x.key == current_key) {
                        Some(index) => {
                            current_best = Some((inner_read_lock[index].key, inner_read_lock[index].value));
                            match inner_read_lock[index].child_keys[child_index]{
                                Some(left_key) => {
                                    current_key = left_key.0;
                                    current_key_hash = read_lock.get_index(current_key);
                                    None
                                }
                                None => Some(current_best)
                            }
                        }
                        None => Some(current_best)
                    }
                }).unwrap(){
                    None => (),
                    Some(x) => return x
                }
            }
        }).unwrap()
    }
    
    pub fn get_or_closest_by_key(&self, key: K, include_key: bool) -> Option<(K,V)>{
        
    }

    pub fn insert_or_update(&self, key: K, value: V) -> bool{
        self.insert_or_update_if(key, value, |_,_| true)
    }

    pub fn insert_or_update_if(&self, key: K, value: V, should_update: impl Fn(&V, &V) -> bool) -> bool{
        
        let inner_function = |root_node_key, lock_guard: LockGuard<K, V>| {

            let mut current_key = root_node_key;
            let mut current_key_hash = lock_guard.get_index(current_key);
            
            loop{
                match lock_guard.list[current_key_hash].write().map(|mut write_lock| {
                    match write_lock.iter().position(|x| x.key == current_key) {
                        Some(index) => {
                            if current_key == key{
                                //update
                                Some((false,
                                    if should_update(&write_lock[index].value, &value){
                                        write_lock[index].value = value;
                                        true
                                    }
                                    else {false},
                                ))
                            }
                            else{
                                current_key = *write_lock[index].child_keys[Self::child_index(&current_key, &key)].get_or_insert(key);
                                current_key_hash = lock_guard.get_index(current_key);
                                None
                            }
                        }
                        None => {
                            if current_key == key {
                                //insert
                                write_lock.push(ConcurrentBSTMapNode::new(key, value));
                                Some((true, lock_guard.no_elements.fetch_add(1, Ordering::Relaxed) >= lock_guard.list.len()))
                            }
                            else {None} //wait for it to be created by another insert
                        }
                    }
                }).unwrap(){
                    None => (),
                    Some(result) => return result
                }
            }
        };

        match loop{
            match self.0.read().map(|read_lock| {
                read_lock.root_node_key.map(|x| inner_function(x, LockGuard::Read(read_lock)))
            }).unwrap(){
                Some(result) => break result,
                None => ()
            }
            match self.0.write().map(|mut write_lock| {
                match write_lock.root_node_key{
                    Some(_) => None,
                    None => Some(inner_function(*write_lock.root_node_key.insert(key), LockGuard::Write(write_lock)))
                }
            }).unwrap(){
                Some(result) => break result,
                None => (),
            }
        }{
            (false, updated) => updated,
            (true, needs_resizing) => {
                if needs_resizing{
                    self.0.write().map(|mut write_lock| {
                        let old_list_length = write_lock.list.len();
                        let no_elements = write_lock.no_elements.load(Ordering::Relaxed);
                        let mut new_list_length = old_list_length;
                        while no_elements >= new_list_length {new_list_length *= 2}
                        if new_list_length > old_list_length{
                            for _ in old_list_length..new_list_length {write_lock.list.push(RwLock::new(Vec::new()))}
                            for i in 0..old_list_length{
                                for entry in write_lock.list[i].write().map(|mut inner_lock| {
                                    let old_entries = inner_lock.clone();
                                    *inner_lock = Vec::new();
                                    old_entries
                                }).unwrap(){
                                    write_lock.list[write_lock.get_index(entry.key)].write().unwrap().push(entry)
                                }
                            }
                        }
                    }).unwrap();
                }
                true
            }
        }
    }
    
    pub fn is_empty(&self) -> bool{
        self.len() == 0
    }
    
    pub fn len(&self) -> usize{
        self.0.read().unwrap().no_elements.load(Ordering::Relaxed)
    }

    pub const fn new() -> Self{
        Self(LazyLock::new(|| RwLock::new(ConcurrentBSTMapInternal::new())))
    }
    
    pub fn remove(&self, key: K){
        self.remove_if(key, |_| true)
    }
    
    pub fn remove_if(&self, key: K, should_remove: impl Fn(&V) -> bool){
        //need to find parent node
        //then find replacement for node and set the parent node child key as the new node key
        self.0.write().map(|mut write_lock| {
            let mut current_key = match write_lock.root_node_key {
                Some(root_node_key) => root_node_key,
                None => return
            };
            let mut current_key_hash = write_lock.get_index(current_key);

            loop {
                if write_lock.list[current_key_hash].write().map(|mut inner_write_lock| {
                    match inner_write_lock.iter().position(|x| x.key == current_key) {
                        Some(index) => {
                            if current_key == key {
                                if should_remove(&inner_write_lock[index].value) {
                                    inner_write_lock.swap_remove(index);
                                }
                                true
                            } else {
                                match inner_write_lock[index].child_keys[Self::child_index(&current_key, &key)]{
                                    Some(x) => {
                                        current_key = x;
                                        current_key_hash = write_lock.get_index(current_key);
                                        false
                                    },
                                    None => true
                                }
                            }
                        }
                        None => {
                            if current_key == key {
                                //insert
                                inner_write_lock.push(ConcurrentBSTMapNode::new(key, value));
                                Some((true, lock_guard.no_elements.fetch_add(1, Ordering::Relaxed) >= lock_guard.list.len()))
                            } else { None } //wait for it to be created by another insert
                        }
                    }
                }).unwrap() {return}
            }
        }).unwrap();
        /*if self.inner.read().map(|read_lock| {
            read_lock.list[get_index(key, read_lock.list.len())].write().map(|mut write_lock| {
                match write_lock.iter().position(|x| x.0 == key){
                    Some(index) => {
                        if should_remove(&write_lock[index].1) {
                            write_lock.swap_remove(index);
                            (read_lock.no_elements.fetch_sub(1, Ordering::Relaxed) < (read_lock.list.len() / 2)) && (read_lock.list.len() > MIN_LIST_LENGTH)
                        }
                        else {false}
                    }
                    None => false
                }
            }).unwrap()
        }).unwrap(){
            self.inner.write().map(|mut write_lock| {
                let old_list_length = write_lock.list.len();
                let no_elements = write_lock.no_elements.load(Ordering::Relaxed);
                let mut new_list_length = old_list_length;
                while (new_list_length > MIN_LIST_LENGTH) && (no_elements < (new_list_length / 2)) {new_list_length /= 2}
                if new_list_length < old_list_length{
                    for i in 0..old_list_length{
                        for entry in write_lock.list[i].write().map(|mut inner_lock| {
                            let old_entries = inner_lock.clone();
                            *inner_lock = Vec::new();
                            old_entries
                        }).unwrap(){
                            write_lock.list[get_index(entry.0, new_list_length)].write().unwrap().push(entry)
                        }
                    }
                    for i in (new_list_length..old_list_length).rev() {_ = write_lock.list.swap_remove(i)}
                }
            }).unwrap();
        }*/
    }
}