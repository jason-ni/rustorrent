
use std::sync::Arc;
use parking_lot::{RwLock, RwLockReadGuard};
use async_std::prelude::*;
use async_std::task;
use async_std::sync as a_sync;
use sha1::Sha1;

use std::net::{SocketAddr, ToSocketAddrs};

use crate::actors::peer::{PeerId, Peer, PeerTask, PeerCommand};
use crate::metadata::Torrent;
use crate::bitfield::{BitFieldUpdate, BitField};
use crate::pieces::{PieceInfo, Pieces, PieceBuffer, PieceToDownload};
use crate::session::Tracker;
use crate::utils::Map;
use crate::errors::TorrentError;

struct PeerState {
    bitfield: BitField,
    queue_tasks: PeerTask,
    addr: a_sync::Sender<PeerCommand>
}

pub enum PeerMessage {
    AddPeer {
        id: PeerId,
        queue: PeerTask,
        addr: a_sync::Sender<PeerCommand>
    },
    RemovePeer {
        id: PeerId ,
        queue: PeerTask
    },
    AddPiece(PieceBuffer),
    UpdateBitfield {
        id: PeerId,
        update: BitFieldUpdate
    }
}

impl std::fmt::Debug for PeerMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use PeerMessage::*;
        match self {
            AddPeer { id, .. } => {
                f.debug_struct("PeerMessage")
                 .field("AddPeer", &id)
                 .finish()
            }
            RemovePeer { id, .. } => {
                f.debug_struct("PeerMessage")
                 .field("RemovePeer", &id)
                 .finish()
            }
            AddPiece(piece) => {
                f.debug_struct("PeerMessage")
                 .field("AddPiece", &piece.piece_index)
                 .finish()
            }
            UpdateBitfield { id, .. } => {
                f.debug_struct("PeerMessage")
                 .field("UpdateBitfield", &id)
                 .finish()
            }
        }
    }
}

pub struct TorrentSupervisor {
    metadata: Torrent,
    trackers: Vec<Tracker>,
    receiver: a_sync::Receiver<PeerMessage>,
    // We keep a Sender to not close the channel
    // in case there is no peer
    _sender: a_sync::Sender<PeerMessage>,

    pieces_detail: Pieces,

    peers: Map<PeerId, PeerState>,

    pieces: Vec<Option<PieceInfo>>,
}

pub type Result<T> = std::result::Result<T, TorrentError>;

impl TorrentSupervisor {
    pub fn new(torrent: Torrent) -> TorrentSupervisor {
        let (_sender, receiver) = a_sync::channel(100);
        let pieces_detail = Pieces::from(&torrent);

        let num_pieces = pieces_detail.num_pieces;
        let mut pieces = Vec::with_capacity(num_pieces);
        pieces.resize_with(num_pieces, Default::default);

        TorrentSupervisor {
            metadata: torrent,
            receiver,
            _sender,
            pieces_detail,
            pieces,
            peers: Default::default(),
            trackers: vec![],
        }
    }

    pub async fn start(&mut self) {
        self.collect_trackers();

        if let Some(addrs) = self.find_tracker() {
            self.connect_to_peers(&addrs);
        }

        self.process_cmds().await;
    }

    fn collect_trackers(&mut self) {
        let trackers = self.metadata.iter_urls().map(Tracker::new).collect();
        self.trackers = trackers;
    }

    fn connect_to_peers(&self, addrs: &[SocketAddr]) {
        for addr in addrs {
            println!("ADDR: {:?}", addr);

            let addr = *addr;
            let sender = self._sender.clone();
            let pieces_detail = self.pieces_detail.clone();

            task::spawn(async move {
                let mut peer = match Peer::new(addr, pieces_detail, sender).await {
                    Ok(peer) => peer,
                    Err(e) => {
                        println!("PEER ERROR {:?}", e);
                        return;
                    }
                };
                peer.start().await;
            });
        }
    }

