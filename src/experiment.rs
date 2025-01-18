use std::sync::{atomic::{AtomicUsize, Ordering}, LazyLock, Mutex, RwLock};

pub struct ConcurrentMap<K, V>(LazyLock<RwLock<ConcurrentMapInner<K, V>>>);

struct ConcurrentMapInner<K, V>{
    no_entries: AtomicUsize,
    list: Vec<Mutex<Vec<Entry<K, V>>>>
}

impl<K, V> ConcurrentMapInner<K, V>{

    fn new() -> Self{
        Self { 
            no_entries: AtomicUsize::new(0), 
            list: {
                let mut new_vec = Vec::new();
                for _ in 0..1024 {new_vec.push(Mutex::new(Vec::new()))}
                new_vec
            }
        }
    }
}

struct Entry<K, V>{
    key: K,
    value: V
}

impl<K, V> Entry<K, V>{
    fn new(key: K, value: V) -> Self{
        Self{
            key,
            value
        }
    }
}

pub trait Resize{
    fn resize(&self, length: usize) -> usize;
}

impl<K: Copy + Resize + Ord, V: Copy> ConcurrentMap<K, V>{

    pub const fn new() -> Self{
        Self(LazyLock::new(|| RwLock::new(ConcurrentMapInner::new())))
    }

    pub fn get(&self, key: K) -> Option<V>{
        self.0.read().map(|read_lock| {
            read_lock.list[key.resize(read_lock.list.len())].lock().unwrap()
            .iter().find(|x| x.key == key).map(|x| x.value)
        }).unwrap()
    }

    pub fn insert_or_update(&self, key: K, value: V) -> bool{
        self.insert_or_update_if(key, value, |_,_| true)
    }

    //make so if lots of keys clusteted together then subdivide further
    //subdivisions adapt to clusters and empty parts alike

    pub fn insert_or_update_if(&self, key: K, value: V, should_update: impl Fn(&V, &V) -> bool) -> bool{
        self.0.read().map(|read_lock| {
            read_lock.list[key.resize(read_lock.list.len())].lock().map(|mut mutex_lock| {
                match mutex_lock.iter().position(|x| x.key == key){
                    Some(index) => {
                        //update
                        if should_update(&mutex_lock[index].value, &value){
                            mutex_lock[index].value = value;
                            true
                        }
                        else {false}
                    }
                    None => {
                        //insert
                        mutex_lock.push(Entry::new(key, value));
                        read_lock.no_entries.fetch_add(1, Ordering::Relaxed);
                        true
                    }
                }
            }).unwrap()
        }).unwrap()
    }

}