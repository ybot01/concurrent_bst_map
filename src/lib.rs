/*use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{RwLockReadGuard, RwLockWriteGuard};
use std::{hash::{DefaultHasher, Hash, Hasher}, sync::{Mutex, RwLock}};

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
    no_elements: AtomicUsize,
    root_node_key: Option<K>,
    list: Vec<Mutex<Vec<ConcurrentBSTNode<K,V>>>>
}

#[derive(Debug)]
pub struct ConcurrentBST<K,V>{
    inner: RwLock<ConcurrentBSTInternal<K,V>>
}

pub trait ShouldUpdate{
    fn should_update_to(&self, other: &Self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
enum InsertStatus{
    Updated(bool),
    Inserted(bool)
}

enum LockGuard<'a, K, V> {
    Read(RwLockReadGuard<'a, ConcurrentBSTInternal<K,V>>),
    Write(RwLockWriteGuard<'a, ConcurrentBSTInternal<K,V>>),
}

impl<'a, K, V> Deref for LockGuard<'a, K, V>{
    type Target = ConcurrentBSTInternal<K,V>;

    fn deref(&self) -> &Self::Target {
        match self{
            LockGuard::Read(lock) => lock,
            LockGuard::Write(lock) => lock
        }
    }
}

impl<K: Copy + Ord + Eq + Hash, V: Copy + ShouldUpdate> ConcurrentBST<K,V>{

    pub fn new() -> Self{
        Self{
            inner: RwLock::new(ConcurrentBSTInternal{
                no_elements: AtomicUsize::new(0),
                root_node_key: None,
                list: Vec::from([const {Mutex::new(Vec::new())}; 1024])
            })
        }
    }

    fn get_key_hash(key: K, max_value: usize) -> usize{
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() % (max_value as u64)) as usize
    }

    pub fn get(&self, key: K) -> Option<V>{
        self.inner.read().map(|rw_lock| {
            rw_lock.list[Self::get_key_hash(key, rw_lock.list.len())].lock().unwrap()
                .iter().find(|x| x.key == key).map(|x| x.value)
        }).unwrap()
    }

    pub fn add_or_update(&self, key: K, value: V) -> bool{

        //need to figure out how to do removals using a read lock instead of write lock for efficiency
        //example problem is if user gets the root node key and then tries to find that node
        //could be removed from the bst and replaced by different root node before can read it so will never find it
        //therefore if removing the root node key need to write lock it to ensure exclusive access
        //remove and replace with new or none if no child nodes
        //if removing a none root node then only care about the nodes parent node reference to the node and everything below that so mark that as locked
        //mark as unlocked once removed

        let inner_function = |root_node_key, lock_guard: LockGuard<K, V>| {
            let mut current_key = root_node_key;
            let mut current_key_hash;
            let list_length = lock_guard.list.len();
            let mut inserted = None;
            let mut continue_loop;
            loop {
                match inserted {
                    Some(result) => return result,
                    None => ()
                }
                current_key_hash = Self::get_key_hash(current_key, list_length);
                continue_loop = true;
                while continue_loop && inserted.is_none() {
                    lock_guard.list[current_key_hash].lock().map(|mut mutex_lock| {
                        match mutex_lock.iter().position(|x| x.key == current_key){
                            Some(index) => {
                                if current_key == key {
                                    //update
                                    inserted = Some(InsertStatus::Updated(
                                        if mutex_lock[index].value.should_update_to(&value){
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
                                //not found
                                if current_key == key{
                                    mutex_lock.push(ConcurrentBSTNode::new(key, value));
                                    inserted = Some(InsertStatus::Inserted(
                                        lock_guard.no_elements.fetch_add(1, Ordering::Relaxed) >= list_length
                                    ));
                                }
                            }
                        }
                    }).unwrap();
                }
            }
        };
        match self.inner.read().map(|read_lock|
            read_lock.root_node_key.map(|x| inner_function(x, LockGuard::Read(read_lock)))
        ).unwrap().unwrap_or(
            self.inner.write().map(|mut write_lock|
                inner_function(*write_lock.root_node_key.get_or_insert(key), LockGuard::Write(write_lock))
            ).unwrap()
        ){
            InsertStatus::Updated(was_updated) => was_updated,
            InsertStatus::Inserted(false) => true,
            InsertStatus::Inserted(true) => {
                self.inner.write().map(|mut rw_lock| {
                    //resize the array if required
                    let mut new_vec_length = rw_lock.list.len();
                    while rw_lock.no_elements.load(Ordering::Relaxed) >= new_vec_length {new_vec_length *= 2}
                    if new_vec_length > rw_lock.list.len(){
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
                true
            }
        }
    }

    pub fn remove(&self, key: K){
        self.remove_if(key, |_| true);
    }

    pub fn remove_if(&self, key: K, should_remove: impl Fn(&V) -> bool){
        //need to find the node which has the node to remove as one of its child node references
        //(if reach an empty reference then exit, key is not in map)
        //this is the parent node, lock the child node reference of the parent node that is the key to remove
        //first go to child node which is the key to remove and check if should be removed, if not then unlock the parent and exit
        //then find the next biggest key after the key to remove by going to right child node and then going to left node until reach a leaf node
        //if there is no right child node of key to remove then simply replace the parent child node with the left child node of the key to remove
        //if no child nodes then set the parent child node to empty and unlock
        //if exists then replace the parent child node with this node and set the child nodes of this node to the child nodes of the key to remove
        //after done all this then delete the user
        //if key to delete is the root node then need to lock root node mutex
        //replace the root node with next biggest node if exists and change the child nodes
        //else if only left child node set root node to left node
        //else if no child nodes then set root node to none


        if !self.inner.read().map(|read_lock| {
            let root_node_key = match read_lock.root_node_key {
                None => return true,
                Some(root_node_key) => {
                    if root_node_key == key {return false}
                    else {root_node_key}
                }
            };
            //first find parent node

            let mut current_key = root_node_key;
            let mut current_key_hash;
            let list_length = read_lock.list.len();
            let mut cni;
            let mut parent_node_internal = None;
            let mut continue_loop;
            let parent_node = loop{
                match parent_node_internal{
                    Some(result) => break result,
                    None => ()
                }
                current_key_hash = Self::get_key_hash(current_key, list_length);
                continue_loop = true;
                cni = if key < current_key {0} else {1};
                while continue_loop && !parent_node_internal.is_none() {
                    read_lock.list[current_key_hash].lock().map(|mut mutex_lock| {
                        match mutex_lock.iter().position(|x| x.key == current_key){
                            Some(index) => {
                                match &mut mutex_lock[index].child_nodes[cni]{
                                    Some((locked, child_key)) => if !*locked{

                                    }
                                    None => {
                                        //item not here so mark removed
                                        parent_node_internal = Some(None);
                                    }
                                }
                            }
                            None => ()
                        }
                        while (counter < mutex_lock.len()) && (mutex_lock[counter].key != current_key) {counter += 1}
                        if counter < mutex_lock.len() {
                            match &mut mutex_lock[counter].child_nodes[cni]{
                                None => {

                                },
                                Some((locked, child_key)) => if !*locked{
                                    if *child_key == key{
                                        *locked = true;
                                        parent_node_internal = Some(Some(current_key));
                                    }
                                    else {
                                        current_key = *child_key;
                                        continue_loop = false;
                                    }
                                }
                            }
                        }
                    }).unwrap();
                }
            }
            true
        }).unwrap(){
            self.inner.write().map(|write_lock| {
                //todo
            }).unwrap();
        }
    }

}*/

