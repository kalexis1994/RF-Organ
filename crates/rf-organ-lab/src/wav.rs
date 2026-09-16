// SPDX-License-Identifier: GPL-2.0-or-later

#[derive(Clone, Debug)]
pub struct Audio {
    pub sample_rate: u32,
    pub frames: Vec<[f64; 2]>,
}

#[derive(Clone, Copy)]
struct Format {
    encoding: u16,
    channels: u16,
    sample_rate: u32,
    block_align: u16,
    bits: u16,
}

pub fn encode_f32(samples: &[f32], channels: u16, sample_rate: u32) -> Result<Vec<u8>, String> {
    if channels == 0 || !samples.len().is_multiple_of(usize::from(channels)) {
        return Err("sample data must contain complete nonzero-channel frames".into());
    }
    let data_bytes = u32::try_from(samples.len().checked_mul(4).ok_or("WAV data too large")?)
        .map_err(|_| "WAV data too large")?;
    let riff_size = 36_u32.checked_add(data_bytes).ok_or("WAV data too large")?;
    let byte_rate = sample_rate
        .checked_mul(u32::from(channels))
        .and_then(|value| value.checked_mul(4))
        .ok_or("WAV rate too large")?;
    let block_align = channels
        .checked_mul(4)
        .ok_or("WAV channel count too large")?;
    let mut bytes = Vec::with_capacity(riff_size as usize + 8);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&3_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(bytes)
}

pub fn decode(bytes: &[u8]) -> Result<Audio, String> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".into());
    }
    let mut format = None;
    let mut data = None;
    let mut offset = 12;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = usize::try_from(read_u32(bytes, offset + 4)?).map_err(|_| "chunk too large")?;
        let start = offset + 8;
        let end = start.checked_add(size).ok_or("invalid WAV chunk size")?;
        if end > bytes.len() {
            return Err("truncated WAV chunk".into());
        }
        match id {
            b"fmt " => format = Some(parse_format(&bytes[start..end])?),
            b"data" => data = Some(&bytes[start..end]),
            _ => {}
        }
        offset = end.checked_add(size & 1).ok_or("invalid WAV padding")?;
    }
    let format = format.ok_or("WAV has no fmt chunk")?;
    let data = data.ok_or("WAV has no data chunk")?;
    decode_samples(format, data)
}

fn parse_format(bytes: &[u8]) -> Result<Format, String> {
    if bytes.len() < 16 {
        return Err("truncated WAV format".into());
    }
    let mut encoding = read_u16(bytes, 0)?;
    let channels = read_u16(bytes, 2)?;
    let sample_rate = read_u32(bytes, 4)?;
    let block_align = read_u16(bytes, 12)?;
    let bits = read_u16(bytes, 14)?;
    if encoding == 0xfffe {
        if bytes.len() < 40 {
            return Err("truncated extensible WAV format".into());
        }
        let extension_size = read_u16(bytes, 16)?;
        let valid_bits = read_u16(bytes, 18)?;
        if extension_size < 22 || valid_bits == 0 || valid_bits > bits {
            return Err("inconsistent extensible WAV format".into());
        }
        const GUID_SUFFIX: [u8; 14] = [
            0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
        ];
        if bytes[26..40] != GUID_SUFFIX {
            return Err("unsupported extensible WAV subformat".into());
        }
        encoding = read_u16(bytes, 24)?;
    }
    if !(1..=2).contains(&channels) {
        return Err("only mono and stereo WAV captures are supported".into());
    }
    let supported = matches!((encoding, bits), (1, 16 | 24 | 32) | (3, 32));
    if !supported {
        return Err(format!(
            "unsupported WAV encoding {encoding} with {bits} bits"
        ));
    }
    let bytes_per_sample = bits / 8;
    if block_align != channels * bytes_per_sample || sample_rate == 0 {
        return Err("inconsistent WAV format".into());
    }
    Ok(Format {
        encoding,
        channels,
        sample_rate,
        block_align,
        bits,
    })
}

