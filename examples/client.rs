use tokio::sync::mpsc;
use tokio::sync::oneshot;

use mini_redis::client;

use bytes::Bytes;

type Responder<T> = oneshot::Sender<mini_redis::Result<T>>;

#[derive(Debug)]
pub enum Command {
    Get {
        key: String,

        // this is the Sender (tx) handle the manager uses
        // to respond to each task that's issued a request
        rsp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        val: Bytes,

        // this is the Sender (tx) handle the manager uses
        // to respond to each task that's issued a request
        rsp: Responder<()>,
    },
}

#[tokio::main]
async fn main() {
    // create multi producer, single consumer channel to send 
    // messages from tasks to this manager
    let (tx, mut rx) = mpsc::channel::<Command>(32);

    let manager = tokio::spawn(async move {
        // Establish a connection to the server
        let mut client = client::connect("127.0.0.1:6379").await.unwrap();

        while let Some(cmd) = rx.recv().await {
            match cmd {
                Command::Get { key, rsp } => {
                    let res = client.get(&key).await;

                    // send response to task
                    let _ = rsp.send(res);
                }
                Command::Set { key, val, rsp } => {
                    let res = client.set(&key, val).await;

                    // send response to task
                    let _ = rsp.send(res);
                }
            }
        }
    });

    let tx2 = tx.clone();

    let t1 = tokio::spawn(async move {
        // create oneshot channel (unbuffered 1-1)
        // to recieve responses back from the manager
        let (rsp_tx, rsp_rx) = oneshot::channel();

        let cmd = Command::Get {
            key: "foo".to_string(),
            rsp: rsp_tx,
        };

        // send message to manager through mpsc chan
        tx.send(cmd).await.unwrap();

        // await response from manager through the oneshot chan
        let res = rsp_rx.await;
        println!("get GOT = {:?}", res);
    });

    let t2 = tokio::spawn(async move {
        // create oneshot channel (unbuffered 1-1)
        // to recieve responses back from the manager
        let (rsp_tx, rsp_rx) = oneshot::channel();

        let cmd = Command::Set {
            key: "foo".to_string(),
            val: "bar".into(),
            rsp: rsp_tx,
        };

        // send message to manager through mpsc chan
        tx2.send(cmd).await.unwrap();

        // await response from manager through the oneshot chan
        let res = rsp_rx.await;
        println!("set GOT = {:?}", res);
    });

    t1.await.unwrap();
    t2.await.unwrap();
    manager.await.unwrap();
}
