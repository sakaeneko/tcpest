use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use memflow::prelude::v1::*;

const LISTEN_ADDR: &str = "0.0.0.0:8888";

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

fn handle<M: PhysicalMemory>(s: &mut TcpStream, mem: &mut M) {
    let mut hdr = [0u8; 20];
    loop {
        if s.read_exact(&mut hdr).is_err() {
            return;
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
                let addr: PhysicalAddress = Address::from(req.addr).into();
                if mem.phys_read_into(addr, data.as_mut_slice()).is_err() {
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
                    return;
                }
                let addr: PhysicalAddress = Address::from(req.addr).into();
                let ok = mem.phys_write(addr, data.as_slice()).is_ok();
                let _ = s.write_all(&Packet {
                    cmd: 2,
                    addr: req.addr,
                    cb: if ok { size as u64 } else { 0 },
                }.to_bytes());
            }
            _ => return,
        }
    }
}

fn main() {
    let mut inventory = Inventory::scan();

    let mut connector = inventory
        .create_connector("kvm", None, None)
        .expect("failed to create kvm connector");

    let listener = TcpListener::bind(LISTEN_ADDR).expect("bind failed");
    println!("rawtcp listening on {}", LISTEN_ADDR);

    for stream in listener.incoming() {
        if let Ok(mut s) = stream {
            println!("client connected");
            handle(&mut s, &mut connector);
            println!("client disconnected");
        }
    }
}