fn decode_samples(format: Format, data: &[u8]) -> Result<Audio, String> {
    let stride = usize::from(format.block_align);
    if !data.len().is_multiple_of(stride) {
        return Err("WAV data contains an incomplete frame".into());
    }
    let bytes_per_sample = usize::from(format.bits / 8);
    let mut frames = Vec::with_capacity(data.len() / stride);
    for frame in data.chunks_exact(stride) {
        let left = decode_sample(format, &frame[..bytes_per_sample])?;
        let right = if format.channels == 1 {
            left
        } else {
            decode_sample(format, &frame[bytes_per_sample..bytes_per_sample * 2])?
        };
        frames.push([left, right]);
    }
    Ok(Audio {
        sample_rate: format.sample_rate,
        frames,
    })
}

fn decode_sample(format: Format, bytes: &[u8]) -> Result<f64, String> {
    let value = match (format.encoding, format.bits) {
        (1, 16) => f64::from(i16::from_le_bytes(bytes.try_into().expect("two bytes"))) / 32_768.0,
        (1, 24) => {
            let raw =
                i32::from(bytes[0]) | (i32::from(bytes[1]) << 8) | (i32::from(bytes[2]) << 16);
            let signed = if raw & 0x80_0000 == 0 {
                raw
            } else {
                raw | !0xff_ffff
            };
            f64::from(signed) / 8_388_608.0
        }
        (1, 32) => {
            f64::from(i32::from_le_bytes(bytes.try_into().expect("four bytes"))) / 2_147_483_648.0
        }
        (3, 32) => f64::from(f32::from_le_bytes(bytes.try_into().expect("four bytes"))),
        _ => unreachable!("format was validated"),
    };
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| "WAV contains a non-finite sample".into())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    bytes
        .get(offset..offset + 2)
        .ok_or_else(|| "truncated WAV integer".to_owned())?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| "invalid WAV integer".into())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "truncated WAV integer".to_owned())?
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| "invalid WAV integer".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_encoder_round_trips_stereo() {
        let samples = [0.0, 0.25, 0.5, -0.5];
        let bytes = encode_f32(&samples, 2, 48_000).unwrap();
        let audio = decode(&bytes).unwrap();
        assert_eq!(audio.sample_rate, 48_000);
        assert_eq!(audio.frames, vec![[0.0, 0.25], [0.5, -0.5]]);
    }

    #[test]
    fn decoder_accepts_pcm16_mono() {
        let mut bytes = encode_f32(&[0.0], 1, 48_000).unwrap();
        bytes[20..22].copy_from_slice(&1_u16.to_le_bytes());
        bytes[28..32].copy_from_slice(&96_000_u32.to_le_bytes());
        bytes[32..34].copy_from_slice(&2_u16.to_le_bytes());
        bytes[34..36].copy_from_slice(&16_u16.to_le_bytes());
        bytes[4..8].copy_from_slice(&38_u32.to_le_bytes());
        bytes[40..44].copy_from_slice(&2_u32.to_le_bytes());
        bytes.truncate(46);
        bytes[44..46].copy_from_slice(&i16::MIN.to_le_bytes());
        let audio = decode(&bytes).unwrap();
        assert_eq!(audio.frames, vec![[-1.0, -1.0]]);
    }

    #[test]
    fn decoder_rejects_non_finite_float_samples() {
        let bytes = encode_f32(&[f32::NAN], 1, 48_000).unwrap();
        assert!(decode(&bytes).is_err());
    }

    #[test]
    fn parser_accepts_the_standard_extensible_float_guid() {
        let mut format = [0_u8; 40];
        format[0..2].copy_from_slice(&0xfffe_u16.to_le_bytes());
        format[2..4].copy_from_slice(&2_u16.to_le_bytes());
        format[4..8].copy_from_slice(&48_000_u32.to_le_bytes());
        format[12..14].copy_from_slice(&8_u16.to_le_bytes());
        format[14..16].copy_from_slice(&32_u16.to_le_bytes());
        format[16..18].copy_from_slice(&22_u16.to_le_bytes());
        format[18..20].copy_from_slice(&32_u16.to_le_bytes());
        format[24..28].copy_from_slice(&3_u32.to_le_bytes());
        format[28..40].copy_from_slice(&[
            0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
        ]);
        let parsed = parse_format(&format).unwrap();
        assert_eq!(parsed.encoding, 3);
    }
}
