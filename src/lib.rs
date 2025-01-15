use std::ops::Sub;
use std::sync::RwLock;

#[derive(Debug)]
struct ConcurrentBSTInternal<K,V>{
    key: K,
    value: V,
    child_nodes: [ConcurrentBSTMap<K,V>; 2]
}

impl<K: Copy + Ord + Sub<Output = K>, V: Copy> ConcurrentBSTInternal<K,V>{
    
    const fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            child_nodes: [const { ConcurrentBSTMap::new() }; 2]
        }
    }
}

#[derive(Debug)]
pub struct ConcurrentBSTMap<K,V>(RwLock<Option<Box<ConcurrentBSTInternal<K,V>>>>);

impl<K: Copy + Ord + Sub<Output = K>, V: Copy> ConcurrentBSTMap<K,V>{

    pub const fn new() -> Self{
        Self(RwLock::new(None))
    }

    pub fn clear(&self){
        *self.0.write().unwrap() = None
    }

    pub fn is_empty(&self) -> bool{
        self.len() == 0
    }

    pub fn len(&self) -> usize{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => 0,
                Some(node) => {
                    1 + node.child_nodes[0].len() + node.child_nodes[1].len()
                }
            }
        }).unwrap()
    }
    
    pub fn depth(&self) -> usize{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => 0,
                Some(node) => {
                    1 + node.child_nodes[0].depth().max(node.child_nodes[1].depth())
                }
            }
        }).unwrap()
    }


    fn get_index(target: K, current: K) -> usize{
        if target < current {0} else {1}
    }

    pub fn contains_key(&self, key: K) -> bool{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => false,
                Some(node) => {
                    if node.key == key {true}
                    else {node.child_nodes[Self::get_index(key, node.key)].contains_key(key)}
                }
            }
        }).unwrap()
    }
    
    pub fn get(&self, key: K) -> Option<V>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => None,
                Some(node) => {
                    if node.key == key {Some(node.value)}
                    else {node.child_nodes[Self::get_index(key, node.key)].get(key)}
                }
            }
        }).unwrap()
    }
    
    fn abs_diff<T: Ord + Sub<Output = T>>(item_1: T, item_2: T) -> T{
        if item_2 > item_1 {item_2 - item_1} 
        else {item_1 - item_2}
    }

    fn internal_get_or_next_by_key(&self, key: K, include_key: bool, is_final_node: bool) -> Option<(K, V, bool)>{
        self.0.read().map(|read_lock| {
            match &*read_lock {
                None => None,
                Some(node) => {
                    let index = Self::get_index(key, node.key);
                    [
                        Some((node.key, node.value, is_final_node)),
                        node.child_nodes[index].internal_get_or_next_by_key(key, include_key, (index == 1) && is_final_node)
                    ].iter().filter_map(|x| *x)
                    .filter(|x| if include_key {x.0 >= key} else {x.0 > key})
                    .min_by_key(|x| x.0 - key)
                }
            }
        }).unwrap()
    }

    pub fn get_or_next_by_key(&self, key: K, include_key: bool, min_if_end: bool) -> Option<(K, V)>{
        match self.internal_get_or_next_by_key(key, include_key, min_if_end){
            None => None,
            Some(result) => {
                match result{
                    (r_key, r_value, false) => Some((r_key, r_value)),
                    (_, _, true) => self.get_min_by_key()
                }
            }
            
        }
    }
    
    pub fn insert_or_update(&self, key: K, value: V)  -> bool{
        self.insert_or_update_if(key, value, &|_,_| true)
    }

    pub fn insert_or_update_if(&self, key: K, value: V, should_update: &impl Fn(&V, &V) -> bool) -> bool{
        loop{
            match self.0.read().map(|read_lock| {
                match &*read_lock{
                    None => None,
                    Some(node) => {
                        if node.key != key {Some(node.child_nodes[Self::get_index(key, node.key)].insert_or_update_if(key, value, should_update))}
                        else {None}
                    }
                }
            }).unwrap(){
                None => (),
                Some(result) => return result
            }
            match self.0.write().map(|mut write_lock| {
                match &mut *write_lock{
                    None => {
                        //insert
                        *write_lock = Some(Box::new(ConcurrentBSTInternal::new(key, value)));
                        Some(true)
                    }
                    Some(node) => {
                        //if a different key than before then retry the read lock
                        if node.key != key {None}
                        else{
                            //update
                            Some(
                                if should_update(&node.value, &value){
                                    node.value = value;
                                    true
                                }
                                else {false}
                            )
                        }
                    }
                }
            }).unwrap(){
                None => (),
                Some(result) => return result
            }
        }
    }
    
    fn internal_get_replacement_key_value(&self, go_left: bool) -> Option<(K,V)>{
        self.0.write().map(|mut write_lock| {
            match &mut *write_lock {
                None => None,
                Some(node) => {
                    match node.child_nodes[if go_left {0} else {1}].internal_get_replacement_key_value(go_left){
                        None => {
                            //found replacement node with no node in chosen direction
                            let key_value = (node.key, node.value);
                            //if got opposite direction node, recursively run on that
                            match node.child_nodes[if go_left {1} else {0}].internal_get_replacement_key_value(go_left){
                                None => *write_lock = None,
                                Some(result) => (node.key, node.value) = result
                            }
                            Some(key_value)
                        }
                        Some(result) => Some(result)
                    }
                }
            }
        }).unwrap()
    }
    
    pub fn remove_if(&self, key: K, should_remove: &impl Fn(&V) -> bool){
        loop{
            if self.0.read().map(|read_lock| {
                match &*read_lock{
                    None => true,
                    Some(node) => {
                        if node.key != key {
                            node.child_nodes[Self::get_index(key, node.key)].remove_if(key, should_remove);
                            true
                        }
                        else {false}
                    }
                }
            }).unwrap() {return}
            if self.0.write().map(|mut write_lock| {
                match &mut *write_lock{
                    None => true,
                    Some(node) => {
                        //if a different key than before then retry the read lock
                        if node.key != key {false}
                        else if should_remove(&node.value){
                            match node.child_nodes[1].internal_get_replacement_key_value(true)
                                .or(node.child_nodes[0].internal_get_replacement_key_value(false)) {
                                None => *write_lock = None,
                                Some(result) => (node.key, node.value) = result
                            }
                            true
                        }
                        else {true}
                    }
                }
            }).unwrap() {return}
        }
    }

    pub fn remove(&self, key: K){
        self.remove_if(key, &|_| true)
    }

    pub fn retain(&self, criteria: &impl Fn(&V) -> bool){
        
    }
}

