use clap::{App, Arg};
use buffett::netutil::bind_to;
use buffett::packet::{Packet, SharedPackets, BLOB_SIZE, PACKET_DATA_SIZE};
use buffett::result::Result;
use buffett::streamer::{receiver, PacketReceiver};
use std::cmp::max;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread::sleep;
use std::thread::{spawn, JoinHandle};
use std::time::Duration;
use std::time::SystemTime;
use std::ffi::CStr;
use crate::rustelo_error::RusteloResult;

fn producer(addr: &SocketAddr, exit: Arc<AtomicBool>) -> JoinHandle<()> {
    let send = UdpSocket::bind("0.0.0.0:0").unwrap();
    let msgs = SharedPackets::default();
    let msgs_ = msgs.clone();
    msgs.write().unwrap().packets.resize(10, Packet::default());
    for w in &mut msgs.write().unwrap().packets {
        w.meta.size = PACKET_DATA_SIZE;
        w.meta.set_addr(&addr);
    }
    spawn(move || loop {
        if exit.load(Ordering::Relaxed) {
            return;
        }
        let mut num = 0;
        for p in &msgs_.read().unwrap().packets {
            let a = p.meta.addr();
            assert!(p.meta.size < BLOB_SIZE);
            send.send_to(&p.data[..p.meta.size], &a).unwrap();
            num += 1;
        }
        assert_eq!(num, 10);
    })
}

fn sink(exit: Arc<AtomicBool>, rvs: Arc<AtomicUsize>, r: PacketReceiver) -> JoinHandle<()> {
    spawn(move || loop {
        if exit.load(Ordering::Relaxed) {
            return;
        }
        let timer = Duration::new(1, 0);
        if let Ok(msgs) = r.recv_timeout(timer) {
            rvs.fetch_add(msgs.read().unwrap().packets.len(), Ordering::Relaxed);
        }
    })
}



/// to do : rewrite benchcster 
#[no_mangle]
pub extern "C" fn benchcaster_main_entry(parm01_num_recv_sockets_ptr: *const libc::c_char) -> RusteloResult  {
/*
#[no_mangle]
  pub extern "C" fn benchcaster_main_entry(parm01_num_recv_sockets_ptr: *const libc::c_char) -> Result<()>  {
*/
    let mut num_sockets = 1usize;

    //handle parameters, convert ptr to &str
    let num_recv_sockets_str = unsafe { CStr::from_ptr(parm01_num_recv_sockets_ptr) }.to_str().unwrap(); 
    /*
    let matches = App::new("bitconch-bench-caster")
        .arg(
            Arg::with_name("num-recv-sockets")
                .long("num-recv-sockets")
                .value_name("NUM")
                .takes_value(true)
                .help("Use NUM receive sockets"),
        ).get_matches();
    */

    /*
    if let Some(n) = matches.value_of("num-recv-sockets") {
        num_sockets = max(num_sockets, n.to_string().parse().expect("integer"));
    }
    */
    if let Some(n) = Some(num_recv_sockets_str) {
        num_sockets = max(num_sockets, n.to_string().parse().expect("integer"));
    }

    let mut port = 0;
    let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);

    let exit = Arc::new(AtomicBool::new(false));

    let mut read_channels = Vec::new();
    let mut read_threads = Vec::new();
    for _ in 0..num_sockets {
        let read = bind_to(port, false).unwrap();
        read.set_read_timeout(Some(Duration::new(1, 0))).unwrap();

        addr = read.local_addr().unwrap();
        port = addr.port();

        let (s_reader, r_reader) = channel();
        read_channels.push(r_reader);
        read_threads.push(receiver(
            Arc::new(read),
            exit.clone(),
            s_reader,
            "bench-streamer",
        ));
    }

    let t_producer1 = producer(&addr, exit.clone());
    let t_producer2 = producer(&addr, exit.clone());
    let t_producer3 = producer(&addr, exit.clone());

    let rvs = Arc::new(AtomicUsize::new(0));
    let sink_threads: Vec<_> = read_channels
        .into_iter()
        .map(|r_reader| sink(exit.clone(), rvs.clone(), r_reader))
        .collect();
    let start = SystemTime::now();
    let start_val = rvs.load(Ordering::Relaxed);
    sleep(Duration::new(5, 0));
    let elapsed = start.elapsed().unwrap();
    let end_val = rvs.load(Ordering::Relaxed);
    let time = elapsed.as_secs() * 10_000_000_000 + u64::from(elapsed.subsec_nanos());
    let ftime = (time as f64) / 10_000_000_000_f64;
    let fcount = (end_val - start_val) as f64;
    println!("performance: {:?}", fcount / ftime);
    exit.store(true, Ordering::Relaxed);
    for t_reader in read_threads {
        //t_reader.join()?;
        tryffi!(t_reader.join());
    }
    /*
    t_producer1.join()?;
    t_producer2.join()?;
    t_producer3.join()?;
    */
    tryffi!(t_producer1.join());
    tryffi!(t_producer2.join());
    tryffi!(t_producer3.join());
    
    for t_sink in sink_threads {
        //t_sink.join()?;
        tryffi!(t_sink.join());
    }
    //Ok(())
    RusteloResult::Success
}
