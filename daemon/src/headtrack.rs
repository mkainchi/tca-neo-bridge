//! OpenTrack "UDP over network" input: datagrams of six little-endian f64 (x, y, z, yaw, pitch, roll), received on
//! 127.0.0.1 only, so nothing else on the network can move the camera.
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Head yaw and pitch in degrees, or None for anything that is not a well-formed OpenTrack datagram.
pub fn parse(datagram: &[u8]) -> Option<[f64; 2]> {
    if datagram.len() != 48 {
        return None;
    }
    let value = |index: usize| f64::from_le_bytes(datagram[8 * index..8 * index + 8].try_into().unwrap());
    let sample = [value(3), value(4)];
    sample.iter().all(|v| v.is_finite()).then_some(sample)
}

type Latest = Arc<Mutex<Option<([f64; 2], Instant)>>>;

pub struct Listener {
    latest: Latest,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    pub fn start(port: u16) -> Result<Self, String> {
        let socket = UdpSocket::bind(("127.0.0.1", port)).map_err(|_| format!("Head tracking could not listen on UDP port {port}; another program may be using it."))?;
        socket.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        let (latest, stop): (Latest, _) = (Arc::new(Mutex::new(None)), Arc::new(AtomicBool::new(false)));
        let (thread_latest, thread_stop) = (latest.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            let mut buffer = [0u8; 64];
            while !thread_stop.load(Ordering::SeqCst) {
                if let Some(sample) = socket.recv(&mut buffer).ok().and_then(|length| parse(&buffer[..length])) {
                    *thread_latest.lock().unwrap() = Some((sample, Instant::now()));
                }
            }
        });
        Ok(Self { latest, stop, thread: Some(thread) })
    }

    /// The latest sample if it arrived within `timeout`.
    pub fn sample(&self, timeout: Duration) -> Option<[f64; 2]> {
        self.latest.lock().unwrap().filter(|(_, at)| at.elapsed() <= timeout).map(|(sample, _)| sample)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}
