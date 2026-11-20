//! Just enough of the zip format to hold a Word document's parts: each file
//! compressed with deflate, then the central directory that lists them.

use std::io::Write;

use flate2::Compression;
use flate2::write::DeflateEncoder;

/// 1 January 1980, the earliest date zip can store, so the output is the same
/// each time for the same input.
const DOS_DATE: u16 = (1 << 5) | 1;
/// Names are UTF-8.
const FLAGS: u16 = 1 << 11;
const DEFLATE: u16 = 8;
const VERSION: u16 = 20;

/// A zip archive of `files`, as (name, contents), in the order given.
pub fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut directory = Vec::new();
    for (name, data) in files {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).expect("writing to memory can't fail");
        let packed = encoder.finish().expect("writing to memory can't fail");
        let crc = crc32fast::hash(data);
        let offset = u32::try_from(out.len()).expect("a report is far below 4 GB");
        let sizes = [len32(packed.len()), len32(data.len())];
        let name_len = u16::try_from(name.len()).expect("part names are short");

        // Local file header.
        put32(&mut out, 0x0403_4b50);
        for v in [VERSION, FLAGS, DEFLATE, 0, DOS_DATE] {
            put16(&mut out, v);
        }
        for v in [crc, sizes[0], sizes[1]] {
            put32(&mut out, v);
        }
        put16(&mut out, name_len);
        put16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&packed);

        // Its entry in the central directory.
        put32(&mut directory, 0x0201_4b50);
        for v in [VERSION, VERSION, FLAGS, DEFLATE, 0, DOS_DATE] {
            put16(&mut directory, v);
        }
        for v in [crc, sizes[0], sizes[1]] {
            put32(&mut directory, v);
        }
        for v in [name_len, 0, 0, 0, 0] {
            put16(&mut directory, v);
        }
        put32(&mut directory, 0);
        put32(&mut directory, offset);
        directory.extend_from_slice(name.as_bytes());
    }
    let count = u16::try_from(files.len()).expect("a report has a few parts");
    let directory_offset = len32(out.len());
    out.extend_from_slice(&directory);
    // End of central directory.
    put32(&mut out, 0x0605_4b50);
    for v in [0, 0, count, count] {
        put16(&mut out, v);
    }
    put32(&mut out, len32(directory.len()));
    put32(&mut out, directory_offset);
    put16(&mut out, 0);
    out
}

fn len32(n: usize) -> u32 {
    u32::try_from(n).expect("a report is far below 4 GB")
}

fn put16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Reads back what `zip` writes, for tests.
#[cfg(test)]
pub fn unzip(archive: &[u8]) -> Vec<(String, Vec<u8>)> {
    use std::io::Read;
    let u16_at = |i: usize| u16::from_le_bytes([archive[i], archive[i + 1]]) as usize;
    let u32_at = |i: usize| u32::from_le_bytes(archive[i..i + 4].try_into().unwrap()) as usize;
    let end = archive.len() - 22;
    assert_eq!(u32_at(end), 0x0605_4b50, "end of central directory");
    let (count, mut at) = (u16_at(end + 10), u32_at(end + 16));
    let mut files = Vec::new();
    for _ in 0..count {
        assert_eq!(u32_at(at), 0x0201_4b50, "central directory entry");
        let (crc, packed, size, name_len, local) = (u32_at(at + 16), u32_at(at + 20), u32_at(at + 24), u16_at(at + 28), u32_at(at + 42));
        let name = String::from_utf8(archive[at + 46..at + 46 + name_len].to_vec()).unwrap();
        assert_eq!(u32_at(local), 0x0403_4b50, "local header for {name}");
        let start = local + 30 + u16_at(local + 26) + u16_at(local + 28);
        let mut data = Vec::new();
        flate2::read::DeflateDecoder::new(&archive[start..start + packed]).read_to_end(&mut data).unwrap();
        assert_eq!(data.len(), size, "{name}");
        assert_eq!(crc32fast::hash(&data) as usize, crc, "{name}");
        files.push((name, data));
        at += 46 + name_len;
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_goes_in_comes_out() {
        let big = "Łódź £ ".repeat(1000);
        let files: [(&str, &[u8]); 3] = [("a.xml", b"<a/>"), ("dir/b.txt", big.as_bytes()), ("empty", b"")];
        let archive = zip(&files);
        let back = unzip(&archive);
        assert_eq!(back.len(), 3);
        for ((name, data), (got_name, got)) in files.iter().zip(&back) {
            assert_eq!(name, got_name);
            assert_eq!(*data, got.as_slice());
        }
        assert!(archive.len() < big.len() / 10, "it is compressed");
    }
}
