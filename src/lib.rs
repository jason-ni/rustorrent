#![allow(
    dead_code,
    // clippy::new_without_default,
    // clippy::single_match,
)]

pub mod bencode;
pub mod bitfield;
pub mod cache_line;
pub mod errors;
pub mod extensions;
pub mod fs;
#[cfg(target_os = "linux")]
pub mod io_uring;
pub mod listener;
pub mod logger;
pub mod metadata;
pub mod peer;
pub mod piece_collector;
pub mod piece_picker;
pub mod pieces;
pub mod session;
pub mod sha1;
pub mod sha1_pool;
pub mod spsc;
pub mod time;
pub mod torrent;
pub mod tracker;
pub mod udp_ext;
pub mod utils;
#[cfg(target_os = "linux")]
pub mod utp;

// pub mod memory_pool;

//https://blog.cloudflare.com/how-to-receive-a-million-packets/
