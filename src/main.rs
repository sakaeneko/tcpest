rust
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

const LISTEN_ADDR: &str = "0.0.0.0:8888";

// ============================================================
// 【改动点 1】把你的 memflow 读内存逻辑填进这里
// 返回值：Ok(()) 表示读取成功，buf 已被填满
//        Err(...) 表示读取失败
// ============================================================
fn read_phys(addr: u64, buf: &mut [u8]) -> Result<(), String> {
    // 示例：把你已经实现好的读内存函数填这里
    // 例如：
    //     my_memflow_reader.read_raw_into(addr, buf)
    //
    // 如果读成功，返回 Ok(())
    // 如果读失败，返回 Err("...".to_string())

    let _ = addr;
    let _ = buf;
    Err("read_phys 还没实现".to_string())
}

// ============================================================
// 【改动点 2】如果你的 memflow 支持写内存，填这里
// 不支持就直接返回 Err 即可
// ============================================================
fn write_phys(_addr: u64, _buf: &[u8]) -> Result<(), String> {
    Err("write_phys 还没实现".to_string())
}

// ---------------- 以下是协议实现，不用动 ----------------

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Packet {
    cmd: u32,
    addr: u64,
    cb: u64,
}

impl Packet {
    fn to_bytes(&self) -> [u8; 20] {
        let mut b = [0u8; 20];
        b[0..4].copy_from_slice(&self.cmd.to_le_bytes());
        b[4..12].copy_from_slice(&self.addr.to_le_bytes());
        b[12..20].copy_from_slice(&self.cb.to_le_bytes());
        b
    }
    fn from_bytes(b: &[u8; 20]) -> Self {
        Self {
            cmd: u32::from_le_bytes(b[0..4].try_into().unwrap()),
            addr: u64::from_le_bytes(b[4..12].try_into().unwrap()),
            cb: u64::from_le_bytes(b[12..20].try_into().unwrap()),
        }
    }
}

fn handle(mut s: TcpStream) {
    let mut hdr = [0u8; 20];
    loop {
        if s.read_exact(&mut hdr).is_err() {
            break;
        }
        let req = Packet::from_bytes(&hdr);

        match req.cmd {
            0 => {
                // STATUS
                let _ = s.write_all(&Packet { cmd: 0, addr: 0, cb: 1 }.to_bytes());
                let _ = s.write_all(&[1u8]);
            }
            1 => {
                // MEM_READ
                let size = req.cb as usize;
                let mut data = vec![0u8; size];
                if read_phys(req.addr, &mut data).is_err() {
                    let _ = s.write_all(&Packet { cmd: 1, addr: req.addr, cb: 0 }.to_bytes());
                    continue;
                }
                let _ = s.write_all(&Packet { cmd: 1, addr: req.addr, cb: size as u64 }.to_bytes());
                let _ = s.write_all(&data);
            }
            2 => {
                // MEM_WRITE
                let size = req.cb as usize;
                let mut data = vec![0u8; size];
                if s.read_exact(&mut data).is_err() {
                    break;
                }
                let ok = write_phys(req.addr, &data).is_ok();
                let _ = s.write_all(&Packet {
                    cmd: 2,
                    addr: req.addr,
                    cb: if ok { size as u64 } else { 0 },
                }.to_bytes());
            }
            _ => break,
        }
    }
}

fn main() {
    let listener = TcpListener::bind(LISTEN_ADDR).expect("bind failed");
    println!("rawtcp listening on {}", LISTEN_ADDR);
    for stream in listener.incoming() {
        if let Ok(s) = stream {
            thread::spawn(move || handle(s));
        }
    }
}
