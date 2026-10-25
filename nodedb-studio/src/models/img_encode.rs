//! A minimal PNG and base64 encoder, so a raster draws as one `<image>`
//! without a dependency. The PNG holds its pixels in stored (uncompressed)
//! deflate blocks, which every decoder reads.

const CRC_POLY: u32 = 0xEDB8_8320;
const MAX_STORED: usize = 65_535;

fn crc32(chunks: &[&[u8]]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for chunk in chunks {
        for byte in *chunk {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ CRC_POLY
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in data {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
}

/// An 8-bit RGBA PNG. `rgba` holds `width * height * 4` bytes, row by row.
/// `None` when the length disagrees with the size or a side is zero.
pub fn png_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return None;
    }
    // Every scanline starts with filter type 0 (none).
    let mut raw = Vec::with_capacity(h * (w * 4 + 1));
    for row in rgba.chunks_exact(w * 4) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(MAX_STORED).peekable();
    while let Some(block) = blocks.next() {
        let last = blocks.peek().is_none();
        let len = block.len() as u16;
        zlib.push(u8::from(last));
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    // 8 bits, RGBA, deflate, adaptive filtering, no interlace.
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    Some(out)
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding.
pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for group in data.chunks(3) {
        let n = group
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= group.len() {
                out.push(char::from(B64[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A `data:` URI for an RGBA image. `None` on a size mismatch.
pub fn png_data_uri(width: u32, height: u32, rgba: &[u8]) -> Option<String> {
    let png = png_rgba(width, height, rgba)?;
    Some(format!("data:image/png;base64,{}", base64(&png)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_check_value() {
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
        assert_eq!(crc32(&[b"1234", b"56789"]), 0xCBF4_3926);
    }

    #[test]
    fn adler32_matches_the_reference_value() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn base64_matches_the_rfc_vectors() {
        for (input, want) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), want, "{input}");
        }
    }

    #[test]
    fn a_png_has_its_signature_header_and_end() {
        let rgba = vec![255u8; 3 * 2 * 4];
        let png = png_rgba(3, 2, &rgba).expect("png");
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(u32::from_be_bytes([png[16], png[17], png[18], png[19]]), 3);
        assert_eq!(u32::from_be_bytes([png[20], png[21], png[22], png[23]]), 2);
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
    }

    #[test]
    fn a_large_image_splits_into_stored_blocks() {
        let (w, h) = (360u32, 181u32);
        let rgba = vec![7u8; (w * h * 4) as usize];
        let png = png_rgba(w, h, &rgba).expect("png");
        let raw = (h * (w * 4 + 1)) as usize;
        let blocks = raw.div_ceil(MAX_STORED);
        assert!(blocks > 1);
        // Signature, three chunk frames, the IHDR body, the zlib header,
        // one 5-byte frame per block, the data, and the Adler-32.
        assert_eq!(
            png.len(),
            8 + 12 + 13 + 12 + (2 + blocks * 5 + raw + 4) + 12
        );
    }

    #[test]
    fn a_size_mismatch_makes_no_image() {
        assert!(png_rgba(2, 2, &[0; 8]).is_none());
        assert!(png_rgba(0, 2, &[]).is_none());
        assert!(png_data_uri(1, 1, &[0, 0, 0, 255]).is_some());
    }
}
