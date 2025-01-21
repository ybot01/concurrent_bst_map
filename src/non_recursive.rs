use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, RwLock};

fn get_index<const N: usize>(key: [u8; N], max_index: usize) -> usize{
    ((((max_index as f64) / (u32::MAX as f64)) * (u32::from_be_bytes(<[u8;4]>::try_from(&key[0..4]).unwrap()) as f64)) as usize).min(max_index - 1)
}

fn leading_zeroes<const N: usize>(array_1: [u8; N], array_2: [u8; N]) -> u32{
    let mut leading_zeroes = 0;
    let mut temp;
    for i in 0..N{
        temp = (array_1[i]^array_2[i]).leading_zeros();
        leading_zeroes += temp;
        if temp < 8 {break}
    }
    leading_zeroes
}

const MIN_LIST_LENGTH: usize = 1024;

#[derive(Debug)]
pub struct ConcurrentMap<const N: usize, V>{
    inner: LazyLock<RwLock<ConcurrentMapInternal<N, V>>>
}

#[derive(Debug)]
struct ConcurrentMapInternal<const N: usize, V>{
    no_elements: AtomicUsize,
    list: Vec<RwLock<Vec<([u8; N],V)>>>
}

impl<const N: usize, V: Copy> ConcurrentMapInternal<N, V>{
    fn new() -> Self{
        Self{
            no_elements: AtomicUsize::new(0),
            list: {
                let mut new_vec = Vec::new();
                for _ in 0..MIN_LIST_LENGTH {new_vec.push(RwLock::new(Vec::new()))}
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
            read_lock.list[get_index(key, read_lock.list.len())].read().unwrap().iter()
                .position(|x| x.0 == key).is_some()
        }).unwrap()
    }
    
    pub fn get(&self, key: [u8; N]) -> Option<V>{
        self.inner.read().map(|read_lock| {
            read_lock.list[get_index(key, read_lock.list.len())].read().unwrap().iter()
                .find(|x| x.0 == key).map(|x| x.1)
        }).unwrap()
    }

    pub fn get_min(&self) -> Option<([u8; N],V)>{
        self.inner.read().unwrap().list[0].read().unwrap().iter().min_by_key(|x| x.0).map(|x| *x)
    }

    pub fn get_max(&self) -> Option<([u8; N],V)>{
        self.inner.read().map(|read_lock| {
            read_lock.list[read_lock.list.len() - 1].read().unwrap().iter()
                .max_by_key(|x| x.0).map(|x| *x)
        }).unwrap()
    }
    
    pub fn get_or_closest_by_key(&self, key: [u8; N], include_key: bool) -> Option<([u8; N],V)>{
        self.inner.read().map(|read_lock| {
            let index = get_index(key, read_lock.list.len());
            let mut min = true;
            let mut max = true;
            match read_lock.list[index].read().unwrap().iter()
                .filter(|x| include_key || (x.0 != key))
                .max_by_key(|x| {
                    if x.0 < key {min = false}
                    else if x.0 > key {max = false}
                    leading_zeroes(x.0, key)
                }).map(|x| *x)
            {
                None => {
                    //get closest left and right
                    let mut left_closest = None;
                    for i in (0..index).rev(){
                        match read_lock.list[i].read().unwrap().iter().max_by_key(|x| leading_zeroes(x.0, key)).map(|x| *x) {
                            None => (),
                            Some(x) => { 
                                left_closest = Some(x);
                                break
                            }
                        }
                    }
                    let mut right_closest = None;
                    for i in (index+1)..read_lock.list.len(){
                        match read_lock.list[i].read().unwrap().iter().max_by_key(|x| leading_zeroes(x.0, key)).map(|x| *x) {
                            None => (),
                            Some(x) => {
                                right_closest = Some(x);
                                break
                            }
                        }
                    }
                    [left_closest, right_closest].iter().filter_map(|x| *x).max_by_key(|x| leading_zeroes(x.0, key))
                }
                Some(closest) => {
                    if min == max {return Some(closest)}
                    else if min {
                        //get closest left
                        match read_lock.list[if index == 0 {read_lock.list.len() - 1} else {index - 1}].read().unwrap().iter()
                            .max_by_key(|x| leading_zeroes(x.0, key)).map(|x| *x){
                            None => Some(closest),
                            Some(left_closest) => [left_closest, closest].iter().max_by_key(|x| leading_zeroes(x.0, key)).map(|x| *x)
                        }
                    }
                    else{
                        //get closest right
                        match read_lock.list[if index == (read_lock.list.len() - 1) {0} else {index + 1}].read().unwrap().iter()
                            .max_by_key(|x| leading_zeroes(x.0, key)).map(|x| *x){
                            None => Some(closest),
                            Some(right_closest) => [right_closest, closest].iter().max_by_key(|x| leading_zeroes(x.0, key)).map(|x| *x)
                        }
                    }
                }
            }
        }).unwrap()
    }

    pub fn insert_or_update(&self, key: [u8; N], value: V) -> bool{
        self.insert_or_update_if(key, value, |_,_| true)
    }

    pub fn insert_or_update_if(&self, key: [u8; N], value: V, should_update: impl Fn(&V, &V) -> bool) -> bool{
        match self.inner.read().map(|read_lock| {
            read_lock.list[get_index(key, read_lock.list.len())].write().map(|mut write_lock| {
                match write_lock.iter().position(|x| x.0 == key){
                    Some(index) => {
                        //update
                        if should_update(&write_lock[index].1, &value){
                            write_lock[index].1 = value;
                            (false, true)
                        }
                        else {(false, false)}
                    }
                    None => {
                        //insert
                        write_lock.push((key, value));
                        (true, read_lock.no_elements.fetch_add(1, Ordering::Relaxed) >= (read_lock.list.len() * 2))
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
                        let mut new_list_length = old_list_length;
                        while no_elements >= (new_list_length * 2) {new_list_length *= 2}
                        if new_list_length > old_list_length{
                            for _ in old_list_length..new_list_length {write_lock.list.push(RwLock::new(Vec::new()))}
                            for i in 0..old_list_length{
                                for entry in write_lock.list[i].write().map(|mut inner_lock| {
                                    let old_entries = inner_lock.clone();
                                    *inner_lock = Vec::new();
                                    old_entries
                                }).unwrap(){
                                    write_lock.list[get_index(entry.0, new_list_length)].write().unwrap().push(entry)
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
        if self.inner.read().map(|read_lock| {
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
        }
    }
}