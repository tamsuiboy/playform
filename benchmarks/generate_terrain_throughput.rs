//! Benchmarks for throughput of terrain generation.

#![deny(missing_docs)]
#![deny(warnings)]

extern crate common;
extern crate client_lib;
extern crate server_lib;

extern crate cgmath;
extern crate collision;

extern crate env_logger;
#[macro_use]
extern crate log;
extern crate time;

mod generate_terrain;

fn main() {
  env_logger::init().unwrap();

  println!("Starting..");
  let start = time::precise_time_ns();

  generate_terrain::generate_all_terrain();

  let now = time::precise_time_ns();
  println!("Completed in {:.1}s", ((now-start) as f32)/1e9);
}
