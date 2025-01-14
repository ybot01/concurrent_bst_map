use std::{net::{Ipv6Addr, SocketAddrV6}, time::{Duration, SystemTime, UNIX_EPOCH}};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, RwLock};
use ed25519_dalek::{ed25519::SignatureBytes, SecretKey};
use rand::random;
use tokio::task::JoinHandle;
use concurrent_bst::ConcurrentBSTMap;

pub(crate) fn timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_millis() as u64
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub(crate) struct User{
    user_id: SecretKey,
    sock_addr: SocketAddrV6,
    update_counter: u64,
    signature: SignatureBytes
}

impl User{
    fn random() -> Self{
        Self{
            user_id: random(),
            sock_addr: SocketAddrV6::new(Ipv6Addr::from(random::<[u8;16]>()), random(), 0, 0),
            update_counter: timestamp(),
            signature: [0;64]
        }
    }
}

fn should_update(user_1: &User, user_2: &User) -> bool{
    user_2.update_counter > user_1.update_counter
}

#[test]
fn length_test(){
    let bst = ConcurrentBSTMap::<SecretKey, User>::new();
    let mut users = Vec::<User>::new();
    let expected = 10000;
    for _ in 0..expected {users.push(User::random())}
    users.iter().for_each(|x| _ = bst.insert_or_update_if(x.user_id, *x, &should_update));
    assert_eq!(bst.len(), expected);
}


#[test]
fn remove_test(){
    let bst = ConcurrentBSTMap::<SecretKey, User>::new();
    let mut users = Vec::<User>::new();
    for _ in 0..10000 {users.push(User::random())}
    users.iter().for_each(|x| _ = bst.insert_or_update_if(x.user_id, *x, &should_update));
    users.iter().for_each(|x| bst.remove(x.user_id));
    assert!(users.iter().all(|x| bst.get(x.user_id).is_none()));
}

#[test]
fn should_update_test() {
    let bst = ConcurrentBSTMap::<SecretKey, User>::new();
    let mut user = User::random();
    assert!(bst.insert_or_update_if(user.user_id, user, &should_update));
    user.update_counter += 1;
    assert!(bst.insert_or_update_if(user.user_id, user, &should_update));
    user.update_counter -= 1;
    assert!(!bst.insert_or_update_if(user.user_id, user, &should_update));
}

#[test]
fn insert_and_get_test() {
    let bst = ConcurrentBSTMap::<SecretKey, User>::new();
    let user = User::random();
    bst.insert_or_update_if(user.user_id, user, &should_update);
    assert!(bst.get(user.user_id).is_some_and(|x| x == user));
}

#[test]
fn bench_insert_or_update_if(){
    let bst = ConcurrentBSTMap::<SecretKey, User>::new();
    let mut user = User::random();
    let mut true_count = 0;
    let total = 1000000;
    let start_time = SystemTime::now();
    for _ in 0..total{
        if bst.insert_or_update_if(user.user_id, user, &should_update) {true_count += 1};
        user.update_counter += 1;
    }
    println!("{}", total as f64 / SystemTime::now().duration_since(start_time).unwrap().as_secs_f64());
    assert_eq!(true_count, total);
}

static GLOBAL_BST: ConcurrentBSTMap<SecretKey, User> = ConcurrentBSTMap::<SecretKey, User>::new();

static TRUE_COUNT: AtomicUsize = AtomicUsize::new(0);

const NO_THREADS: usize = 10;
const TOTAL_PER_THREAD: usize = 100000;

static USER_LIST: LazyLock<RwLock<Vec<User>>> = LazyLock::new(|| {
    let mut list = Vec::<User>::new();
    for _ in 0..(NO_THREADS*TOTAL_PER_THREAD) {list.push(User::random())}
    RwLock::new(list)
});

#[test]
fn bench_multi_thread_insert_or_update_if_and_remove(){
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            _ = USER_LIST.read().unwrap().clone();
            let mut threads= Vec::<JoinHandle<Duration>>::new();
            for i in 0..NO_THREADS{
                threads.push(tokio::spawn(async move{
                    let start_index = TOTAL_PER_THREAD * i;
                    let start_time = SystemTime::now();
                    USER_LIST.read().map(|read_lock| {
                        for i in start_index..(start_index+TOTAL_PER_THREAD) {
                            let user = read_lock[i];
                            if GLOBAL_BST.insert_or_update_if(user.user_id, user, &should_update){
                                TRUE_COUNT.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }).unwrap();
                    SystemTime::now().duration_since(start_time).unwrap()
                }))
            }
            while threads.iter().any(|x| !x.is_finished()) {}
            let mut max_duration = Duration::ZERO;
            let mut duration;
            for i in threads{
                duration = i.await.unwrap();
                if duration > max_duration{
                    max_duration = duration;
                }
            }
            println!("{}", (NO_THREADS*TOTAL_PER_THREAD) as f64 / max_duration.as_secs_f64());
            assert_eq!(TRUE_COUNT.load(Ordering::Relaxed), NO_THREADS*TOTAL_PER_THREAD);
            
            threads = Vec::new();
            for i in 0..NO_THREADS{
                threads.push(tokio::spawn(async move{
                    let start_index = TOTAL_PER_THREAD * i;
                    let start_time = SystemTime::now();
                    USER_LIST.read().map(|read_lock| {
                        for i in start_index..(start_index+TOTAL_PER_THREAD) {
                            GLOBAL_BST.remove(read_lock[i].user_id);
                        }
                    }).unwrap();
                    SystemTime::now().duration_since(start_time).unwrap()
                }))
            }
            while threads.iter().any(|x| !x.is_finished()) {}
            max_duration = Duration::ZERO;
            for i in threads{
                duration = i.await.unwrap();
                if duration > max_duration{
                    max_duration = duration;
                }
            }
            println!("{}", (NO_THREADS*TOTAL_PER_THREAD) as f64 / max_duration.as_secs_f64());
        });
}