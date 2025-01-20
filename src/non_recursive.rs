use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex, RwLock};

pub trait GetIndex{
    fn get_index(&self, max_index: usize) -> usize;
}

#[derive(Debug)]
pub struct ConcurrentMap<K, V>{
    inner: LazyLock<RwLock<ConcurrentMapInternal<K, V>>>
}

#[derive(Debug)]
struct ConcurrentMapInternal<K, V>{
    no_elements: AtomicUsize,
    list: Vec<Mutex<Vec<(K,V)>>>
}

impl<K: Copy + Ord, V: Copy> ConcurrentMapInternal<K, V>{
    fn new() -> Self{
        Self{
            no_elements: AtomicUsize::new(0),
            list: {
                let mut new_vec = Vec::new();
                for _ in 0..1024 {new_vec.push(Mutex::new(Vec::new()))}
                new_vec
            }
        }
    }
}

impl<K: Copy + Ord + GetIndex, V: Copy> ConcurrentMap<K, V>{

    pub fn clear(&self){
        *self.inner.write().unwrap() = ConcurrentMapInternal::new();
    }
    
    pub fn contains_key(&self, key: K) -> bool{
        self.inner.read().map(|read_lock| {
            read_lock.list[key.get_index(read_lock.list.len())].lock().unwrap().iter()
                .position(|x| x.0 == key).is_some()
        }).unwrap()
    }
    
    pub fn get(&self, key: K) -> Option<V>{
        self.inner.read().map(|read_lock| {
            read_lock.list[key.get_index(read_lock.list.len())].lock().unwrap().iter()
                .find(|x| x.0 == key).map(|x| x.1)
        }).unwrap()
    }
    
    pub fn get_min(&self) -> Option<(K,V)>{
        self.inner.read().unwrap().list[0].lock().unwrap().iter().min_by_key(|x| x.0).map(|x| *x)
    }

    pub fn get_max(&self) -> Option<(K,V)>{
        self.inner.read().map(|read_lock| {
            read_lock.list[read_lock.list.len() - 1].lock().unwrap().iter()
                .max_by_key(|x| x.0).map(|x| *x)
        }).unwrap()
    }

    pub fn insert_or_update(&self, key: K, value: V) -> bool{
        self.insert_or_update_if(key, value, |_,_| true)
    }

    pub fn insert_or_update_if(&self, key: K, value: V, should_update: impl Fn(&V, &V) -> bool) -> bool{
        match self.inner.read().map(|read_lock| {
            read_lock.list[key.get_index(read_lock.list.len())].lock().map(|mut mutex_lock| {
                match mutex_lock.iter().position(|x| x.0 == key){
                    Some(index) => {
                        //update
                        if should_update(&mutex_lock[index].1, &value){
                            mutex_lock[index].1 = value;
                            (false, true)
                        }
                        else {(false, false)}
                    }
                    None => {
                        //insert
                        mutex_lock.push((key, value));
                        (true, read_lock.no_elements.fetch_add(1, Ordering::Relaxed) >= read_lock.list.len())
                    }
                }
            }).unwrap()
        }).unwrap(){
            (false, updated) => updated,
            (true, needs_resizing) => {
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
                                    write_lock.list[entry.0.get_index(new_list_length)].lock().unwrap().push(entry)
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
        self.inner.read().unwrap().no_elements.load(Ordering::Relaxed)
    }

    pub const fn new() -> Self{
        Self {
            inner: LazyLock::new(|| RwLock::new(ConcurrentMapInternal::new()))
        }
    }
    
    pub fn remove(&self, key: K){
        self.remove_if(key, |_| true)
    }
    
    pub fn remove_if(&self, key: K, should_remove: impl Fn(&V) -> bool){
        self.inner.read().map(|read_lock| {
            read_lock.list[key.get_index(read_lock.list.len())].lock().map(|mut mutex_lock| {
                match mutex_lock.iter().position(|x| x.0 == key){
                    Some(index) => if should_remove(&mutex_lock[index].1) {_ = mutex_lock.swap_remove(index)},
                    None => ()
                }
            }).unwrap();
        }).unwrap();
    }
}