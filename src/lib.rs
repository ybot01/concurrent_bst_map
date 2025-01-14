//#![recursion_limit = "128"] //default is 128, can increase if desired

use std::sync::RwLock;

#[derive(Debug)]
struct ConcurrentBSTInternal<K,V>{
    key: K,
    value: V,
    child_nodes: [ConcurrentBSTMap<K,V>; 2]
}

impl<K: Copy + Ord, V: Copy> ConcurrentBSTInternal<K,V>{
    
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

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
enum Direction{
    Left,
    Right
}

impl Direction{
    
    fn to_usize(&self) -> usize{
        match self{
            Direction::Left => 0,
            Direction::Right => 1
        }
    }
    
    fn get_opposite(&self) -> Self{
        match self{
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left
        }
    }
    
    fn get_direction<K>(target_key: K, current_key: K) -> Self where K: Ord{
        if target_key < current_key {Self::Left} else {Self::Right}
    }
}


impl<K: Copy + Ord, V: Copy> ConcurrentBSTMap<K,V>{

    pub const fn new() -> Self{
        Self(RwLock::new(None))
    }

    pub fn clear(&self){
        *self.0.write().unwrap() = None
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
    
    pub fn get(&self, key: K) -> Option<V>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => None,
                Some(node) => {
                    if node.key == key {Some(node.value)}
                    else {node.child_nodes[Direction::get_direction(key, node.key).to_usize()].get(key)}
                }
            }
        }).unwrap()
    }

    pub fn get_or_closest(&self, key: K) -> Option<V>{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => None,
                Some(node) => {
                    if node.key == key {Some(node.value)}
                    else{
                        match node.child_nodes[Direction::get_direction(key, node.key).to_usize()].get_or_closest(key){
                            None => {
                                
                            }
                            Some(result) => Some(result)
                        }
                    }
                    
                }
            }
        }).unwrap()
    }

    pub fn contains_key(&self, key: K) -> bool{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => false,
                Some(node) => {
                    if node.key == key {true}
                    else {node.child_nodes[Direction::get_direction(key, node.key).to_usize()].contains_key(key)}
                }
            }
        }).unwrap()
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
                        if node.key != key {Some(node.child_nodes[Direction::get_direction(key, node.key).to_usize()].insert_or_update_if(key, value, should_update))}
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
    
    fn internal_get_replacement_key_value(&self, direction: Direction) -> Option<(K,V)>{
        self.0.write().map(|mut write_lock| {
            match &mut *write_lock {
                None => None,
                Some(node) => {
                    match node.child_nodes[direction.to_usize()].internal_get_replacement_key_value(direction){
                        None => {
                            //found replacement node with no node in chosen direction
                            let key_value = (node.key, node.value);
                            //if got opposite direction node, recursively run on that
                            match node.child_nodes[direction.get_opposite().to_usize()].internal_get_replacement_key_value(direction){
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
                            node.child_nodes[Direction::get_direction(key, node.key).to_usize()].remove_if(key, should_remove);
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
                            match node.child_nodes[Direction::Right.to_usize()].internal_get_replacement_key_value(Direction::Left)
                                .or(node.child_nodes[Direction::Left.to_usize()].internal_get_replacement_key_value(Direction::Right)) {
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

}

#[derive(Debug)]
pub struct ConcurrentBSTSet<K>(ConcurrentBSTMap<K, ()>);

impl<K: Copy + Ord> ConcurrentBSTSet<K>{
    
    pub const fn new() -> Self{
        Self(ConcurrentBSTMap::new())
    }

    pub fn len(&self) -> usize{
        self.0.len()
    }

    pub fn clear(&self){
        self.0.clear();
    }

    pub fn contains_key(&self, key: K) -> bool{
        self.0.contains_key(key)
    }

    pub fn insert(&self, key: K){
        self.0.insert_or_update_if(key, (), &|_,_| false);
    }
    
    pub fn remove(&self, key: K){
        self.0.remove(key)
    }
}