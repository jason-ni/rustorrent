use std::env;
use rustorrent::utp::{listener, UtpError};
use std::net::SocketAddr;
use tokio::fs;
use tokio::io::AsyncWriteExt;

async fn run() -> Result<(), UtpError> {
    let mut args = env::args();
    args.next();
    let listen_ip = args.next().expect("listen ip must input");
    let remote_addr_arg = args.next().expect("remote address must input");
    let remote_addr: SocketAddr = remote_addr_arg.parse().unwrap();
    let file_name = args.next().expect("send file name must input");
    log::debug!("before open");
    let mut f = fs::File::open(file_name).await?;
    log::debug!("before connect");
    let l = listener::UtpListener::bind(format!("{}:0", listen_ip)).await;
    let mut conn = l.connect(remote_addr).await?;
    conn.write_all("hello".as_bytes()).await?;
    Ok(())
}
fn main() {
    env_logger::init();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let fut = async {
        run().await.map_err(|e| log::debug!("run exit with error: {:?}", e));
    };
    let _ = rt.block_on(fut);
}