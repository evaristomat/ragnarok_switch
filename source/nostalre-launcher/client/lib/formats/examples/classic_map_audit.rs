//! Export exact client resources for comparing deployed terrain with classic server cells.
use std::path::Path;
use nostalre_formats::{grf::GrfArchive, gat::GatFile};
use std::io::Write;
use flate2::{write::ZlibEncoder, Compression};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let root = &args[1];
    let output = Path::new(&args[2]);
    std::fs::create_dir_all(output).unwrap();
    let paths = ["ragswitch-assets.grf", "data.grf", "official_data.grf"].map(|name| format!("{root}/{name}"));
    let grf = GrfArchive::open_layered(&paths, None).unwrap();
    let mut cache = vec![0u8; 8];
    let mut count = 0u16;
    let files=grf.layered_file_list();
    for name in files.iter().map(|entry|entry.name.as_str()).filter(|name| name.starts_with("data/") && name.ends_with(".gat")) {
        let short = &name[5..name.len()-4];
        if short.len()>11 || short.contains('/') { continue; }
        let data=grf.read_file(name).unwrap();
        let gat=GatFile::parse(&data).unwrap();
        let cells:Vec<_>=(0..gat.cells.len()).map(|i| data[14+20*i+16]).collect();
        let mut encoder=ZlibEncoder::new(Vec::new(),Compression::fast());
        encoder.write_all(&cells).unwrap();let packed=encoder.finish().unwrap();
        let mut header=[0u8;12];header[..short.len()].copy_from_slice(short.as_bytes());
        cache.extend_from_slice(&header);
        cache.extend_from_slice(&(gat.width as i16).to_le_bytes());
        cache.extend_from_slice(&(gat.height as i16).to_le_bytes());
        cache.extend_from_slice(&(packed.len() as i32).to_le_bytes());
        cache.extend_from_slice(&packed);count+=1;
    }
    let size=cache.len() as u32;cache[..4].copy_from_slice(&size.to_le_bytes());cache[4..6].copy_from_slice(&count.to_le_bytes());
    std::fs::write(output.join("all-client-cache.dat"),cache).unwrap();
    println!("ALL_CLIENT_TERRAIN_EXPORTED {count} maps");
    for name in grf.entry_names().filter(|name| name.ends_with(".gat") && (name.contains("izlude") || name.contains("prontera"))) {
        let data = grf.read_file(name).unwrap();
        let gat = GatFile::parse(&data).unwrap();
        println!("MAP_VARIANT {name} {}x{}",gat.width,gat.height);
    }
    for map in &args[3..] {
        let data = grf.read_file(&format!("data/{map}.gat")).unwrap();
        let gat = GatFile::parse(&data).unwrap();
        std::fs::write(output.join(format!("{map}.gat")), &data).unwrap();
        println!("CLIENT_MAP {map} {}x{}", gat.width, gat.height);
    }
}
