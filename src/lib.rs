use std::sync::RwLock;

#[derive(Debug)]
struct ChildNode<K,V>(RwLock<Option<Box<ConcurrentBSTNode<K,V>>>>);

impl<K: Copy + Ord, V: Copy> ChildNode<K,V>{

    const fn new() -> Self{
        Self(RwLock::new(None))
    }

    fn get_index(target: K, current: K) -> usize{
        if target < current {0} else {1}
    }

    fn len(&self) -> usize{
        self.0.read().map(|read_lock| {
            match &*read_lock{
                None => 0,
                Some(node) => {
                    1 + node.child_nodes[0].len() + node.child_nodes[1].len()
                }
            }
        }).unwrap()
    }
    
    fn get(&self, key: K) -> Option<V>{
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

    fn insert_or_update_if(&self, key: K, value: V, should_update: &impl Fn(&V, &V) -> bool) -> bool{
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
                        *write_lock = Some(Box::new(ConcurrentBSTNode::new(key, value)));
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
    
    fn remove_if(&self, key: K, should_remove: &impl Fn(&V) -> bool){
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
}

#[derive(Debug)]
struct ConcurrentBSTNode<K,V>{
    key: K,
    value: V,
    child_nodes: [ChildNode<K,V>; 2]
}

impl<K: Copy + Ord, V: Copy> ConcurrentBSTNode<K,V>{
    
    const fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            child_nodes: [const { ChildNode::new() }; 2]
        }
    }
}

#[derive(Debug)]
pub struct ConcurrentBSTMap<K, V>(ChildNode<K,V>);

impl<K: Copy + Ord, V: Copy> ConcurrentBSTMap<K,V>{

    pub const fn new() -> Self{
        Self(ChildNode::new())
    }

    pub fn len(&self) -> usize{
        self.0.len()
    }

    pub fn clear(&self){
        *self.0.0.write().unwrap() = None;
    }

    pub fn get(&self, key: K) -> Option<V>{
        self.0.get(key)
    }

    pub fn contains_key(&self, key: K) -> bool{
        self.get(key).is_some()
    }

    pub fn insert_or_update(&self, key: K, value: V)  -> bool{
        self.insert_or_update_if(key, value, &|_,_| true)
    }
    
    pub fn insert_or_update_if(&self, key: K, value: V, should_update: &impl Fn(&V, &V) -> bool) -> bool{
        self.0.insert_or_update_if(key, value, should_update)
    }
    
    pub fn remove(&self, key: K){
        self.remove_if(key, &|_| true)
    }

    pub fn remove_if(&self, key: K, should_remove: &impl Fn(&V) -> bool){
        self.0.remove_if(key, should_remove)
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
        self.0.insert_or_update(key, ());
    }
    
    pub fn remove(&self, key: K){
        self.0.remove(key)
    }
}

