use std::hash::Hasher;
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash},
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use mini_redis::{Connection, Frame};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone)]
struct ShardedDb {
    shards: Arc<Vec<Mutex<HashMap<String, Bytes>>>>,
}

impl ShardedDb {
    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        hash as usize % self.shards.len()
    }

    pub fn new(num_shards: usize) -> ShardedDb {
        let mut db = Vec::with_capacity(num_shards);
        for _ in 0..num_shards {
            db.push(Mutex::new(HashMap::new()));
        }

        ShardedDb {
            shards: Arc::new(db),
        }
    }

    pub fn set(&self, key: String, val: Bytes) {
        let idx = self.shard_index(&key);

        {
            let mut guard = self.shards[idx].lock().unwrap();
            guard.insert(key, val);
        }
    }

    pub fn get(&self, key: &str) -> Option<Bytes> {
        let idx = self.shard_index(key);

        let val = {
            let guard = self.shards[idx].lock().unwrap();
            guard.get(key).cloned()
        };

        val
    }
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    let db = ShardedDb::new(8);

    loop {
        let (socket, _) = listener.accept().await.unwrap();

        let db_handle = db.clone();

        tokio::spawn(async move {
            process(socket, db_handle).await;
        });
    }
}

async fn process(socket: TcpStream, db: ShardedDb) {
    use mini_redis::Command::{self, Get, Set};

    let mut connection = Connection::new(socket);

    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                db.set(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }

            Get(cmd) => {
                if let Some(value) = db.get(cmd.key()) {
                    Frame::Bulk(value)
                } else {
                    Frame::Null
                }
            }

            cmd => {
                panic!("unimplemented {:?}", cmd)
            }
        };

        connection.write_frame(&response).await.unwrap();
    }
}
