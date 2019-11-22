use serde::{Serialize, Deserialize};
use smallvec::SmallVec;
use url::Url;

use std::iter::Iterator;
use std::sync::Arc;

type StackVec<T> = SmallVec<[T; 16]>;

// #[derive(Debug, Serialize, Deserialize)]
// pub struct MetaFile<'a> {
//     pub length: i64,
//     pub md5sum: Option<&'a str>,
//     pub path: StackVec<&'a str>,
// }

// #[derive(Debug, Serialize, Deserialize)]
// #[serde(untagged)]
// pub enum InfoFile<'a> {
//     Single {
//         name: &'a str,
//         length: i64,
//         md5sum: Option<&'a str>,
//     },
//     Multiple {
//         name: &'a str,
//         files: StackVec<MetaFile<'a>>
//     },
// }

// #[derive(Serialize, Deserialize)]
// pub struct MetaInfo<'a> {
//     #[serde(with = "serde_bytes")]
//     pub pieces: &'a [u8],
//     #[serde(rename="piece length")]
//     pub piece_length: i64,
//     pub private: Option<i64>,
//     #[serde(flatten)]
//     pub files: InfoFile<'a>,
// }

// impl<'a> std::fmt::Debug for MetaInfo<'a> {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         f.debug_struct("Info")
//          .field("piece_length", &self.piece_length)
//          .field("pieces", &&self.pieces[..10])
//          .field("files", &self.files)
//          .finish()
//     }
// }

// #[derive(Debug, Serialize, Deserialize)]
// pub struct MetaTorrent<'a> {
//     pub announce: &'a str,
//     pub info: MetaInfo<'a>,
//     #[serde(rename="announce-list")]
//     pub announce_list: Option<StackVec<StackVec<&'a str>>>,
//     #[serde(rename="creation date")]
//     pub creation_date: Option<u64>,
//     pub comment: Option<&'a str>,
//     #[serde(rename="created by")]
//     pub created_by: Option<&'a str>,
//     pub encoding: Option<&'a str>
// }

#[derive(Debug, Serialize, Deserialize)]
pub struct MetaFile {
    pub length: i64,
    pub md5sum: Option<String>,
    pub path: StackVec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InfoFile {
    Single {
        name: String,
        length: i64,
        md5sum: Option<String>,
    },
    Multiple {
        name: String,
        files: StackVec<MetaFile>
    },
}

#[derive(Serialize, Deserialize)]
pub struct MetaInfo {
    #[serde(with = "serde_bytes")]
    pub pieces: Vec<u8>,
    #[serde(rename="piece length")]
    pub piece_length: i64,
    pub private: Option<i64>,
    #[serde(flatten)]
    pub files: InfoFile,
}

impl<'a> std::fmt::Debug for MetaInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Info")
         .field("piece_length", &self.piece_length)
         .field("pieces", &&self.pieces[..10])
         .field("files", &self.files)
         .finish()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetaTorrent {
    pub announce: String,
    pub info: MetaInfo,
    #[serde(rename="announce-list")]
    pub announce_list: Option<StackVec<StackVec<String>>>,
    #[serde(rename="creation date")]
    pub creation_date: Option<u64>,
    pub comment: Option<String>,
    #[serde(rename="created by")]
    pub created_by: Option<String>,
    pub encoding: Option<String>
}

#[derive(Debug)]
pub struct Torrent {
    pub meta: MetaTorrent,
    pub info_hash: Arc<Vec<u8>>,
}

pub struct UrlIterator<'a> {
    list: Vec<&'a str>,
    index: usize
}

impl<'a> Iterator for UrlIterator<'a> {
    type Item = Url;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let index = self.index;
            self.index += 1;
            match self.list.get(index).map(|u| u.parse()) {
                Some(Ok(url)) => return Some(url),
                None => return None,
                _ => {}
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.list.len(), None)
    }
}

impl Torrent {
    pub fn iter_urls(&self) -> UrlIterator {
        let mut vec = vec![];

        if let Some(list) = &self.meta.announce_list {
            for l in list {
                for inner in l {
                    vec.push(inner.as_ref());
                }
            }
        };

        vec.push(self.meta.announce.as_ref());

        println!("TRACKERS={:?}", vec);

        UrlIterator {
            index: 0,
            list: vec,
        }
    }

    pub fn files_total_size(&self) -> usize {
        match &self.meta.info.files {
            InfoFile::Single { length, .. } => {
                *length as usize
            },
            InfoFile::Multiple { files, .. } => {
                files.iter().fold(0, |acc, f| acc + f.length as usize)
            }
        }
    }

    pub fn sha_pieces(&self) -> Vec<Arc<Vec<u8>>> {
        let pieces = self.meta.info.pieces.as_slice();
        let mut vec = Vec::with_capacity(pieces.len() / 20);

        println!("PIECES LEN = {:?}", pieces.len());

        for piece in pieces.chunks_exact(20) {
            let mut bytes = Vec::with_capacity(20);
            unsafe { bytes.set_len(20) }
            bytes.as_mut_slice().copy_from_slice(piece);
            vec.push(Arc::new(bytes));
        }

        vec
    }
}
