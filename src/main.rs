fn timestamp() -> [u8; 4] {
    (std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32)
        .to_be_bytes()
}

fn validate_and_get_len(msg: &[u8]) -> Option<usize> {
    if msg[0] != 0x21 || msg[1] != 0x31 {
        eprintln!(" bad magic {:#x} {:#x}", msg[0], msg[1]);
        None
    } else {
        Some(u16::from_be_bytes([msg[2], msg[3]]) as usize)
    }
}

fn process(proto: &'static str, msg: &[u8], resp: &mut [u8]) -> usize {
    let did = u32::from_be_bytes([msg[8], msg[9], msg[10], msg[11]]);
    if &msg[4..12] == [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff] {
        eprintln!(" {} client hello", proto);
        resp[..32].copy_from_slice(msg);
        resp[12..16].copy_from_slice(&timestamp()[..]);
        return 32;
    }
    if msg.len() == 32 {
        eprintln!(" {} {:#x} ping", proto, did);
        resp[..32].copy_from_slice(msg);
        return 32;
    }
    eprintln!(" {} {:#x} something real, ignoring", proto, did);
    return 0;
}

fn serve_udp() -> () {
    let socket = std::net::UdpSocket::bind("0.0.0.0:8053").unwrap();

    loop {
        let mut inbuf = [0; 512];
        let mut outbuf = [0; 256];
        let (len, src) = match socket.recv_from(&mut inbuf) {
            Ok(x) => x,
            Err(x) => {
                eprintln!(" could not recv: {:?}", x);
                continue;
            }
        };
        // eprintln!("UDP< {:?} {:?} {:x?}", len, src, &inbuf[..len]);
        if let Some(field_len) = validate_and_get_len(&inbuf[..len]) {
            if field_len != len {
                eprintln!(" bad length {} vs actual {}", field_len, len);
                continue;
            }
            let outlen = process("UDP", &inbuf[..len], &mut outbuf);
            if outlen > 0 {
                // eprintln!("UDP> {:?} {:?} {:x?}", outlen, src, &outbuf[..outlen]);
                if let Err(x) = socket.send_to(&outbuf[..outlen], &src) {
                    eprintln!(" could not send: {:?}", x);
                }
            }
        }
    }
}

fn serve_tcp() -> () {
    let listener = std::net::TcpListener::bind("0.0.0.0:8053").unwrap();

    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            std::thread::spawn(move || loop {
                use std::io::{Read, Write};
                let mut inbuf = [0; 1024];
                let mut outbuf = [0; 256];
                if let Err(x) = stream.read_exact(&mut inbuf[..32]) {
                    eprintln!(
                        " could not read (initial): {:?}, probably just a disconnection",
                        x
                    );
                    return;
                }
                if let Some(field_len) = validate_and_get_len(&inbuf[..32]) {
                    if field_len < 32 {
                        eprintln!(" bad length {} < 32", field_len);
                        continue;
                    }
                    if field_len > 32 {
                        if let Err(x) = stream.read_exact(&mut inbuf[32..field_len]) {
                            eprintln!(" could not read (continuation): {:?}", x);
                            return;
                        }
                    }
                    // eprintln!("TCP< {:?} {:x?}", field_len, &inbuf[..field_len]);
                    let outlen = process("TCP", &inbuf[..field_len], &mut outbuf);
                    if outlen > 0 {
                        // eprintln!("TCP> {:?} {:x?}", outlen, &outbuf[..outlen]);
                        stream.write(&outbuf[..outlen]).unwrap();
                    }
                } else {
                    eprintln!(" not a valid message??");
                }
            });
        }
    }
}

fn main() -> () {
    std::thread::spawn(serve_tcp);
    serve_udp()
}
