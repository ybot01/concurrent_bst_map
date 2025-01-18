use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug)]
pub struct ConcurrentBSTMap<K, V>{
    inner: LazyLock<RwLock<ConcurrentBSTMapInternal<K, V>>>
}

#[derive(Debug)]
struct ConcurrentBSTMapInternal<K, V>{
    root_node_key: Option<K>,
    no_elements: AtomicUsize,
    list: Vec<Mutex<Vec<ConcurrentBSTMapEntry<K, V>>>>
}

impl<K, V> ConcurrentBSTMapInternal<K, V>{

    fn new() -> Self{
        Self{
            root_node_key: None,
            no_elements: AtomicUsize::new(0),
            list: {
                let mut new_vec = Vec::new();
                for _ in 0..1024 {new_vec.push(Mutex::new(Vec::new()))}
                new_vec
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
enum EntryLock{
    Read(usize),
    AwaitingWrite(usize)
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
struct ConcurrentBSTMapEntry<K, V>{
    key: K,
    value: V,
    child_keys: [Option<(K, EntryLock)>; 2]
}

impl<K: Copy, V> ConcurrentBSTMapEntry<K, V>{
    const fn new(key: K, value: V) -> Self{
        Self{
            key,
            value,
            child_keys: [None; 2]
        }
    }
}

fn get_index<T: Hash>(item: T, max_index: usize) -> usize{
    let mut hasher = DefaultHasher::new();
    item.hash(&mut hasher);
    (hasher.finish() % (max_index as u64)) as usize
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
enum InsertResult{
    Updated(bool),
    Inserted(bool)
}

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

impl<K: Copy + Ord + Hash, V: Copy> ConcurrentBSTMap<K, V>{

    pub fn clear(&self){
        *self.inner.write().unwrap() = ConcurrentBSTMapInternal::new();
    }
    
    pub fn contains_key(&self, key: K) -> bool{
        self.inner.read().map(|read_lock| {
            read_lock.list[get_index(key, read_lock.list.len())].lock().unwrap().iter()
                .position(|x| x.key == key).is_some()
        }).unwrap()
    }
    
    pub fn get(&self, key: K) -> Option<V>{
        self.inner.read().map(|read_lock| {
            read_lock.list[get_index(key, read_lock.list.len())].lock().unwrap().iter()
                .find(|x| x.key == key).map(|x| x.value)
        }).unwrap()
    }
    
    fn get_child_index(target: K, current: K) -> usize{
        if target < current {0} else {1}
    }

    pub fn insert_or_update(&self, key: K, value: V) -> bool{
        self.insert_or_update_if(key, value, |_,_| true)
    }

    pub fn insert_or_update_if(&self, key: K, value: V, should_update: impl Fn(&V, &V) -> bool) -> bool{
        loop{
            match self.inner.read().map(|read_lock| {
                let mut current_key = match read_lock.root_node_key{
                    Some(x) => x,
                    None => return None
                };
                let list_length = read_lock.list.len();
                let mut current_key_index = (current_key, get_index(current_key, list_length));
                loop {
                    if current_key_index.0 != current_key {current_key_index = (current_key, get_index(current_key, list_length))}
                    match read_lock.list[current_key_index.1].lock().map(|mut mutex_lock| {
                        match mutex_lock.iter().position(|x| x.key == current_key) {
                            Some(index) => {
                                if current_key == key {
                                    //update
                                    Some(InsertResult::Updated(
                                        if should_update(&mutex_lock[index].value, &value){
                                            mutex_lock[index].value = value;
                                            true
                                        }
                                        else {false}
                                    ))
                                }
                                else{
                                    match *mutex_lock[index].child_keys[Self::get_child_index(key, current_key)].get_or_insert((key, EntryLock::Read(
                                    ))){
                                        (next_key, ) => current_key = next_key,
                                        (_, true) => ()
                                    }
                                    None
                                }
                            }
                            None => {
                                if current_key == key{
                                    //insert
                                    mutex_lock.push(ConcurrentBSTMapEntry::new(key, value));
                                    Some(InsertResult::Inserted(
                                      if read_lock.no_elements.fetch_add(1, Ordering::Relaxed) >= list_length {true}
                                      else {false}
                                    ))
                                }
                                else {None}
                            }
                        }
                    }).unwrap(){
                        Some(result) => return Some(result),
                        None => ()
                    }
                }
            }).unwrap(){
                Some(result) => {
                    return match result{
                        InsertResult::Updated(was_updated) => was_updated,
                        InsertResult::Inserted(needs_resizing) => {
                            if needs_resizing{
                                self.inner.write().map(|mut write_lock| {
                                    let old_list_length = write_lock.list.len();
                                    let no_elements = write_lock.no_elements.load(Ordering::Relaxed);
                                    if no_elements >= old_list_length {
                                        let mut new_list_length = old_list_length;
                                        while no_elements >= new_list_length {new_list_length *= 2}
                                        for _ in old_list_length..new_list_length {write_lock.list.push(Mutex::new(Vec::new()))}
                                        for i in 0..old_list_length{
                                            for entry in write_lock.list[i].lock().map(|mut inner_lock| {
                                                let old_entries = inner_lock.clone();
                                                *inner_lock = Vec::new();
                                                old_entries
                                            }).unwrap(){
                                                write_lock.list[get_index(entry.key, new_list_length)].lock().unwrap().push(entry)
                                            }
                                        }
                                    }
                                }).unwrap();
                            }
                            true
                        }
                    }
                }
                None => _ = self.inner.write().unwrap().root_node_key.get_or_insert(key)
            }
        }
    }
    
    pub fn is_empty(&self) -> bool{
        self.len() == 0
    }
    
    pub fn len(&self) -> usize{
        self.inner.read().unwrap().no_elements.load(Ordering::Relaxed)
    }

    pub const fn new() -> Self{
        Self {
            inner: LazyLock::new(|| RwLock::new(ConcurrentBSTMapInternal::new()))
        }
    }
    
    pub fn remove(&self, key: K){
        self.remove_if(key, |_,_| true)
    }
    
    pub fn remove_if(&self, key: K, should_remove: impl Fn(&K, &V) -> bool){

        //find parent node and node, check should be removed
        //if node is root node then change to write lock and remove aswell as changing the root node key
        //else lock the parent node relevant child node and stay in read lock
        //go onto node to remove
        //if has no child nodes then just delete it
        //go to right child node and then left nodes and find next largest key, delete it, remove it from its parent nodes child nodes and set the node to be this node
        //if no right child node then go to left child node and then right nodes, same as above but previous smallest
        
        let inner_function = |root_node_key, lock_guard: LockGuard<K, V>| {
            
            let mut current_key = root_node_key;
            let list_length = lock_guard.list.len();
            
            let parent_node_key = 
            
            loop{
                match lock_guard.list[get_index(current_key, list_length)].lock().map(|mutex_lock| {
                    match mutex_lock.iter().position(|x| x.key == current_key) {
                        Some(index) => {
                            if current_key == key {
                                //one to remove
                                Some(
                                    if should_remove(&mutex_lock[index].key, &mutex_lock[index].value) {
                                        
                                        true
                                    }
                                    else {false}
                                )
                            }
                            else{
                                match mutex_lock[index].child_keys[Self::get_child_index(key, current_key)]{
                                    Some((next_key, locked)) => {
                                        if !locked {current_key = next_key}
                                        None
                                    }
                                    None => Some(false)
                                }
                            }
                        }
                        None => Some(false)
                    }
                }).unwrap() {
                    Some(result) => {
                        if result{

                        }
                    }
                    None => ()
                }
            }
        };
        
        loop{
            if self.inner.read().map(|read_lock| {
                match read_lock.root_node_key{
                    Some(x) => {
                        if x == key {false}
                        else{
                            inner_function(x, LockGuard::Read(read_lock));
                            true
                        }
                    },
                    None => true
                }
            }).unwrap() {return}
            if self.inner.write().map(|mut write_lock| {
                match write_lock.root_node_key{
                    Some(x) => {
                        if x == key {
                            write_lock.root_node_key = None;
                            inner_function(x, LockGuard::Write(write_lock));
                            true
                        }
                        else {false}
                    }
                    None => true
                }
            }).unwrap() {return}
        }
        
    }
    
}