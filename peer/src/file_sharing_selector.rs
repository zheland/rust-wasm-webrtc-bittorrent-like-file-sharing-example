use core::borrow::BorrowMut;
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::ops::DerefMut;
use std::collections::HashSet;

use rand::Rng;
use tracker_protocol::{FileSha256, PeerId};

use crate::{FilePieceIdx, FileSharingState};

#[derive(Debug)]
pub struct FileSharingSelector<T, U, V> {
    data: Option<FileSharingSelectorData<T>>,
    rng: U,
    _rng: PhantomData<V>,
    sent: HashSet<(PeerId, FileSha256, FilePieceIdx)>,
}

#[derive(Clone, Debug, Default)]
pub struct FileSharingSelectorData<T> {
    queue_idx: usize,
    states: Vec<QueueItem<T>>,
    len: usize,
}

#[derive(Clone, Debug)]
struct QueueItem<T> {
    sha256: FileSha256,
    state: T,
    len: usize,
}

#[derive(Clone, Debug)]
struct QueueData<T> {
    queue_idx: usize,
    sha256: FileSha256,
    state: T,
    len: usize,
}

impl<T, U, V> FileSharingSelector<T, U, V>
where
    T: DerefMut<Target = FileSharingState>,
    U: BorrowMut<V>,
    V: Rng,
{
    pub fn new<I: IntoIterator<Item = (FileSha256, T)>>(states: I, rng: U) -> Self {
        Self {
            data: FileSharingSelectorData::new(states),
            rng,
            _rng: PhantomData,
            sent: HashSet::new(),
        }
    }
}

impl<T> FileSharingSelectorData<T>
where
    T: DerefMut<Target = FileSharingState>,
{
    pub fn new<I: IntoIterator<Item = (FileSha256, T)>>(states: I) -> Option<Self> {
        let states = states.into_iter().filter_map(|(sha256, state)| {
            let (queue_idx, len) = state
                .pieces()
                .next_queue()
                .map(|queue| (queue.0, queue.1.len()))?;
            Some(QueueData {
                queue_idx,
                sha256,
                state,
                len,
            })
        });
        states.fold(
            None,
            |result: Option<FileSharingSelectorData<T>>, data| match result {
                Some(result) => Some(result.merge(data)),
                None => Some(FileSharingSelectorData::from(data)),
            },
        )
    }

    fn merge(mut self, data: QueueData<T>) -> Self {
        use core::cmp::Ordering;

        match self.queue_idx.cmp(&data.queue_idx) {
            Ordering::Less => Self::from(data),
            Ordering::Equal => {
                self.states.push(QueueItem {
                    sha256: data.sha256,
                    state: data.state,
                    len: data.len,
                });
                self.len += data.len;
                self
            }
            Ordering::Greater => self,
        }
    }

    //pub fn into_states(self) -> Vec<(FileSha256, T)> {
    //    self.states
    //        .into_iter()
    //        .map(|item| (item.sha256, item.state))
    //        .collect()
    //}
}

impl<T> From<QueueData<T>> for FileSharingSelectorData<T> {
    fn from(data: QueueData<T>) -> Self {
        Self {
            queue_idx: data.queue_idx,
            states: vec![QueueItem {
                sha256: data.sha256,
                state: data.state,
                len: data.len,
            }],
            len: data.len,
        }
    }
}

impl<T, U, V> Iterator for FileSharingSelector<T, U, V>
where
    T: DerefMut<Target = FileSharingState>,
    U: BorrowMut<V>,
    V: Rng,
{
    type Item = (PeerId, FileSha256, FilePieceIdx);

    fn next(&mut self) -> Option<Self::Item> {
        match self.data.take() {
            Some(mut data) => {
                let mut idx = self.rng.borrow_mut().gen_range(0..data.len);
                //log::debug!(
                //    "{} {} {:?}",
                //    idx,
                //    data.len,
                //    data.states
                //        .iter_mut()
                //        .map(|v| &*v.state)
                //        .collect::<Vec<_>>()
                //);

                let (state, sha256, state_len, file_piece_idx) = 'outer: loop {
                    for item in data.states.iter_mut() {
                        if idx < item.len {
                            let file_piece_idx = item.state.pieces().next_queue().unwrap().1[idx];
                            break 'outer (
                                item.state.borrow_mut(),
                                item.sha256,
                                &mut item.len,
                                file_piece_idx,
                            );
                        } else {
                            idx -= item.len;
                        }
                    }
                    panic!("index is smaller than lists len sum");
                };

                let peer_id = state.select_peer(file_piece_idx).unwrap();
                *state_len -= 1;
                data.len -= 1;
                if data.len == 0 {
                    self.data = None;
                    // TODO: Do not return same pieces
                    // self.data = FileSharingSelectorData::new(data.into_states());
                } else {
                    self.data = Some(data);
                }

                Some((peer_id, sha256, file_piece_idx))
            }
            None => None,
        }
    }
}

impl<T, U, V> FusedIterator for FileSharingSelector<T, U, V>
where
    T: DerefMut<Target = FileSharingState>,
    U: BorrowMut<V>,
    V: Rng,
{
}
