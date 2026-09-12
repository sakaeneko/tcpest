use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use memflow::prelude::v1::*;

const LISTEN_ADDR: &str = "0.0.0.0:8888";

fn init_memflow() -> ConnectorInstance {
    let inventory = Inventory::scan();

    let args = ConnectorArgs::new().insert("vm_id", "win10");

    inventory
        .create_connector("kvm", &args)
        .expect("failed to create kvm connector")
}

fn read_phys(mem: &mut ConnectorInstance, addr: u64, buf: &mut [u8]) -> Result<(), String> {
    mem.phys_read_raw_into(Address::from(addr), buf)
        .map_err(|e| format!("{:?}", e))
}

fn write_phys(mem: &mut ConnectorInstance, addr: u64, buf: &[u8]) -> Result<(), String> {
    mem.phys_write_raw(Address::from(addr), buf)
        .map_err(|e| format!("{:?}", e))
}

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

fn handle(mut s: TcpStream, mem: Arc<Mutex<ConnectorInstance>>) {
    let mut hdr = [0u8; 20];
    loop {
        if s.read_exact(&mut hdr).is_err() {
            break;
        }
        let req = Packet::from_bytes(&hdr);

        match req.cmd {
            0 => {
                let _ = s.write_all(&Packet { cmd: 0, addr: 0, cb: 1 }.to_bytes());
                let _ = s.write_all(&[1u8]);
            }
            1 => {
                let size = req.cb as usize;
                let mut data = vec![0u8; size];
                let result = {
                    let mut m = mem.lock().unwrap();
                    read_phys(&mut *m, req.addr, &mut data)
                };
                if result.is_err() {
                    let _ = s.write_all(&Packet { cmd: 1, addr: req.addr, cb: 0 }.to_bytes());
                    continue;
                }
                let _ = s.write_all(&Packet { cmd: 1, addr: req.addr, cb: size as u64 }.to_bytes());
                let _ = s.write_all(&data);
            }
            2 => {
                let size = req.cb as usize;
                let mut data = vec![0u8; size];
                if s.read_exact(&mut data).is_err() {
                    break;
                }
                let ok = {
                    let mut m = mem.lock().unwrap();
                    write_phys(&mut *m, req.addr, &data).is_ok()
                };
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
    let mem = Arc::new(Mutex::new(init_memflow()));
    let listener = TcpListener::bind(LISTEN_ADDR).expect("bind failed");
    println!("rawtcp listening on {}", LISTEN_ADDR);

    for stream in listener.incoming() {
        if let Ok(s) = stream {
            let m = Arc::clone(&mem);
            thread::spawn(move || handle(s, m));
        }
    }
}
