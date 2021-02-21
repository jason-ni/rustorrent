use rustorrent::utp::{listener, UtpError, stream::UtpStream, UtpState};
use std::env;
use std::sync::Arc;
use shared_arena::SharedArena;

async fn handle_conn(conn: UtpStream, arena: Arc<SharedArena<[u8; 64]>>) -> Result<(), UtpError> {
    log::debug!("pending on read stream");
    let mut buf = arena.alloc_with(|b|{
        let buf_ref = unsafe { &*(b.as_mut_ptr() as *mut [u8; 64])};
        buf_ref
    });
    let _size = conn.read(buf.as_mut()).await?;
    log::debug!("received data: {}", String::from_utf8_lossy(buf.as_ref()));
    Ok(())
}

async fn run() -> Result<(), UtpError> {
    let mut args = env::args();
    args.next();
    let listen_addr = args.next().expect("listen addr must input");
    let l = listener::UtpListener::bind(listen_addr).await;
    let arena: SharedArena<[u8; 64]> = SharedArena::new();
    let arena_ref = Arc::new(arena);
    loop {
        let (conn, peer) = l.accept().await;
        log::debug!("accepting connection from {:?}", peer);
        tokio::spawn(handle_conn(conn, arena_ref.clone()));
    }
}

fn main() {
    env_logger::init();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let fut = async {
        run().await.map_err(|e| log::debug!("run exit with error: {:?}", e))
    };
    let _ = rt.block_on(fut);
}