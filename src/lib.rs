use std::{hash::{DefaultHasher, Hash, Hasher}, sync::{Mutex, RwLock}};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
struct ConcurrentBSTNode<K,V>{
    key: K,
    value: V,
    child_nodes: [Option<(bool, K)>; 2]
}

impl<K: Copy,V> ConcurrentBSTNode<K,V>{
    const fn new(key: K, value: V) -> Self{
        Self{
            key,
            value,
            child_nodes: [None; 2]
        }
    }
}

#[derive(Debug)]
struct ConcurrentBSTInternal<K,V>{
    no_elements: Mutex<usize>,
    root_node_key: Mutex<Option<K>>,
    list: Vec<Mutex<Vec<ConcurrentBSTNode<K,V>>>>
}

#[derive(Debug)]
pub struct ConcurrentBST<K,V>{
    inner: RwLock<ConcurrentBSTInternal<K,V>>
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
enum InsertStatus{
    Updated(bool),
    Inserted,
    SizeIncreaseRequired
}

impl<K: Copy + Ord + Eq + Hash, V: Copy> ConcurrentBST<K,V>{

    pub fn new() -> Self{
        Self{
            inner: RwLock::new(ConcurrentBSTInternal{
                no_elements: Mutex::new(0),
                root_node_key: Mutex::new(None),
                list: Vec::from([const {Mutex::new(Vec::new())}; 1024])
            })
        }
    }

    fn get_key_hash(key: K, max_value: usize) -> usize{
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() % (max_value as u64)) as usize
    }

    pub fn add_or_update(&self, key: K, value: V, should_update_value: impl Fn(&V, &V) -> bool) -> bool{
        let inserted = self.inner.read().map(|rw_lock| {
            let inner_function = |start_key| {
                let list_length = rw_lock.list.len();
                let mut inserted = None;
                let mut current_key = start_key;
                let mut current_key_hash;
                loop {
                    match inserted {
                        Some(result) => return result,
                        None => {
                            current_key_hash = Self::get_key_hash(current_key, list_length);
                            let mut continue_loop = true;
                            while continue_loop && inserted.is_none() {
                                rw_lock.list[current_key_hash].lock().map(|mut mutex_lock| {
                                    match mutex_lock.iter().position(|x| x.key == current_key){
                                        Some(index) => {
                                            if current_key == key {
                                                //update
                                                inserted = Some(InsertStatus::Updated(
                                                    if should_update_value(&mutex_lock[index].value, &value) {
                                                        mutex_lock[index].value = value;
                                                        true
                                                    }
                                                    else {false}
                                                ));
                                            }
                                            else {
                                                //go to next child node if not locked
                                                //if locked exit lock and keep reacquiring until not locked
                                                match *mutex_lock[index].child_nodes[if key < current_key {0} else {1}].get_or_insert((false, key)) {
                                                    (true, _) => (),
                                                    (false, child_key) => {
                                                        continue_loop = false;
                                                        current_key = child_key;
                                                    },
                                                }
                                            }
                                        }
                                        None => {
                                            //not found, insert if enough room
                                            rw_lock.no_elements.lock().map(|mut no_elements| {
                                                inserted = Some(
                                                    if *no_elements >= list_length {InsertStatus::SizeIncreaseRequired}
                                                    else{
                                                        mutex_lock.push(ConcurrentBSTNode::new(key, value));
                                                        *no_elements += 1;
                                                        InsertStatus::Inserted
                                                    }
                                                );
                                            }).unwrap();
                                        }
                                    }
                                }).unwrap();
                            }
                        }
                    }
                }
            };

            let mut insert_result = None;

            let root_node_key = *rw_lock.root_node_key.lock().unwrap().get_or_insert_with(|| {
                insert_result = Some(inner_function(key));
                key
            });

            insert_result.unwrap_or(inner_function(root_node_key))
            
        }).unwrap();

        match inserted{
            InsertStatus::Updated(was_updated) => was_updated,
            InsertStatus::Inserted => true,
            InsertStatus::SizeIncreaseRequired => {
                //double the length of the vec if required
                self.inner.write().map(|mut rw_lock| {
                    if *rw_lock.no_elements.lock().unwrap() >= rw_lock.list.len(){
                        let new_vec_length = rw_lock.list.len() * 2;
                        let mut new_vec = Vec::<Mutex<Vec<ConcurrentBSTNode<K,V>>>>::new();
                        for _ in 0..new_vec_length { new_vec.push(Mutex::new(Vec::new())) }
                        for vec in rw_lock.list.iter(){
                            vec.lock().unwrap().iter().for_each(|node| {
                                new_vec[Self::get_key_hash(node.key, new_vec_length)].lock().unwrap().push(*node);
                            })
                        }
                        rw_lock.list = new_vec;
                    }
                }).unwrap();
                
                self.add_or_update(key, value, should_update_value)
            }
        }
    }

