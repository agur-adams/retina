use retina_core::config::load_config;
use retina_core::subscription::{Connection, connection::Flow};
use retina_core::dpdk::{rte_get_tsc_hz, rte_rdtsc};
use retina_core::Runtime;
use retina_filtergen::filter;

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration}; 

use anyhow::Result;
use clap::Parser;
use serde::{Serialize};
use chrono::Utc;

// Define command-line arguments.
#[derive(Parser, Debug)]
struct Args {
    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    config: PathBuf,
    #[clap(
        short,
        long,
        parse(from_os_str),
        value_name = "FILE",
        default_value = "conn.jsonl"
    )]
    outfile: PathBuf,
}

// Holds the desired collection data from a connection record.
#[derive(Debug, Serialize)]
struct ConnRecord {
    //proto: usize,
    ts_utc: i64,
    //ts_tsc: u64,
    //ts_sec: u64,
    duration: Duration,
    //max_inactivity: Duration,
    //time_to_second_packet: Duration,
    history: String,
    orig_nb_pkts: u64,
    orig_nb_bytes: u64,
    resp_nb_pkts: u64,
    resp_nb_bytes: u64,
}

// Holds the start time information for the Retina runtime.
#[derive(Debug, Serialize)]
struct StartTime {
    start_utc: i64,
    start_tsc: u64,
    start_sec: u64,
}

#[filter("tls")]
fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    let config = load_config(&args.config);

    // Use `BufWriter` to improve the speed of repeated write calls to the same file.
    let file = Mutex::new(BufWriter::new(File::create(&args.outfile)?));
    let cnt = AtomicUsize::new(0);

    let callback = |conn: Connection| {
        let record = ConnRecord { 
                    //proto: conn.five_tuple.proto, 
                    ts_utc: conn.ts_utc,
                    //ts_tsc: conn.ts_tsc,
                    //ts_sec: conn.ts_sec,
                    duration: conn.duration, 
                    //max_inactivity: conn.max_inactivity, 
                    //time_to_second_packet: conn.time_to_second_packet, 
                    history: conn.history(), 
                    orig_nb_pkts: conn.orig.nb_pkts,
                    orig_nb_bytes: conn.orig.nb_bytes,
                    resp_nb_pkts: conn.resp.nb_pkts,
                    resp_nb_bytes: conn.resp.nb_bytes,
        };
        if let Ok(serialized) = serde_json::to_string(&record) {
            let mut wtr = file.lock().unwrap();
            wtr.write_all(serialized.as_bytes()).unwrap();
            wtr.write_all(b"\n").unwrap();
            cnt.fetch_add(1, Ordering::Relaxed);
        }
    };

    // Configure the Retina runtime.
    let mut runtime = Runtime::new(config, filter, callback)?;
    
    // Capture the start time when the Retina runtime begins.
    let start_time = StartTime {
        start_utc: Utc::now().timestamp(), 
        start_tsc: unsafe { rte_rdtsc() },
        start_sec: (unsafe { rte_rdtsc() } as f64 / unsafe { rte_get_tsc_hz() as f64 } * 1e9) as u64,
    };
    if let Ok(start_serialized) = serde_json::to_string(&start_time) {
        let mut wtr = file.lock().unwrap();
        wtr.write_all(start_serialized.as_bytes()).unwrap();
        wtr.write_all(b"\n").unwrap();
    };    

    // Launch the Retina runtime.
    runtime.run();
    
    // Writes the buffered input to file and closes the application.
    let mut wtr = file.lock().unwrap();
    wtr.flush()?;
    println!("Done. Logged {:?} connections to {:?}", cnt, &args.outfile);
    Ok(())
}
