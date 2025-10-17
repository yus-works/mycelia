use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::{self, io};

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = TcpStream::connect("127.0.0.1:6142").await?;
    let (mut rd, mut wr) = socket.into_split();

    tokio::spawn(async move {
        wr.write_all(b"hello\n\n").await?;
        wr.write_all(b"world\n\n").await?;

        Ok::<_, io::Error>(())
    });

    let mut buf = vec![0; 128];

    loop {
        let n = rd.read(&mut buf).await?;

        if n == 0 {
            break;
        }

        println!("GOT {}", String::from_utf8_lossy(&buf[..n]));
    }
    Ok(())
}