    fn find_tracker(&mut self) -> Option<Vec<SocketAddr>> {
        let torrent = &self.metadata;

        loop {
            for tracker in &mut self.trackers {
                println!("TRYING {:?}", tracker);
                match tracker.announce(&torrent) {
                    Ok(peers) => return Some(peers),
                    Err(e) => {
                        eprintln!("[Tracker announce] {:?}", e);
                        continue;
                    }
                };
            }
        }
        None
    }

    async fn process_cmds(&mut self) {
        use PeerMessage::*;

        while let Some(msg) = self.receiver.recv().await {
            match msg {
                UpdateBitfield { id, update } => {
                    if self.find_pieces_for_peer(id, &update).await {
                        let peer = self.peers.get(&id).unwrap();
                        peer.addr.send(PeerCommand::TasksAvailables).await;
                    }

                    if let Some(peer) = self.peers.get_mut(&id) {
                        peer.bitfield.update(update);
                    };
                }
                RemovePeer { id, queue } => {
                    self.peers.remove(&id);

                    for piece in self.pieces.iter_mut().filter_map(Option::as_mut) {
                        piece.workers.retain(|p| !Arc::ptr_eq(&p, &queue) );
                    }
                }
                AddPeer { id, queue, addr } => {
                    self.peers.insert(id, PeerState {
                        bitfield: BitField::new(self.pieces_detail.num_pieces),
                        queue_tasks: queue,
                        addr
                    });
                }
                AddPiece (piece_block) => {
                    let index = piece_block.piece_index;
                    let sha1_torrent = self.pieces_detail.sha1_pieces.get(index).map(Arc::clone);

                    if let Some(sha1_torrent) = sha1_torrent {
                        let sha1 = Sha1::from(&piece_block.buf).digest();
                        let sha1 = sha1.bytes();
                        if sha1 == sha1_torrent.as_slice() {
                            //println!("SHA1 ARE GOOD !! {}", piece_block.piece_index);
                        } else {
                            println!("WRONG SHA1 :() {}", piece_block.piece_index);
                        }
                    } else {
                        println!("PIECE RECEIVED BUT NOT FOUND {}", piece_block.piece_index);
                    }

                    println!("[S] PIECE RECEIVED {} {}", piece_block.piece_index, piece_block.buf.len());
                }
            }
        }
    }

    async fn find_pieces_for_peer(&mut self, peer: PeerId, update: &BitFieldUpdate) -> bool {
        let mut pieces = &mut self.pieces;
        let nblock_piece = self.pieces_detail.nblocks_piece;
        let block_size = self.pieces_detail.block_size;

        let queue_peer = self.peers.get_mut(&peer).map(|p| &mut p.queue_tasks).unwrap();
        let mut queue = queue_peer.write().await;

        let mut found = false;

        match update {
            BitFieldUpdate::BitField(bitfield) => {
                let pieces = pieces.iter_mut()
                                   .enumerate()
                                   .filter(|(index, p)| p.is_none() && bitfield.get_bit(*index))
                                   .take(20);

                for (piece, value) in pieces {
                    for i in 0..nblock_piece {
                        queue.push_back(PieceToDownload::new(piece, i * block_size, block_size));
                    }
                    //println!("[{:?}] PUSHING PIECE={}", peer.id, piece);
                    value.replace(PieceInfo::new(queue_peer.clone()));
                    if !found {
                        found = true;
                    }
                }
            }
            BitFieldUpdate::Piece(piece) => {
                let piece = *piece;

                if piece >= pieces.len() {
                    return false;
                }

                if pieces.get(piece).unwrap().is_none() {
                    for i in 0..nblock_piece {
                        queue.push_back(PieceToDownload::new(piece, i * block_size, block_size));
                    }
                    //println!("[{:?}] _PUSHING PIECE={}", peer.id, piece);
                    pieces.get_mut(piece).unwrap().replace(PieceInfo::new(queue_peer.clone()));
                    found = true;
                }

            }
        }

        found
    }
}
