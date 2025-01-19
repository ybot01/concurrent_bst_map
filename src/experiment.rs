use std::sync::RwLock;

pub struct ConcurrentMap<const N: usize, V>(RwLock<ConcurrentMapInternal<N, V>>);

enum ConcurrentMapInternal<const N: usize, V>{
    Item([u8; N], V),
    List(Vec<(u8, Box<ConcurrentMap<N, V>>)>)
}

impl<const N: usize, V: Copy> ConcurrentMapInternal<N, V>{
    
    const fn new_empty_list() -> Self{
        Self::List(Vec::new())
    }
}

impl<const N: usize, V: Copy> ConcurrentMap<N, V>{

    const fn get_index(key: [u8; N], depth: usize) -> u8{
        match depth % 4{
            0 => (key[depth/4] & 0b11000000) >> 6,
            1 => (key[depth/4] & 0b00110000) >> 4,
            2 => (key[depth/4] & 0b00001100) >> 2,
            _ => key[depth/4] & 0b00000011
        }
    }
    
    pub const fn new() -> Self{
        Self(RwLock::new(ConcurrentMapInternal::new_empty_list()))
    }
    
    const fn new_item(key: [u8;N], value: V) -> Self{
        Self(RwLock::new(ConcurrentMapInternal::Item(key, value)))
    }

    pub fn clear(&self){
        *self.0.write().unwrap() = ConcurrentMapInternal::new_empty_list();
    }

    pub fn get(&self, key: [u8; N]) -> Option<V>{
        self.get_internal(key, 0)
    }
    
    fn get_internal(&self, key: [u8; N], depth: usize) -> Option<V>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                ConcurrentMapInternal::Item(item_key, item_value) => if *item_key == key {Some(*item_value)} else {None}
                ConcurrentMapInternal::List(list) => {
                    let index = Self::get_index(key, depth);
                    list.iter().find(|x| x.0 == index)
                        .and_then(|x| x.1.get_internal(key, depth + 1))
                }
            }
        }).unwrap()
    }

    pub fn insert_or_update(&self, key: [u8; N], value: V) -> bool{
        self.insert_or_update_if(key, value, &|_,_| true)
    }
    
    pub fn insert_or_update_if(&self, key: [u8; N], value: V, should_update: &impl Fn(&V, &V) -> bool) -> bool{
        self.insert_or_update_if_internal(key, value, should_update, 0)
    }

    fn insert_or_update_if_internal(&self, key: [u8; N], value: V, should_update: &impl Fn(&V, &V) -> bool, depth: usize) -> bool{
        let index = Self::get_index(key, depth);
        loop{
            match self.0.read().map(|read_lock| {
                match &*read_lock{
                    ConcurrentMapInternal::Item(_,_) => None, //change to write lock
                    ConcurrentMapInternal::List(list) => {
                        list.iter().find(|x| x.0 == index)
                            .map(|x| x.1.insert_or_update_if_internal(key, value, should_update, depth + 1))
                    }
                }
            }).unwrap(){
                None => (),
                Some(x) => return x
            }
            match self.0.write().map(|mut write_lock| {
                match &mut *write_lock {
                    ConcurrentMapInternal::Item(existing_key, existing_value) => {
                        if *existing_key == key{
                            //update
                            Some(
                                if should_update(existing_value, &value){
                                    *existing_value = value;
                                    true
                                }
                                else {false}
                            )
                        }
                        else{
                            //insert and restructure
                            *write_lock = Self::deepen_tree((*existing_key, *existing_value), (key, value), depth);
                            Some(true)
                        }
                    }
                    ConcurrentMapInternal::List(list) => {
                        if list.iter().any(|x| x.0 == index) {None}
                        else {
                            list.push((index, Box::new(Self::new_item(key, value))));
                            Some(true)
                        }
                    }
                }
            }).unwrap(){
                None => (),
                Some(x) => return x
            }
        }
    }

    fn deepen_tree(item_1: ([u8; N], V), item_2: ([u8; N], V), depth: usize) -> ConcurrentMapInternal<N, V> {
        let item_1_index = Self::get_index(item_1.0, depth);
        let item_2_index = Self::get_index(item_2.0, depth);
        let mut new_vec = Vec::new();
        if item_1_index == item_2_index {
            new_vec.push((item_1_index, Box::new(ConcurrentMap(RwLock::new(Self::deepen_tree(item_1, item_2, depth + 1))))));
        }
        else{
            new_vec.push((item_1_index, Box::new(Self::new_item(item_1.0, item_1.1))));
            new_vec.push((item_2_index, Box::new(Self::new_item(item_2.0, item_2.1))));
        }
        ConcurrentMapInternal::List(new_vec)
    }

    pub fn remove(&self, key: [u8; N], value: V){
        self.remove_if(key, value, &|_,_| true)
    }

    pub fn remove_if(&self, key: [u8; N], value: V, should_remove: &impl Fn(&[u8; N], &V) -> bool){
        self.remove_if_internal(key, value, should_remove, 0);
    }

    fn remove_if_internal(&self, key: [u8; N], value: V, should_remove: &impl Fn(&[u8; N], &V) -> bool, depth: usize) -> bool{
        let index = Self::get_index(key, depth);
        loop {
            if self.0.read().map(|read_lock| {
                match &*read_lock {
                    ConcurrentMapInternal::Item(existing_key, existing_value) => {
                        Some((*existing_key == key) && should_remove(existing_key, existing_value))
                    }
                    ConcurrentMapInternal::List(list) => {
                        list.iter().find(|x| x.0 == index)
                            .map(|x| x.1.remove_if_internal(key, value, should_remove, depth + 1))
                    }
                }
            }).unwrap().is_some_and(|x| x){
                self.0.write().map(|mut write_lock| {
                    match &mut *write_lock {
                        ConcurrentMapInternal::Item(_,_) => true,
                        ConcurrentMapInternal::List(list) => {
                            match list.iter().position(|x| x.0 == index){
                                Some(x) => {
                                    if list[x].1.0.read().unwrap().
                                }
                                None => 
                            }
                            
                        }
                    }
                }).unwrap();
            }
        }
    }
}