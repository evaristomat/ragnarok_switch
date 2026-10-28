use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

use crate::FormatError;
use crate::mixcrypt;
use crate::res_name_table::ResNameTable;

#[path = "byte_cache.rs"]
mod byte_cache;
use byte_cache::ByteCache;

const READ_CACHE_BUDGET: usize = if cfg!(any(target_os = "horizon", test)) { 16 * 1024 * 1024 } else { 0 };
const HEADER_SIZE: usize = 46;
const FILE_OFFSET: usize = 7;
/// Version 0x300 archives (GRF Editor's "Event Horizon" format) carry 64-bit
/// data offsets, so they can grow past 4 GB.
const V3_MAGIC: &[u8; 14] = b"Event Horizon\0";

struct GrfEntry {
    compressed_size: u32,
    compressed_size_aligned: u32,
    uncompressed_size: u32,
    flags: u8,
    offset: u64,
}

pub struct GrfFileInfo {
    pub name: String,
    pub compressed_size: u32,
    pub compressed_size_aligned: u32,
    pub uncompressed_size: u32,
    pub flags: u8,
}

pub struct GrfArchive {
    entries: HashMap<String, GrfEntry>,
    file: Mutex<File>,
    path: PathBuf,
    file_data_end_offset: u64,
    writable: bool,
    data_dir: Option<DataDirIndex>,
    overlays: Vec<GrfArchive>,
    overrides: HashMap<String, Vec<u8>>,
    res_name_table: OnceLock<ResNameTable>,
    read_cache: Option<Mutex<ByteCache>>,
    #[cfg(test)]
    archive_reads: std::sync::atomic::AtomicUsize,
}

/// Extracted files on disk, keyed by their normalized path relative to the
/// directory. A resource name resolves against this by dropping its leading
/// `data/` component, so a directory holding `sprite/foo.spr` overrides the
/// archive entry `data/sprite/foo.spr`.
struct DataDirIndex {
    files: HashMap<String, PathBuf>,
}

impl DataDirIndex {
    fn build(dir: &Path) -> DataDirIndex {
        let mut files = HashMap::new();
        index_dir(dir, dir, &mut files);
        DataDirIndex { files }
    }

    fn lookup(&self, name: &str) -> Option<&PathBuf> {
        self.files.get(&data_dir_key(name))
    }
}

fn data_dir_key(name: &str) -> String {
    let normalized = name.to_lowercase().replace('\\', "/");
    normalized
        .strip_prefix(nostalre_resources::dir::DATA)
        .unwrap_or(&normalized)
        .to_string()
}

fn index_dir(root: &Path, dir: &Path, out: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            index_dir(root, &path, out);
        } else if let Ok(relative) = path.strip_prefix(root) {
            let key = relative.to_string_lossy().to_lowercase().replace('\\', "/");
            out.insert(key, path);
        }
    }
}

fn parse_grf(file: &mut File) -> Result<(HashMap<String, GrfEntry>, u64, u32), FormatError> {
    let mut header_buf = [0u8; HEADER_SIZE];
    file.read_exact(&mut header_buf)?;

    let version = u32::from_le_bytes(header_buf[42..46].try_into().unwrap());
    let magic_ok = if version == 0x300 {
        header_buf[..14] == *V3_MAGIC
    } else {
        &header_buf[..16] == b"Master of Magic\0"
    };
    if !magic_ok {
        return Err(FormatError::InvalidMagic);
    }
    if !(0x100..=0x200).contains(&version) && version != 0x300 {
        return Err(FormatError::UnsupportedVersion(
            (version >> 8) as u8,
            version as u8,
        ));
    }

    let file_count = u32::from_le_bytes(header_buf[38..42].try_into().unwrap());
    let (file_table_offset, actual_file_count) = if version == 0x300 {
        // The 64-bit offset takes the reserved-files slot, and the count is exact.
        let offset = u64::from_le_bytes(header_buf[30..38].try_into().unwrap());
        (offset, file_count as usize)
    } else {
        let offset = u32::from_le_bytes(header_buf[30..34].try_into().unwrap());
        let reserved_files = u32::from_le_bytes(header_buf[34..38].try_into().unwrap());
        (
            offset as u64,
            (file_count - reserved_files) as usize - FILE_OFFSET,
        )
    };
    let file_data_end_offset = HEADER_SIZE as u64 + file_table_offset;

    file.seek(SeekFrom::Start(file_data_end_offset))?;

    let entries = match version {
        0x300 => {
            // A zero word precedes the table's sizes.
            read_u32_le(file)?;
            parse_v2_entries(file, actual_file_count, OffsetWidth::U64)?
        }
        0x200 => parse_v2_entries(file, actual_file_count, OffsetWidth::U32)?,
        _ => parse_v1_entries(file, actual_file_count)?,
    };

    Ok((entries, file_data_end_offset, version))
}

