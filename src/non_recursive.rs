use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex, RwLock};

fn get_index<const N: usize>(key: [u8; N], max_index: usize) -> usize{
    ((((u64::from_be_bytes(<[u8;8]>::try_from(&key[0..8]).unwrap()) as f64) / (u64::MAX as f64)) * (max_index as f64)) as usize).min(max_index - 1)
}

#[derive(Debug)]
pub struct ConcurrentMap<const N: usize, V>{
    inner: LazyLock<RwLock<ConcurrentMapInternal<N, V>>>
}

#[derive(Debug)]
struct ConcurrentMapInternal<const N: usize, V>{
    no_elements: AtomicUsize,
    list: Vec<Mutex<Vec<([u8; N],V)>>>
}

impl<const N: usize, V: Copy> ConcurrentMapInternal<N, V>{
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

impl<const N: usize, V: Copy> ConcurrentMap<N, V>{

    pub fn clear(&self){
        *self.inner.write().unwrap() = ConcurrentMapInternal::new();
    }
    
    pub fn contains_key(&self, key: [u8; N]) -> bool{
        self.inner.read().map(|read_lock| {
            read_lock.list[get_index(key, read_lock.list.len())].lock().unwrap().iter()
                .position(|x| x.0 == key).is_some()
        }).unwrap()
    }
    
    pub fn get(&self, key: [u8; N]) -> Option<V>{
        self.inner.read().map(|read_lock| {
            read_lock.list[get_index(key, read_lock.list.len())].lock().unwrap().iter()
                .find(|x| x.0 == key).map(|x| x.1)
        }).unwrap()
    }

    pub fn get_min(&self) -> Option<([u8; N],V)>{
        self.inner.read().unwrap().list[0].lock().unwrap().iter().min_by_key(|x| x.0).map(|x| *x)
    }

    pub fn get_max(&self) -> Option<([u8; N],V)>{
        self.inner.read().map(|read_lock| {
            read_lock.list[read_lock.list.len() - 1].lock().unwrap().iter()
                .max_by_key(|x| x.0).map(|x| *x)
        }).unwrap()
    }

    pub fn insert_or_update(&self, key: [u8; N], value: V) -> bool{
        self.insert_or_update_if(key, value, |_,_| true)
    }

    pub fn insert_or_update_if(&self, key: [u8; N], value: V, should_update: impl Fn(&V, &V) -> bool) -> bool{
        match self.inner.read().map(|read_lock| {
            read_lock.list[get_index(key, read_lock.list.len())].lock().map(|mut mutex_lock| {
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
                                    write_lock.list[get_index(entry.0, new_list_length)].lock().unwrap().push(entry)
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
    
    pub fn remove(&self, key: [u8; N]){
        self.remove_if(key, |_| true)
    }
    
    pub fn remove_if(&self, key: [u8; N], should_remove: impl Fn(&V) -> bool){
        self.inner.read().map(|read_lock| {
            read_lock.list[get_index(key, read_lock.list.len())].lock().map(|mut mutex_lock| {
                match mutex_lock.iter().position(|x| x.0 == key){
                    Some(index) => if should_remove(&mutex_lock[index].1) {
                        mutex_lock.swap_remove(index);
                        read_lock.no_elements.fetch_sub(1, Ordering::Relaxed);
                    }
                    None => ()
                }
            }).unwrap();
        }).unwrap();
    }
}