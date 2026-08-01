use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Maximum number of simultaneous TCP client handlers.
const MAX_TCP_HANDLERS: usize = 32;

/// How long a TCP connection may stay idle before we drop it.
const TCP_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Back-off when a socket operation fails (prevents busy-loops).
const ERROR_BACKOFF: Duration = Duration::from_millis(50);

fn timestamp() -> [u8; 4] {
    (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32)
        .to_be_bytes()
}

fn validate_and_get_len(msg: &[u8]) -> Option<usize> {
    if msg.len() < 4 || msg[0] != 0x21 || msg[1] != 0x31 {
        if msg.len() >= 2 {
            eprintln!(" bad magic {:#x} {:#x}", msg[0], msg[1]);
        }
        None
    } else {
        Some(u16::from_be_bytes([msg[2], msg[3]]) as usize)
    }
}

fn process(proto: &'static str, msg: &[u8], resp: &mut [u8]) -> usize {
    if msg.len() < 12 {
        return 0;
    }
    let did = u32::from_be_bytes([msg[8], msg[9], msg[10], msg[11]]);

    // Client hello (DID field all 0xff)
    if msg.len() >= 12 && &msg[4..12] == [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff] {
        eprintln!(" {} client hello", proto);
        let n = 32.min(msg.len());
        resp[..n].copy_from_slice(&msg[..n]);
        if n >= 16 {
            resp[12..16].copy_from_slice(&timestamp());
        }
        return 32;
    }

    // Short ping / keep-alive
    if msg.len() == 32 {
        eprintln!(" {} {:#x} ping", proto, did);
        resp[..32].copy_from_slice(msg);
        return 32;
    }

    // Everything else is currently ignored (no cloud key available)
    eprintln!(" {} {:#x} something real (len={}), ignoring", proto, did, msg.len());
    0
}

fn serve_udp() {
    let socket = match UdpSocket::bind("0.0.0.0:8053") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("UDP bind failed: {:?}", e);
            return;
        }
    };
    // Explicitly blocking – protects against accidental non-blocking mode
    let _ = socket.set_nonblocking(false);

    loop {
        let mut inbuf = [0u8; 512];
        let mut outbuf = [0u8; 256];

        let (len, src) = match socket.recv_from(&mut inbuf) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("UDP recv error: {:?}", e);
                thread::sleep(ERROR_BACKOFF);
                continue;
            }
        };

        if let Some(field_len) = validate_and_get_len(&inbuf[..len]) {
            if field_len != len {
                eprintln!("UDP bad length {} vs actual {}", field_len, len);
                continue;
            }
            let outlen = process("UDP", &inbuf[..len], &mut outbuf);
            if outlen > 0 {
                if let Err(e) = socket.send_to(&outbuf[..outlen], src) {
                    eprintln!("UDP send error: {:?}", e);
                }
            }
        }
    }
}

fn handle_tcp_client(mut stream: TcpStream, active: Arc<AtomicUsize>) {
    // Ensure the counter is decremented when the handler exits
    struct Guard(Arc<AtomicUsize>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    let _guard = Guard(active);

    if let Err(e) = stream.set_read_timeout(Some(TCP_READ_TIMEOUT)) {
        eprintln!("TCP set_read_timeout failed: {:?}", e);
        return;
    }
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    loop {
        let mut inbuf = [0u8; 1024];
        let mut outbuf = [0u8; 256];

        // Read header
        if let Err(e) = stream.read_exact(&mut inbuf[..32]) {
            // Timeout or disconnect – just leave
            let kind = e.kind();
            if kind != std::io::ErrorKind::UnexpectedEof
                && kind != std::io::ErrorKind::WouldBlock
                && kind != std::io::ErrorKind::TimedOut
            {
                eprintln!("TCP read (header) error: {:?}", e);
            }
            return;
        }

        let Some(field_len) = validate_and_get_len(&inbuf[..32]) else {
            eprintln!("TCP not a valid message");
            return;
        };

        if field_len < 32 {
            eprintln!("TCP bad length {} < 32", field_len);
            return;
        }
        if field_len > inbuf.len() {
            eprintln!("TCP packet too large: {}", field_len);
            return;
        }

        if field_len > 32 {
            if let Err(e) = stream.read_exact(&mut inbuf[32..field_len]) {
                eprintln!("TCP read (body) error: {:?}", e);
                return;
            }
        }

        let outlen = process("TCP", &inbuf[..field_len], &mut outbuf);
        if outlen > 0 {
            if let Err(e) = stream.write_all(&outbuf[..outlen]) {
                eprintln!("TCP write error: {:?}", e);
                return;
            }
        }
    }
}

fn serve_tcp() {
    let listener = match TcpListener::bind("0.0.0.0:8053") {
        Ok(l) => l,
        Err(e) => {
            eprintln!("TCP bind failed: {:?}", e);
            return;
        }
    };
    let _ = listener.set_nonblocking(false);

    let active = Arc::new(AtomicUsize::new(0));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let current = active.load(Ordering::SeqCst);
                if current >= MAX_TCP_HANDLERS {
                    eprintln!(
                        "TCP: too many concurrent connections ({}), dropping",
                        current
                    );
                    // Drop the stream – connection is closed
                    continue;
                }
                active.fetch_add(1, Ordering::SeqCst);
                let active = Arc::clone(&active);
                thread::spawn(move || handle_tcp_client(stream, active));
            }
            Err(e) => {
                eprintln!("TCP accept error: {:?}", e);
                thread::sleep(ERROR_BACKOFF);
            }
        }
    }
}

fn main() {
    eprintln!("miio-fakecloud starting (UDP+TCP :8053)");
    thread::spawn(serve_tcp);
    serve_udp();
}
