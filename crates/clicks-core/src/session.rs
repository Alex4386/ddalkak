//! Poll-based request queue for transports that are driven by callbacks
//! (CoreBluetooth delegates, Android `BluetoothGattCallback`, ...).
//!
//! ```text
//! session.enqueue(...);                       // or set_* helpers
//! while let Some(frame) = session.poll_transmit() { write(frame) }
//! on_notify(bytes) => session.handle_notification(bytes); poll_transmit() again
//! on_timeout()     => session.handle_timeout();          poll_transmit() again
//! ```
//!
//! Only one request is in flight at a time, matching the official app.

use std::collections::VecDeque;

use crate::frame::{Decoder, FrameError};
use crate::model::{self, KeyboardState, ModelError};
use crate::protocol::{Message, Request, Response};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    Applied { request: Request, response: Response },
    Rejected { request: Request, status: u8 },
    /// A response arrived while nothing (or something else) was in flight.
    Stray(Response),
    Unsolicited(Vec<u8>),
    FrameError(FrameError),
}

#[derive(Debug, Default)]
pub struct Session {
    state: KeyboardState,
    decoder: Decoder,
    queue: VecDeque<Request>,
    in_flight: Option<Request>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> &KeyboardState {
        &self.state
    }

    /// State as it will be once every queued write has succeeded. Used to
    /// build read-modify-write requests while earlier writes are pending.
    pub fn projected_state(&self) -> KeyboardState {
        let mut s = self.state.clone();
        for r in self.in_flight.iter().chain(&self.queue) {
            if r.is_write() {
                s.apply(r, &Response { opcode: r.opcode(), status: 0, data: Vec::new() });
            }
        }
        s
    }

    pub fn enqueue(&mut self, requests: impl IntoIterator<Item = Request>) {
        self.queue.extend(requests);
    }

    pub fn enqueue_with<F>(&mut self, build: F) -> Result<usize, ModelError>
    where
        F: FnOnce(&KeyboardState) -> Result<Vec<Request>, ModelError>,
    {
        let reqs = build(&self.projected_state())?;
        let n = reqs.len();
        self.queue.extend(reqs);
        Ok(n)
    }

    pub fn enqueue_sync(&mut self) {
        self.enqueue(model::sync_plan());
    }

    pub fn in_flight(&self) -> Option<&Request> {
        self.in_flight.as_ref()
    }

    pub fn pending(&self) -> usize {
        self.queue.len() + self.in_flight.is_some() as usize
    }

    /// Next frame to write, if the channel is idle.
    pub fn poll_transmit(&mut self) -> Option<Vec<u8>> {
        if self.in_flight.is_some() {
            return None;
        }
        while let Some(req) = self.queue.pop_front() {
            match req.encode() {
                Ok(frame) => {
                    self.in_flight = Some(req);
                    return Some(frame);
                }
                // Only an oversized `Raw` payload can fail to encode; drop it.
                Err(_) => continue,
            }
        }
        None
    }

    pub fn handle_notification(&mut self, bytes: &[u8]) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        for frame in self.decoder.push(bytes) {
            let payload = match frame {
                Ok(p) => p,
                Err(e) => {
                    events.push(SessionEvent::FrameError(e));
                    continue;
                }
            };
            let response = match Message::parse(&payload) {
                Message::Response(r) => r,
                Message::Other(raw) => {
                    events.push(SessionEvent::Unsolicited(raw));
                    continue;
                }
            };
            match self.in_flight.take() {
                Some(request) if request.matches(&response) => {
                    if response.is_ok() {
                        self.state.apply(&request, &response);
                        events.push(SessionEvent::Applied { request, response });
                    } else {
                        events.push(SessionEvent::Rejected { request, status: response.status });
                    }
                }
                other => {
                    self.in_flight = other;
                    events.push(SessionEvent::Stray(response));
                }
            }
        }
        events
    }

    /// Gives up on the in-flight request so the queue can continue.
    pub fn handle_timeout(&mut self) -> Option<Request> {
        self.decoder.reset();
        self.in_flight.take()
    }

    /// Drops queued requests (not the one in flight), e.g. after a rejection.
    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }

    /// Call after a disconnect. Drops queued work and cached state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame;

    #[test]
    fn drives_one_request_at_a_time() {
        let mut s = Session::new();
        s.enqueue([Request::ReadRegister { addr: 8, len: 1 }, Request::ReadRegister { addr: 6, len: 2 }]);
        assert_eq!(s.poll_transmit(), Some(vec![0xff, 0x04, 0x25, 0x08, 0x01, 0x11]));
        assert_eq!(s.poll_transmit(), None);
        let ev = s.handle_notification(&[0xff, 0x05, 0x02, 0x25, 0x00, 0x05, 0x73]);
        assert!(matches!(ev[..], [SessionEvent::Applied { .. }]));
        assert_eq!(s.state().byte(8), Some(5));
        assert!(s.poll_transmit().is_some());
        assert_eq!(s.pending(), 1);
        assert!(s.handle_timeout().is_some());
        assert_eq!(s.pending(), 0);
    }

    #[test]
    fn clear_queue_keeps_the_request_in_flight() {
        let mut s = Session::new();
        s.enqueue_sync();
        assert!(s.poll_transmit().is_some());
        s.clear_queue();
        assert_eq!(s.pending(), 1);
        s.handle_notification(&[0xff, 0x05, 0x02, 0x25, 0x00, 0x05, 0x73]);
        assert_eq!((s.pending(), s.poll_transmit()), (0, None));
    }

    #[test]
    fn read_modify_write_sees_queued_writes() {
        let mut s = Session::new();
        s.enqueue([Request::ReadRegister { addr: 8, len: 1 }]);
        s.poll_transmit();
        s.handle_notification(&frame::encode(&[0x02, 0x25, 0x00, 0x05]).unwrap());
        s.enqueue_with(|st| st.set_flag(4, true)).unwrap();
        s.enqueue_with(|st| st.set_flag(1, true)).unwrap();
        assert_eq!(s.poll_transmit().unwrap()[4], 0x15);
        assert_eq!(s.projected_state().byte(8), Some(0x17));
    }
}