struct ConcurrentBSTMapIterator<'a, K, V> {
    map: &'a ConcurrentBSTMap<K,V>,
    current_key: Option<K>
}

impl<'a, K: Copy + Ord + Sub<Output = K>, V: Copy> Iterator for ConcurrentBSTMapIterator<'a, K, V>{
    type Item = (K,V);

    fn next(&mut self) -> Option<Self::Item> {
        match self.current_key{
            None => None,
            Some(current_key) => {
                let next_key_value = self.map.get_or_next_by_key(current_key, false, false);
                self.current_key = next_key_value.map(|x| x.0);
                next_key_value
            }
        }
    }
}

#[derive(Debug)]
pub struct ConcurrentBSTSet<K>(ConcurrentBSTMap<K, ()>);

impl<K: Copy + Ord + Sub<Output = K>> ConcurrentBSTSet<K>{
    
    pub const fn new() -> Self{
        Self(ConcurrentBSTMap::new())
    }

    pub fn clear(&self){
        self.0.clear();
    }

    pub fn is_empty(&self) -> bool{
        self.0.is_empty()
    }

    pub fn len(&self) -> usize{
        self.0.len()
    }

    pub fn depth(&self) -> usize{
        self.0.depth()
    }

    pub fn contains_key(&self, key: K) -> bool{
        self.0.contains_key(key)
    }

    pub fn get_or_next(&self, key: K, min_if_end: bool) -> Option<K>{
        self.0.get_or_next_by_key(key, true, min_if_end).map(|x| x.0)
    }

    pub fn insert(&self, key: K){
        self.0.insert_or_update_if(key, (), &|_,_| false);
    }
    
    pub fn remove(&self, key: K){
        self.0.remove(key)
    }
}