    pub fn get(&self, key: K) -> Option<V>{
        self.inner.read().map(|rw_lock| {
            rw_lock.list[Self::get_key_hash(key, rw_lock.list.len())].lock().unwrap()
                .iter().find(|x| x.key == key).map(|x| x.value)
        }).unwrap()
    }
    
    /*pub fn remove(&self, key: K){
        self.remove_if(key, |_| true);
    }*/

    /*pub fn remove_if(&self, key: K, should_remove: impl FnOnce(&V) -> bool){
        self.inner.read().map(|rw_lock| {
            
            let inner_function = |start_key| {
                let list_length = rw_lock.list.len();
                let mut current_key_hash;
                let mut continue_loop;
                //need to find the parent of the node to remove and lock its child node references
                //then 
                
            };
            
            let removed = rw_lock.root_node_key.lock().map(|mutex_lock| {
                match *mutex_lock{
                    None => None,
                    Some(root_node_key) => {
                        if root_node_key == key{
                            //need to keep the mutex lock as root node needs to be removed and replaced if possible
                            inner_function(root_node_key);
                            None
                        }
                        else {Some(root_node_key)}
                    }
                }
            }).unwrap();
            
            match removed{
                None => return,
                Some(root_node_key) => inner_function(root_node_key)
            }
        }).unwrap();
        let inner_function = |found_parent_node| self.inner.read().map(|rw_lock| {
            let list_length = rw_lock.list.len();
            let mut current_key_hash;
            let mut counter;
            let mut continue_loop;
            loop{
                match found_parent_node {
                    Some(possible_parent_node) => { 
                        match possible_parent_node{
                            None => return,
                            Some(parent_node) => {
                                if parent_node == key{
                                    //root node
                                    
                                }
                                else{
                                    
                                }
                            }
                        }
                    }
                    None => {
                        continue_loop = true;
                        counter = 0;
                        current_key_hash = Self::get_key_hash(current_key, list_length);
                        while continue_loop && found_parent_node.is_none() {
                            rw_lock.list[(current_key_hash + counter) % list_length].lock().map(|mutex_lock| {
                                match *mutex_lock {
                                    None => found_parent_node = Some(None),
                                    Some(mut node) => {
                                        if node.key == current_key {
                                            //dont test for if current key == key as this can only happen if at root node which is already tested for
                                            match &mut node.child_nodes[if key < current_key {0} else {1}] {
                                                None => found_parent_node = Some(None),
                                                Some((locked, child_key)) => if !*locked {
                                                    if *child_key == key {
                                                        //lock so cant edit the child node key or any value below in tree without having to lock with mutex
                                                        //doesnt matter if someone else edits this node value as key will remain the same
                                                        *locked = true;
                                                        found_parent_node = Some(Some(current_key));
                                                    }
                                                    else {
                                                        continue_loop = false;
                                                        current_key = *child_key;
                                                    }
                                                }
                                            }
                                        } 
                                        else { counter += 1 }
                                    }
                                }
                            }).unwrap();
                        }
                    }
                }
            }
        }).unwrap();
    }*/

}