impl GrfArchive {
    /// The bundled offline server uses the kRO map cache. Loose translation
    /// packs may include pre-Renewal geometry and must not replace those maps.
    /// Text, sprites and other loose resources retain their normal priority.
    pub fn use_archived_map_geometry(&mut self) -> usize {
        let Some(data_dir) = self.data_dir.as_mut() else {
            return 0;
        };
        let geometry = |name: &str| matches!(name.rsplit('.').next(), Some("gat" | "gnd" | "rsw"));
        let archived: HashSet<String> = self.entries.keys()
            .chain(self.overlays.iter().flat_map(|layer| layer.entries.keys()))
            .filter(|name| geometry(name))
            .map(|name| data_dir_key(name))
            .collect();
        let before = data_dir.files.len();
        data_dir.files.retain(|name, _| !geometry(name) || !archived.contains(name));
        before - data_dir.files.len()
    }

    pub fn open(path: &Path) -> Result<Self, FormatError> {
        let mut file = File::open(path).map_err(FormatError::Io)?;
        let (entries, file_data_end_offset, _) = parse_grf(&mut file)?;

        Ok(GrfArchive {
            entries,
            file: Mutex::new(file),
            path: path.to_path_buf(),
            file_data_end_offset,
            writable: false,
            read_cache: (READ_CACHE_BUDGET > 0).then(|| Mutex::new(ByteCache::new(READ_CACHE_BUDGET))),
            data_dir: None,
            overlays: Vec::new(),
            overrides: HashMap::new(),
            res_name_table: OnceLock::new(),
            #[cfg(test)]
            archive_reads: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    /// Open a priority-ordered stack of archives plus an optional directory of
    /// extracted files. `grf_paths[0]` is the primary archive; later paths only
    /// provide files the earlier ones lack. `data_dir`, when present, overrides
    /// every archive.
    pub fn open_layered(
        grf_paths: &[String],
        data_dir: Option<&Path>,
    ) -> Result<Self, FormatError> {
        let (first, rest) = grf_paths
            .split_first()
            .ok_or(FormatError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "grf_paths is empty",
            )))?;

        let mut archive = GrfArchive::open(Path::new(first))?;

        for path in rest {
            match GrfArchive::open(Path::new(path)) {
                Ok(overlay) => archive.overlays.push(overlay),
                Err(e) => tracing::error!("Failed to open overlay GRF {path}: {e}"),
            }
        }

        // The whole archive stack shares a fixed budget, regardless of layer count.
        let per_layer = READ_CACHE_BUDGET / (1 + archive.overlays.len());
        archive.read_cache = (per_layer > 0).then(|| Mutex::new(ByteCache::new(per_layer)));
        for overlay in &mut archive.overlays {
            overlay.read_cache = (per_layer > 0).then(|| Mutex::new(ByteCache::new(per_layer)));
        }

        if let Some(dir) = data_dir {
            if dir.is_dir() {
                archive.data_dir = Some(DataDirIndex::build(dir));
            } else {
                tracing::warn!("data_dir {} is not a directory, ignoring", dir.display());
            }
        }

        Ok(archive)
    }

    pub fn open_rw(path: &Path) -> Result<Self, FormatError> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(FormatError::Io)?;
        let (entries, file_data_end_offset, version) = parse_grf(&mut file)?;
        // `save` writes the 0x200 layout only.
        if version == 0x300 {
            return Err(FormatError::UnsupportedVersion(3, 0));
        }

        Ok(GrfArchive {
            entries,
            file: Mutex::new(file),
            path: path.to_path_buf(),
            file_data_end_offset,
            writable: true,
            read_cache: None,
            data_dir: None,
            overlays: Vec::new(),
            overrides: HashMap::new(),
            res_name_table: OnceLock::new(),
            #[cfg(test)]
            archive_reads: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    pub fn create(path: &Path) -> Result<Self, FormatError> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(FormatError::Io)?;

        let mut header = [0u8; HEADER_SIZE];
        header[..16].copy_from_slice(b"Master of Magic\0");
        header[30..34].copy_from_slice(&0u32.to_le_bytes());
        header[34..38].copy_from_slice(&0u32.to_le_bytes());
        header[38..42].copy_from_slice(&(FILE_OFFSET as u32).to_le_bytes());
        header[42..46].copy_from_slice(&0x200u32.to_le_bytes());
        file.write_all(&header)?;

        let empty_table = Vec::new();
        let compressed = zlib_compress(&empty_table)?;
        file.write_all(&(compressed.len() as u32).to_le_bytes())?;
        file.write_all(&0u32.to_le_bytes())?;
        file.write_all(&compressed)?;
        file.flush()?;

        Ok(GrfArchive {
            entries: HashMap::new(),
            file: Mutex::new(file),
            path: path.to_path_buf(),
            file_data_end_offset: HEADER_SIZE as u64,
            writable: true,
            read_cache: None,
            data_dir: None,
            overlays: Vec::new(),
            overrides: HashMap::new(),
            res_name_table: OnceLock::new(),
            #[cfg(test)]
            archive_reads: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    /// Reads `name`, falling back to the `resnametable.txt` alias when nothing
    /// carries that name. Maps such as `pvp_n_2-2` ship no geometry of their
    /// own and exist only as aliases.
    pub fn read_file(&self, name: &str) -> Result<Vec<u8>, FormatError> {
        match self.read_file_direct(name) {
            Ok(data) => Ok(data),
            Err(e) => {
                let Some(alias) = self.alias_table().resolve(name) else {
                    return Err(e);
                };
                tracing::debug!("Resource {name} redirected to {alias}");
                self.read_file_direct(&alias)
            }
        }
    }

    /// Every layer contributes its own table; the higher-priority layer wins a
    /// key both define.
    fn alias_table(&self) -> &ResNameTable {
        self.res_name_table.get_or_init(|| {
            let mut table = ResNameTable::default();
            let path = nostalre_resources::table::RES_NAME;
            if let Some(disk) = self.data_dir.as_ref().and_then(|d| d.lookup(path))
                && let Ok(data) = std::fs::read(disk)
            {
                table.extend_from(ResNameTable::parse(&data));
            }
            for layer in std::iter::once(self).chain(self.overlays.iter()) {
                if let Ok(data) = layer.read_archive_entry(path) {
                    table.extend_from(ResNameTable::parse(&data));
                }
            }
            tracing::info!("Loaded resource name table ({} entries)", table.len());
            table
        })
    }

    /// Replaces what `read_file` returns for `name`, ahead of `data_dir` and
    /// every layer. `grf-merge` installs the tables it merged across layers
    /// here so the reachability walk sees the merged rows.
    pub fn set_override(&mut self, name: &str, data: Vec<u8>) {
        self.overrides
            .insert(name.to_lowercase().replace('\\', "/"), data);
    }

    fn read_file_direct(&self, name: &str) -> Result<Vec<u8>, FormatError> {
        if !self.overrides.is_empty()
            && let Some(data) = self.overrides.get(&name.to_lowercase().replace('\\', "/"))
        {
            return Ok(data.clone());
        }

        if let Some(path) = self.data_dir.as_ref().and_then(|d| d.lookup(name)) {
            return std::fs::read(path).map_err(FormatError::Io);
        }

        let name_lower = name.to_lowercase().replace('\\', "/");
        if !self.entries.contains_key(&name_lower) {
            for overlay in &self.overlays {
                if overlay.entries.contains_key(&name_lower) {
                    return overlay.read_archive_entry(name);
                }
            }
        }
        self.read_archive_entry(name)
    }

    /// This archive's own entries, ignoring `data_dir` and overlays.
    fn read_archive_entry(&self, name: &str) -> Result<Vec<u8>, FormatError> {
        let name_lower = name.to_lowercase().replace('\\', "/");
        let Some(entry) = self.entries.get(&name_lower) else {
            return Err(FormatError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("file not found in archive: {name}"),
            )));
        };

        if let Some(cache) = &self.read_cache {
            if let Some(bytes) = cache.lock().unwrap().get(&name_lower) { return Ok(bytes); }
        }

        #[cfg(test)]
        self.archive_reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let position = entry.offset + HEADER_SIZE as u64;
        let mut compressed = vec![0u8; entry.compressed_size_aligned as usize];

        {
            let mut file = self.file.lock().unwrap();
            file.seek(SeekFrom::Start(position))?;
            file.read_exact(&mut compressed)?;
        }

        mixcrypt::decrypt_file(entry.flags, entry.compressed_size, &mut compressed);

        let mut decompressed = Vec::with_capacity(entry.uncompressed_size as usize);
        let mut decoder = ZlibDecoder::new(compressed.as_slice());
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| FormatError::DecompressionFailed(e.to_string()))?;

        // Only immutable archive data enters this cache. Loose files and overrides
        // are resolved before this function and remain live for translation edits.
        if let Some(cache) = &self.read_cache { cache.lock().unwrap().insert(name_lower, &decompressed); }
        Ok(decompressed)
    }

    pub fn add_file(&mut self, name: &str, data: &[u8]) -> Result<(), FormatError> {
        if !self.writable {
            return Err(FormatError::ReadOnly);
        }

        let name_lower = name.to_lowercase().replace('\\', "/");
        let compressed = zlib_compress(data)?;
        let compressed_len = compressed.len() as u32;

        let file = self.file.get_mut().unwrap();
        file.seek(SeekFrom::Start(self.file_data_end_offset))?;
        file.write_all(&compressed)?;

        let entry = GrfEntry {
            compressed_size: compressed_len,
            compressed_size_aligned: compressed_len,
            uncompressed_size: data.len() as u32,
            flags: 0x01,
            offset: self.file_data_end_offset - HEADER_SIZE as u64,
        };

        self.file_data_end_offset += compressed_len as u64;
        self.entries.insert(name_lower, entry);
        Ok(())
    }

    pub fn remove_file(&mut self, name: &str) -> Result<bool, FormatError> {
        if !self.writable {
            return Err(FormatError::ReadOnly);
        }
        let name_lower = name.to_lowercase().replace('\\', "/");
        Ok(self.entries.remove(&name_lower).is_some())
    }

    pub fn save(&mut self) -> Result<(), FormatError> {
        if !self.writable {
            return Err(FormatError::ReadOnly);
        }

        // The 0x200 layout this writes has 32-bit offsets
        if self.file_data_end_offset - HEADER_SIZE as u64 > u32::MAX as u64 {
            return Err(FormatError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "archive data past 4 GiB cannot be saved as GRF 0x200",
            )));
        }

        let table_raw = build_file_table(&self.entries);
        let table_compressed = zlib_compress(&table_raw)?;

        let file = self.file.get_mut().unwrap();
        file.seek(SeekFrom::Start(self.file_data_end_offset))?;
        file.write_all(&(table_compressed.len() as u32).to_le_bytes())?;
        file.write_all(&(table_raw.len() as u32).to_le_bytes())?;
        file.write_all(&table_compressed)?;

        let end_pos = self.file_data_end_offset + 8 + table_compressed.len() as u64;
        file.set_len(end_pos)?;

        let file_table_offset = (self.file_data_end_offset - HEADER_SIZE as u64) as u32;
        let file_count = (self.entries.len() + FILE_OFFSET) as u32;

        file.seek(SeekFrom::Start(30))?;
        file.write_all(&file_table_offset.to_le_bytes())?;
        file.write_all(&0u32.to_le_bytes())?;
        file.write_all(&file_count.to_le_bytes())?;
        file.flush()?;

        Ok(())
    }