use std::sync::RwLock;

pub trait ShouldUpdate {
    fn should_update_to(&self, other: &Self) -> bool;
}

#[derive(Debug)]
struct ChildNode<K,V>(RwLock<Option<Box<ConcurrentBSTNode<K,V>>>>);

impl<K: Copy + Ord, V: Copy + ShouldUpdate> ChildNode<K,V>{
    const fn new() -> Self{
        Self(RwLock::new(None))
    }

    fn get_index(target: K, current: K) -> usize{
        if target < current {0} else {1}
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

    fn add_or_update(&self, key: K, value: V) -> bool{
        loop{
            match self.0.read().map(|read_lock| {
                match &*read_lock{
                    None => None,
                    Some(node) => {
                        if node.key != key {Some(node.child_nodes[Self::get_index(key, node.key)].add_or_update(key, value))}
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
                        if node.key == key{
                            //update
                            Some(
                                if node.value.should_update_to(&value){
                                    node.value = value;
                                    true
                                }
                                else {false}
                            )
                        }
                        //if a different key than before then retry the read lock
                        else {None}
                    }
                }
            }).unwrap(){
                None => (),
                Some(result) => return result
            }
        }
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
                match &*write_lock{
                    None => true,
                    Some(node) => {
                        if node.key == key{
                            if should_remove(&node.value){
                                match (&*node.child_nodes[0].0.read().unwrap(), &*node.child_nodes[1].0.read().unwrap()){
                                    //todo
                                    (None, None) => {
                                        //if no child nodes then can simply delete it
                                        *write_lock = None;
                                    },
                                    (None, Some(rcn)) => {
                                        
                                    },
                                    (Some(lcn), None) => {
                                        //if only left child then replace
                                        *write_lock = Some(lcn)
                                    },
                                    (Some(lcn), Some(rcn)) => {

                                    }
                                }
                            }
                            true
                        }
                        //if a different key than before then retry the read lock
                        else {false}
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

impl<K: Copy + Ord, V: Copy + ShouldUpdate> ConcurrentBSTNode<K,V>{
    const fn new(key: K, value: V) -> Self{
        Self{
            key,
            value,
            child_nodes: [const {ChildNode::new()}; 2]
        }
    }
    
    /*fn get(&self, key: K) -> Option<V>{
        self.child_nodes[if key < self.key {0} else {1}].read().map(|read_lock| {
            match &*read_lock{
                None => None,
                Some(child_node) => { 
                    if child_node.key == key {Some(child_node.value)}
                    else {child_node.get(key)}
                }
            }
        }).unwrap()
    }
    
    fn add_or_update(&self, key: K, value: V) -> bool{
        let index = if key < self.key {0} else {1};
        let mut insert_status = None;
        loop{
            self.child_nodes[index].read().map(|read_lock| {
                match &*read_lock{
                    None => (),
                    Some(child_node) => if child_node.key != key {insert_status = Some(child_node.add_or_update(key, value))}
                }
            }).unwrap();
            match insert_status{
                Some(result) => return result,
                None => ()
            }
            self.child_nodes[index].write().map(|mut write_lock| {
                match &mut *write_lock{
                    None => {
                        //insert
                        *write_lock = Some(Box::new(ConcurrentBSTNode::new(key, value)));
                        insert_status = Some(true);
                    }
                    Some(child_node) => {
                        if child_node.key == key{
                            //update
                            insert_status = Some(
                                if child_node.value.should_update_to(&value){
                                    child_node.value = value;
                                    true
                                }
                                else {false}
                            );
                        }
                        //if a different key than before then retry the read lock
                    }
                }

            }).unwrap();
            match insert_status{
                Some(result) => return result,
                None => ()
            }
        }
    }
    
    fn remove_if(&self, key: K, should_remove: &impl Fn(&V) -> bool){
        let index = if key < self.key {0} else {1};
        let mut removed = false;
        loop{
            self.child_nodes[index].read().map(|read_lock| {
                match &*read_lock{
                    None => removed = true,
                    Some(child_node) => {
                        if child_node.key != key {
                            removed = true;
                            child_node.remove_if(key, should_remove);
                        }
                    }
                }
            }).unwrap();
            if removed {return}
            self.child_nodes[index].write().map(|mut write_lock| {
                let child_nodes_exist = match &*write_lock{
                    None => {
                        removed = true;
                        return;
                    }
                    Some(child_node) => {
                        if child_node.key == key{
                            if should_remove(&child_node.value){
                                (child_node.child_nodes[0].read().unwrap().is_some(), child_node.child_nodes[1].read().unwrap().is_some())
                            }
                            else{
                                removed = true;
                                return;
                            }
                        }
                        else {return}
                    }
                };

                match child_nodes_exist{
                    (false, false) => {
                        //if no child nodes then can simply delete it
                        *write_lock = None;
                    },
                    (false, true) => {

                    },
                    (true, false) => {
                        //if only left child then replace
                        write_lock = Some()
                    },
                    (true, true) => {

                    }
                }
            }).unwrap();
            if removed {return}
        }
    }*/
}

#[derive(Debug)]
pub struct ConcurrentBST<K, V>(RwLock<ChildNode<K,V>>);

impl<K: Copy + Ord, V: Copy + ShouldUpdate> ConcurrentBST<K,V>{

    pub const fn new() -> Self{
        Self(RwLock::new(ChildNode::new()))
    }

    pub fn get(&self, key: K) -> Option<V>{
        self.0.read().unwrap().get(key)
    }
    
    pub fn add_or_update(&self, key: K, value: V) -> bool{
        self.0.read().unwrap().add_or_update(key, value)
    }
    
    pub fn remove(&self, key: K){
        self.remove_if(key, &|_| true)
    }

    pub fn remove_if(&self, key: K, should_remove: &impl Fn(&V) -> bool){
        self.0.read().unwrap().remove_if(key, should_remove)
    }
}