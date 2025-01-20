use std::sync::RwLock;

pub struct ConcurrentMap<const N: usize, V>(RwLock<ConcurrentMapInternal<N, V>>);

enum ConcurrentMapInternal<const N: usize, V>{
    Item([u8; N], V),
    List([Option<Box<ConcurrentMap<N, V>>>; 4])
}

impl<const N: usize, V: Copy> ConcurrentMapInternal<N, V>{
    
    const fn new_empty_list() -> Self{
        Self::List([const {None}; 4])
    }
}

impl<const N: usize, V: Copy> ConcurrentMap<N, V>{
    
    const fn get_index(key: [u8; N], depth: usize) -> usize{
        (match depth % 4{
            0 => (key[depth/4] & 0b11000000) >> 6,
            1 => (key[depth/4] & 0b00110000) >> 4,
            2 => (key[depth/4] & 0b00001100) >> 2,
            _ => key[depth/4] & 0b00000011
        }) as usize
    }
    
    pub fn len(&self) -> usize{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                ConcurrentMapInternal::Item(_,_) => 1,
                ConcurrentMapInternal::List(list) => {
                    let mut length = 0;
                    for i in list{
                        match i{
                            None => (),
                            Some(x) => length += x.len()
                        }
                    }
                    length
                }
            }
        }).unwrap()
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
                    match &list[Self::get_index(key, depth)]{
                        None => None,
                        Some(x) => x.get_internal(key, depth + 1)
                    }
                }
            }
        }).unwrap()
    }
    
    fn get_next(&self, key: [u8; N], depth: usize) -> Option<([u8; N], V)>{
        //go down to where key is or would be at item end node
        //then continuously go up and look for next index until find one 
        //then go down and find minimum key in that
        self.0.read().map(|read_lock| {
            match &*read_lock{
                ConcurrentMapInternal::Item(item_key, item_value) => ,
                ConcurrentMapInternal::List(list) => {
                    
                }
            }
        }).unwrap()
    }
    
    pub fn get_min(&self) -> Option<([u8; N], V)>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                ConcurrentMapInternal::Item(item_key, item_value) => Some((*item_key, *item_value)),
                ConcurrentMapInternal::List(list) => {
                    for i in 0..4{
                        match &list[i]{
                            None => (),
                            Some(x) => return x.get_min()
                        }
                    }
                    None
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
                        match &list[index]{
                            None => None, //change to write lock
                            Some(x) => Some(x.insert_or_update_if_internal(key, value, should_update, depth + 1))
                        }
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
                        match &list[index]{
                            None => {
                                list[index] = Some(Box::new(Self::new_item(key, value)));
                                Some(true)
                            }
                            Some(_) => None //change back to read lock
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
        let mut new_list = [const {None}; 4];
        if item_1_index == item_2_index {
            new_list[item_1_index] = Some(Box::new(ConcurrentMap(RwLock::new(Self::deepen_tree(item_1, item_2, depth + 1)))));
        }
        else{
            new_list[item_1_index] = Some(Box::new(Self::new_item(item_1.0, item_1.1)));
            new_list[item_2_index] = Some(Box::new(Self::new_item(item_2.0, item_2.1)));
        }
        ConcurrentMapInternal::List(new_list)
    }

    /*pub fn remove(&self, key: [u8; N], value: V){
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
    }*/
}