    pub fn repack(&mut self) -> Result<(), FormatError> {
        if !self.writable {
            return Err(FormatError::ReadOnly);
        }

        let mut blobs: Vec<(String, GrfEntry, Vec<u8>)> = Vec::with_capacity(self.entries.len());
        {
            let file = self.file.get_mut().unwrap();
            for (name, entry) in &self.entries {
                let position = entry.offset + HEADER_SIZE as u64;
                let mut blob = vec![0u8; entry.compressed_size_aligned as usize];
                file.seek(SeekFrom::Start(position))?;
                file.read_exact(&mut blob)?;
                blobs.push((
                    name.clone(),
                    GrfEntry {
                        compressed_size: entry.compressed_size,
                        compressed_size_aligned: entry.compressed_size_aligned,
                        uncompressed_size: entry.uncompressed_size,
                        flags: entry.flags,
                        offset: 0,
                    },
                    blob,
                ));
            }
        }

        let file = self.file.get_mut().unwrap();
        let mut write_pos = HEADER_SIZE as u64;
        self.entries.clear();

        for (name, mut entry, blob) in blobs {
            file.seek(SeekFrom::Start(write_pos))?;
            file.write_all(&blob)?;
            entry.offset = write_pos - HEADER_SIZE as u64;
            write_pos += blob.len() as u64;
            self.entries.insert(name, entry);
        }

        self.file_data_end_offset = write_pos;
        self.save()
    }

