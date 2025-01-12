use std::{net::{Ipv6Addr, SocketAddrV6}, time::{Duration, SystemTime, UNIX_EPOCH}};
use std::sync::atomic::{AtomicUsize, Ordering};
use ed25519_dalek::{ed25519::SignatureBytes, SecretKey};
use rand::random;
use tokio::task::JoinHandle;
use concurrent_bst::alternative_idea::{ConcurrentBST, ShouldUpdate};
//use concurrent_bst::{ConcurrentBST, ShouldUpdate};

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
    
    const DEFAULT: Self = Self{
        user_id: [0;32],
        sock_addr: SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0),
        update_counter: 0,
        signature: [0;64]
    };
    
    fn random() -> Self{
        Self{
            user_id: random(),
            sock_addr: SocketAddrV6::new(Ipv6Addr::from(random::<[u8;16]>()), random(), 0, 0),
            update_counter: timestamp(),
            signature: [0;64]
        }
    }
}

impl ShouldUpdate for User{
    fn should_update_to(&self, other: &Self) -> bool {
        other.update_counter > self.update_counter
    }
}

#[test]
fn test() {
    let bst = ConcurrentBST::<SecretKey, User>::new([0;32], User::random());
    let mut user = User::random();
    assert!(bst.add_or_update(user.user_id, user));
    user.update_counter += 1;
    assert!(bst.add_or_update(user.user_id, user));
    user.update_counter -= 1;
    assert!(!bst.add_or_update(user.user_id, user));
}

#[test]
fn insert_and_get_test() {
    let bst = ConcurrentBST::<SecretKey, User>::new([0;32], User::random());
    let user = User::random();
    bst.add_or_update(user.user_id, user);
    assert!(bst.get(user.user_id).is_some_and(|x| x == user));
}

#[test]
fn bench(){
    let bst = ConcurrentBST::<SecretKey, User>::new([0;32], User::random());
    let mut user = User::random();
    let mut true_count = 0;
    let total = 1000000;
    let start_time = SystemTime::now();
    for _ in 0..total{
        if bst.add_or_update(user.user_id, user) {true_count += 1};
        user.update_counter += 1;
    }
    println!("{}", total as f64 / SystemTime::now().duration_since(start_time).unwrap().as_secs_f64());
    assert_eq!(true_count, total);
}

static GLOBAL_BST: ConcurrentBST<SecretKey, User> = ConcurrentBST::<SecretKey, User>::new([0;32], User::DEFAULT);

static TRUE_COUNT: AtomicUsize = AtomicUsize::new(0);

#[test]
fn bench_multi_thread(){
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            const NO_THREADS: usize = 10;
            const TOTAL_PER_THREAD: usize = 100000;
            let mut threads= Vec::<JoinHandle<Duration>>::new();
            for _ in 0..NO_THREADS{
                threads.push(tokio::spawn(async{
                    let mut random_users = Vec::<User>::new();
                    for _ in 0..TOTAL_PER_THREAD {random_users.push(User::random())}
                    let start_time = SystemTime::now();
                    for user in random_users{
                        if GLOBAL_BST.add_or_update(user.user_id, user){
                            TRUE_COUNT.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    SystemTime::now().duration_since(start_time).unwrap()
                }))
            }
            while threads.iter().any(|x| !x.is_finished()) {}
            let mut max_duration = Duration::ZERO;
            for i in threads{
                let duration = i.await.unwrap();
                if duration > max_duration{
                    max_duration = duration;
                }
            }
            println!("{}", (NO_THREADS*TOTAL_PER_THREAD) as f64 / max_duration.as_secs_f64());
            //test how many have been filled, is it = no threads * total per thread?
            println!("{:?}", TRUE_COUNT);
        });
}