//! Frames per action of the ACT files named on stdin (one per line, as kRO's effects ask the resource
//! manager), read from the rig's GRFs in its DATA.INI order (rdata.grf, data.grf, then its data folder). Prints JSON
//! {name: {"path": found, "frames": [per action]}}; names not found map to null.
use nostalre_formats::act::ActFile;
use nostalre_formats::grf::GrfArchive;
use std::io::BufRead;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = std::env::args()
        .nth(1)
        .ok_or("usage: act_frames <rig client dir> < names")?;
    let grfs = vec![format!("{client}/rdata.grf"), format!("{client}/data.grf")];
    let archive = GrfArchive::open_layered(&grfs, Some(&Path::new(&client).join("data")))?;
    let mut out = Vec::new();
    for line in std::io::stdin().lock().lines() {
        let name = line?;
        let candidates = [
            format!("data\\sprite\\{name}"),
            format!("data\\{name}"),
            name.clone(),
        ];
        let found = candidates
            .iter()
            .find_map(|c| archive.read_file(c).ok().map(|d| (c.clone(), d)));
        let entry = match found {
            Some((path, data)) => match ActFile::parse(&data) {
                Ok(act) => {
                    let frames: Vec<usize> = act.actions.iter().map(|a| a.motions.len()).collect();
                    format!("{{\"path\": {:?}, \"frames\": {:?}}}", path, frames)
                }
                Err(e) => format!("{{\"path\": {:?}, \"error\": {:?}}}", path, e.to_string()),
            },
            None => "null".to_string(),
        };
        out.push(format!("{:?}: {}", name, entry));
    }
    println!("{{{}}}", out.join(",\n"));
    Ok(())
}