    pub fn file_exists(&self, name: &str) -> bool {
        self.entry_exists(name)
            || self
                .alias_table()
                .resolve(name)
                .is_some_and(|alias| self.entry_exists(&alias))
    }

    fn entry_exists(&self, name: &str) -> bool {
        if self
            .data_dir
            .as_ref()
            .and_then(|d| d.lookup(name))
            .is_some()
        {
            return true;
        }
        let name_lower = name.to_lowercase().replace('\\', "/");
        self.overrides.contains_key(&name_lower)
            || self.entries.contains_key(&name_lower)
            || self
                .overlays
                .iter()
                .any(|o| o.entries.contains_key(&name_lower))
    }

    /// Counts only the primary archive; overlays and `data_dir` are not
    /// included. The same holds for the other listing methods below.
    pub fn file_count(&self) -> usize {
        self.entries.len()
    }

    pub fn entry_names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn find_first_with_extension(&self, extension: &str) -> Option<&str> {
        let ext = extension.to_lowercase();
        self.entries
            .keys()
            .find(|name| name.ends_with(&ext))
            .map(|s| s.as_str())
    }

    pub fn files_with_extension(&self, extension: &str) -> Vec<&str> {
        let ext = extension.to_lowercase();
        self.entries
            .keys()
            .filter(|name| name.ends_with(&ext))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn file_list(&self) -> Vec<GrfFileInfo> {
        let mut list: Vec<GrfFileInfo> = self
            .entries
            .iter()
            .map(|(name, entry)| GrfFileInfo {
                name: name.clone(),
                compressed_size: entry.compressed_size,
                compressed_size_aligned: entry.compressed_size_aligned,
                uncompressed_size: entry.uncompressed_size,
                flags: entry.flags,
            })
            .collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    /// Every name in the layer stack, listed once and resolved the way
    /// `read_file` resolves it: a name several archives carry is reported with
    /// the sizes of the earliest one that has it. Ordered by layer, then name.
    /// `data_dir` and overrides are not listed.
    pub fn layered_file_list(&self) -> Vec<GrfFileInfo> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut list = Vec::new();
        for layer in std::iter::once(self).chain(self.overlays.iter()) {
            let mut names: Vec<&String> = layer.entries.keys().collect();
            names.sort();
            for name in names {
                if !seen.insert(name.as_str()) {
                    continue;
                }
                let entry = &layer.entries[name];
                list.push(GrfFileInfo {
                    name: name.clone(),
                    compressed_size: entry.compressed_size,
                    compressed_size_aligned: entry.compressed_size_aligned,
                    uncompressed_size: entry.uncompressed_size,
                    flags: entry.flags,
                });
            }
        }
        list
    }

    pub fn file_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.entries.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn zlib_compress(data: &[u8]) -> Result<Vec<u8>, FormatError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish().map_err(FormatError::Io)
}

fn build_file_table(entries: &HashMap<String, GrfEntry>) -> Vec<u8> {
    let mut table = Vec::new();
    for (name, entry) in entries {
        let backslash_name = name.replace('/', "\\");
        let (encoded, _, _) = encoding_rs::EUC_KR.encode(&backslash_name);
        table.extend_from_slice(&encoded);
        table.push(0);
        table.extend_from_slice(&entry.compressed_size.to_le_bytes());
        table.extend_from_slice(&entry.compressed_size_aligned.to_le_bytes());
        table.extend_from_slice(&entry.uncompressed_size.to_le_bytes());
        table.push(entry.flags);
        table.extend_from_slice(&(entry.offset as u32).to_le_bytes());
    }
    table
}

fn read_u32_le(file: &mut File) -> Result<u32, FormatError> {
    let mut buf = [0u8; 4];
    file.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

#[derive(Clone, Copy)]
enum OffsetWidth {
    U32,
    U64,
}

fn parse_v2_entries(
    file: &mut File,
    file_count: usize,
    offset_width: OffsetWidth,
) -> Result<HashMap<String, GrfEntry>, FormatError> {
    let entry_len = match offset_width {
        OffsetWidth::U32 => 17,
        OffsetWidth::U64 => 21,
    };
    let table_compressed_size = read_u32_le(file)?;
    let table_uncompressed_size = read_u32_le(file)?;

    let mut compressed_table = vec![0u8; table_compressed_size as usize];
    file.read_exact(&mut compressed_table)?;

    let mut table = Vec::with_capacity(table_uncompressed_size as usize);
    let mut decoder = ZlibDecoder::new(compressed_table.as_slice());
    decoder
        .read_to_end(&mut table)
        .map_err(|e| FormatError::DecompressionFailed(e.to_string()))?;

    // the header's count is trusted only as far as the table goes: a writer counting differently still opens
    let mut entries = HashMap::with_capacity(file_count.min(table.len() / entry_len));
    let mut pos = 0;
    for _ in 0..file_count {
        if pos >= table.len() {
            break;
        }
        let name_end = table[pos..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(FormatError::UnexpectedEof)?;
        let name_bytes = &table[pos..pos + name_end];
        let (name_decoded, _, _) = encoding_rs::EUC_KR.decode(name_bytes);
        let name = name_decoded.into_owned().to_lowercase().replace('\\', "/");
        pos += name_end + 1;

        if pos + entry_len > table.len() {
            return Err(FormatError::UnexpectedEof);
        }

        let compressed_size = u32::from_le_bytes(table[pos..pos + 4].try_into().unwrap());
        let compressed_size_aligned =
            u32::from_le_bytes(table[pos + 4..pos + 8].try_into().unwrap());
        let uncompressed_size = u32::from_le_bytes(table[pos + 8..pos + 12].try_into().unwrap());
        let flags = table[pos + 12];
        let offset = match offset_width {
            OffsetWidth::U32 => {
                u32::from_le_bytes(table[pos + 13..pos + 17].try_into().unwrap()) as u64
            }
            OffsetWidth::U64 => u64::from_le_bytes(table[pos + 13..pos + 21].try_into().unwrap()),
        };
        pos += entry_len;

        entries.insert(
            name,
            GrfEntry {
                compressed_size,
                compressed_size_aligned,
                uncompressed_size,
                flags,
                offset,
            },
        );
    }
    Ok(entries)
}

fn parse_v1_entries(
    file: &mut File,
    file_count: usize,
) -> Result<HashMap<String, GrfEntry>, FormatError> {
    let mut table = Vec::new();
    file.read_to_end(&mut table)?;

    let mut entries = HashMap::with_capacity(file_count);
    let mut ofs = 0;
    for _ in 0..file_count {
        if ofs + 6 > table.len() {
            return Err(FormatError::UnexpectedEof);
        }
        let entry_len = u32::from_le_bytes(table[ofs..ofs + 4].try_into().unwrap()) as usize;
        let name_len = (entry_len as u8).wrapping_sub(6) as usize;
        if ofs + 6 + name_len > table.len() {
            return Err(FormatError::UnexpectedEof);
        }
        let decrypt_len = (name_len + 7) & !7;
        let mut name_buf = vec![0u8; decrypt_len];
        let copy_len = name_len.min(table.len() - (ofs + 6));
        name_buf[..copy_len].copy_from_slice(&table[ofs + 6..ofs + 6 + copy_len]);
        mixcrypt::decrypt_filename(&mut name_buf);

        let name_end = name_buf.iter().position(|&b| b == 0).unwrap_or(name_len);
        let (name_decoded, _, _) = encoding_rs::EUC_KR.decode(&name_buf[..name_end]);
        let name = name_decoded.into_owned().to_lowercase().replace('\\', "/");

        let ofs2 = ofs + entry_len + 4;
        if ofs2 + 17 > table.len() {
            return Err(FormatError::UnexpectedEof);
        }

        let raw_compressed = u32::from_le_bytes(table[ofs2..ofs2 + 4].try_into().unwrap());
        let raw_aligned = u32::from_le_bytes(table[ofs2 + 4..ofs2 + 8].try_into().unwrap());
        let uncompressed_size = u32::from_le_bytes(table[ofs2 + 8..ofs2 + 12].try_into().unwrap());
        let entry_type = table[ofs2 + 12];
        let offset = u32::from_le_bytes(table[ofs2 + 13..ofs2 + 17].try_into().unwrap());

        let compressed_size = raw_compressed
            .wrapping_sub(uncompressed_size)
            .wrapping_sub(715);
        let compressed_size_aligned = raw_aligned.wrapping_sub(37579);
        let flags = if entry_type & 0x01 != 0 {
            encryption_flags_for_extension(&name) | 0x01
        } else {
            entry_type
        };

        ofs = ofs2 + 17;

        if entry_type & 0x01 == 0 {
            continue;
        }

        entries.insert(
            name,
            GrfEntry {
                compressed_size,
                compressed_size_aligned,
                uncompressed_size,
                flags,
                offset: offset as u64,
            },
        );
    }
    Ok(entries)
}

fn encryption_flags_for_extension(filename: &str) -> u8 {
    match filename.rsplit('.').next() {
        Some("gnd" | "gat" | "act" | "str") => mixcrypt::GRF_FLAG_HEADER_DES_CRYPT,
        _ => mixcrypt::GRF_FLAG_FULL_MIX_CRYPT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_refuses_data_past_4_gib() {
        let path = std::env::temp_dir().join(format!("grf_unit_{}_big.grf", std::process::id()));
        let mut grf = GrfArchive::create(&path).unwrap();
        grf.file_data_end_offset = HEADER_SIZE as u64 + u32::MAX as u64 + 1;
        let saved = grf.save();
        drop(grf);
        let _ = std::fs::remove_file(&path);
        assert!(saved.is_err());
    }
}

#[cfg(test)]
mod cache_regression {
 use super::*;
 #[test]
 fn repeated_read_only_asset_avoids_disk_and_decompression() {
  let path=std::env::temp_dir().join(format!("grf_cache_{}.grf",std::process::id()));
  let mut archive=GrfArchive::create(&path).unwrap();
  archive.add_file("data/foo.spr",&[42;65536]).unwrap();archive.save().unwrap();drop(archive);
  let archive=GrfArchive::open(&path).unwrap();
  for name in ["data/foo.spr","DATA/FOO.SPR","data\\foo.spr"] {assert_eq!(archive.read_file(name).unwrap(),vec![42;65536]);}
  assert_eq!(archive.archive_reads.load(std::sync::atomic::Ordering::Relaxed),1,"same immutable resource was physically read and decompressed again");
  drop(archive);std::fs::remove_file(path).unwrap();
 }
}

#[cfg(test)]mod cache_benchmark {
 use super::*;
 #[test]#[ignore]fn repeated_asset_benchmark(){
  let path=std::env::temp_dir().join(format!("grf_bench_{}.grf",std::process::id()));
  let bytes:Vec<u8>=(0..256*1024).map(|i|((i*37+i/17)%251)as u8).collect();
  let mut archive=GrfArchive::create(&path).unwrap();archive.add_file("data/model.rsm",&bytes).unwrap();archive.save().unwrap();drop(archive);
  let archive=GrfArchive::open(&path).unwrap();let start=std::time::Instant::now();
  for _ in 0..512 {std::hint::black_box(archive.read_file("data/model.rsm").unwrap());}
  println!("GRF_BENCH reads=512 bytes_per_read={} elapsed_us={} physical_reads={}",bytes.len(),start.elapsed().as_micros(),archive.archive_reads.load(std::sync::atomic::Ordering::Relaxed));
  drop(archive);std::fs::remove_file(path).unwrap();
 }
}

#[cfg(test)]mod cache_safety {
 use super::*;
 fn file(label:&str)->PathBuf{std::env::temp_dir().join(format!("grf_safe_{}_{label}.grf",std::process::id()))}
 fn make(path:&Path,name:&str,bytes:&[u8]){let mut a=GrfArchive::create(path).unwrap();a.add_file(name,bytes).unwrap();a.save().unwrap();}
 #[test]fn offline_map_geometry_stays_with_archives_but_translations_stay_loose(){
  let path=file("offline_geometry");
  let mut base=GrfArchive::create(&path).unwrap();
  for ext in ["gat","gnd","rsw"] {base.add_file(&format!("data/izlude.{ext}"),format!("renewal {ext}").as_bytes()).unwrap();}
  base.add_file("data/table.txt",b"Korean").unwrap();base.save().unwrap();drop(base);
  let dir=path.with_extension("dir");std::fs::create_dir_all(&dir).unwrap();
  for ext in ["gat","gnd","rsw"] {std::fs::write(dir.join(format!("izlude.{ext}")),format!("old map {ext}")).unwrap();}
  std::fs::write(dir.join("table.txt"),b"Portugues BR").unwrap();
  std::fs::write(dir.join("custom.gat"),b"custom map only on disk").unwrap();
  let mut archive=GrfArchive::open_layered(&[path.to_string_lossy().into_owned()],Some(&dir)).unwrap();
  assert_eq!(archive.read_file("data/izlude.gat").unwrap(),b"old map gat","generic archive behavior must remain configurable");
  archive.use_archived_map_geometry();
  for ext in ["gat","gnd","rsw"] {assert_eq!(archive.read_file(&format!("DATA\\IZLUDE.{ext}")).unwrap(),format!("renewal {ext}").as_bytes(),"loose old geometry replaced the offline server map");}
  assert_eq!(archive.read_file("data/table.txt").unwrap(),b"Portugues BR");
  assert_eq!(archive.read_file("data/custom.gat").unwrap(),b"custom map only on disk");
  assert_eq!(archive.use_archived_map_geometry(),0,"the policy is idempotent");
  drop(archive);std::fs::remove_file(path).unwrap();std::fs::remove_dir_all(dir).unwrap();
 }
 #[test]fn loose_translation_is_reread_and_override_wins_after_cache_warmup(){
  let path=file("loose");make(&path,"data/table.txt",b"original");
  let dir=path.with_extension("dir");std::fs::create_dir_all(&dir).unwrap();std::fs::write(dir.join("table.txt"),b"traducao A").unwrap();
  let mut a=GrfArchive::open_layered(&[path.to_string_lossy().into_owned()],Some(&dir)).unwrap();
  assert_eq!(a.read_archive_entry("data/table.txt").unwrap(),b"original");
  assert_eq!(a.read_file("data/table.txt").unwrap(),b"traducao A");
  std::fs::write(dir.join("table.txt"),b"traducao B").unwrap();assert_eq!(a.read_file("data/table.txt").unwrap(),b"traducao B");
  a.set_override("data/table.txt",b"override".to_vec());assert_eq!(a.read_file("data/table.txt").unwrap(),b"override");
  drop(a);std::fs::remove_file(path).unwrap();std::fs::remove_dir_all(dir).unwrap();
 }
 #[test]fn writable_archives_bypass_cache_and_never_return_stale_edits(){
  let path=file("rw");make(&path,"data/foo.txt",b"A");let mut a=GrfArchive::open_rw(&path).unwrap();
  assert!(a.read_cache.is_none());assert_eq!(a.read_file("data/foo.txt").unwrap(),b"A");a.add_file("data/foo.txt",b"B").unwrap();
  assert_eq!(a.read_file("data/foo.txt").unwrap(),b"B");a.remove_file("data/foo.txt").unwrap();assert!(a.read_file("data/foo.txt").is_err());
  drop(a);std::fs::remove_file(path).unwrap();
 }
 #[test]fn concurrent_warm_reads_are_consistent_and_use_no_more_disk_reads(){
  let path=file("threads");make(&path,"data/foo.txt",&[13;4096]);
  let a=std::sync::Arc::new(GrfArchive::open(&path).unwrap());a.read_file("data/foo.txt").unwrap();
  let workers:Vec<_>=(0..8).map(|_|{let a=a.clone();std::thread::spawn(move||for _ in 0..100{assert_eq!(a.read_file("data/foo.txt").unwrap(),vec![13;4096]);})}).collect();
  for w in workers{w.join().unwrap();}assert_eq!(a.archive_reads.load(std::sync::atomic::Ordering::Relaxed),1);
  drop(a);std::fs::remove_file(path).unwrap();
 }
}
