//! IEC 60870-5-104 线路值的校验与量化。

use super::{TypeId, lookup_type_descriptor};

const FILE_TRANSFER_FIXED_ASDU_BYTES: u16 = 13;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasurementValueError;

pub fn encode_asdu(
    type_id: u8,
    cause: u16,
    common_address: u16,
    payload: &[u8],
    object_count: u8,
    sequence: bool,
) -> Vec<u8> {
    let vsq = (object_count & 0x7F) | if sequence { 0x80 } else { 0 };
    let mut bytes = Vec::with_capacity(6 + payload.len());
    bytes.push(type_id);
    bytes.push(vsq);
    bytes.extend_from_slice(&cause.to_le_bytes());
    bytes.extend_from_slice(&common_address.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

pub fn encode_ioa(ioa: u32) -> [u8; 3] {
    let bytes = ioa.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

pub fn encode_u24(value: u32) -> [u8; 3] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

pub fn decode_ioa(input: &[u8]) -> Option<u32> {
    let bytes = input.get(..3)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]))
}

pub fn encode_information_object(ioa: u32, fields: &[&[u8]]) -> Vec<u8> {
    let field_len = fields.iter().map(|field| field.len()).sum::<usize>();
    let mut payload = Vec::with_capacity(3 + field_len);
    payload.extend_from_slice(&encode_ioa(ioa));
    for field in fields {
        payload.extend_from_slice(field);
    }
    payload
}

pub fn file_transfer_segment_payload_max(max_asdu_bytes: u16) -> Option<usize> {
    let available = max_asdu_bytes.checked_sub(FILE_TRANSFER_FIXED_ASDU_BYTES)?;
    (available > 0).then_some(usize::from(available).min(u8::MAX as usize))
}

pub fn file_transfer_single_section_max_file_size(max_asdu_bytes: u16) -> Option<u32> {
    file_transfer_segment_payload_max(max_asdu_bytes).map(|bytes| (bytes * u8::MAX as usize) as u32)
}

pub fn encode_ioa_qualifier(ioa: u32, qualifier: u8) -> Vec<u8> {
    encode_information_object(ioa, &[&[qualifier]])
}

pub fn encode_single_command(ioa: u32, value: bool, select: bool, qu: u8) -> Vec<u8> {
    let qualifier = u8::from(value) | ((qu & 0x1F) << 2) | if select { 0x80 } else { 0 };
    encode_ioa_qualifier(ioa, qualifier)
}

pub fn encode_double_command(ioa: u32, value: u8, select: bool, qu: u8) -> Vec<u8> {
    let qualifier = (value & 0x03) | ((qu & 0x1F) << 2) | if select { 0x80 } else { 0 };
    encode_ioa_qualifier(ioa, qualifier)
}

pub fn encode_regulating_step_command(ioa: u32, step: u8, select: bool, qu: u8) -> Vec<u8> {
    let qualifier = (step & 0x03) | ((qu & 0x1F) << 2) | if select { 0x80 } else { 0 };
    encode_ioa_qualifier(ioa, qualifier)
}

pub fn encode_setpoint_i16(ioa: u32, value: i16, qualifier: u8) -> Vec<u8> {
    encode_information_object(ioa, &[&value.to_le_bytes(), &[qualifier]])
}

pub fn encode_setpoint_f32(ioa: u32, value: f32, qualifier: u8) -> Vec<u8> {
    encode_information_object(ioa, &[&value.to_le_bytes(), &[qualifier]])
}

pub fn encode_clock_sync_command(ioa: u32, timestamp: [u8; 7]) -> Vec<u8> {
    encode_information_object(ioa, &[&timestamp])
}

pub fn encode_test_command(ioa: u32, pattern: u16) -> Vec<u8> {
    encode_information_object(ioa, &[&pattern.to_le_bytes()])
}

pub fn encode_timed_test_command(ioa: u32, pattern: u16, timestamp: [u8; 7]) -> Vec<u8> {
    encode_information_object(ioa, &[&pattern.to_le_bytes(), &timestamp])
}

pub fn encode_bitstring_command(ioa: u32, value: u32) -> Vec<u8> {
    encode_information_object(ioa, &[&value.to_le_bytes()])
}

pub fn encode_read_command(ioa: u32) -> Vec<u8> {
    encode_information_object(ioa, &[])
}

pub fn append_cp56_time2a(mut payload: Vec<u8>, timestamp: [u8; 7]) -> Vec<u8> {
    payload.extend_from_slice(&timestamp);
    payload
}

pub fn compose_sq1_payload(chunks: &[Vec<u8>]) -> Option<Vec<u8>> {
    if chunks.len() < 2 {
        return None;
    }
    let first = chunks.first()?;
    let object_len = first.len();
    if object_len < 3 {
        return None;
    }

    let first_ioa = decode_ioa(first)?;
    let mut combined = Vec::with_capacity(object_len + (chunks.len() - 1) * (object_len - 3));
    combined.extend_from_slice(first);
    for (index, payload) in chunks.iter().enumerate().skip(1) {
        if payload.len() != object_len || decode_ioa(payload)? != first_ioa.checked_add(index as u32)? {
            return None;
        }
        combined.extend_from_slice(&payload[3..]);
    }
    Some(combined)
}

pub fn encode_file_ready(ioa: u32, file_name: u16, qualifier: u8, length: u32) -> Vec<u8> {
    encode_information_object(ioa, &[&file_name.to_le_bytes(), &[qualifier], &encode_u24(length)])
}

pub fn encode_section_ready(ioa: u32, file_name: u16, qualifier: u8, section: u8, length: u32) -> Vec<u8> {
    encode_information_object(ioa, &[&file_name.to_le_bytes(), &[qualifier], &[section], &encode_u24(length)])
}

pub fn encode_select_call(ioa: u32, file_name: u16, qualifier: u8, section: u8) -> Vec<u8> {
    encode_information_object(ioa, &[&file_name.to_le_bytes(), &[qualifier], &[section]])
}

pub fn encode_last_section(
    ioa: u32,
    file_name: u16,
    qualifier: u8,
    last_section_number: u8,
    last_segment_number: u8,
) -> Vec<u8> {
    encode_information_object(ioa, &[&file_name.to_le_bytes(), &[qualifier], &[last_section_number], &[
        last_segment_number,
    ]])
}

pub fn encode_ack_section(ioa: u32, file_name: u16, qualifier: u8, section: u8) -> Vec<u8> {
    encode_information_object(ioa, &[&file_name.to_le_bytes(), &[qualifier], &[section]])
}

pub fn encode_segment(ioa: u32, file_name: u16, section: u8, data: &[u8]) -> Vec<u8> {
    assert!(data.len() <= u8::MAX as usize, "TI125 segment data exceeds LOS capacity");
    let length = data.len() as u8;
    encode_information_object(ioa, &[&file_name.to_le_bytes(), &[section], &[length], data])
}

pub fn encode_directory(ioa: u32, file_name: u16, length: u32, status: u8, timestamp: [u8; 7]) -> Vec<u8> {
    encode_information_object(ioa, &[&file_name.to_le_bytes(), &encode_u24(length), &[status], &timestamp])
}

pub fn encode_query_log(ioa: u32, file_name: u16, range_start_time: &[u8; 7], range_end_time: &[u8; 7]) -> Vec<u8> {
    encode_information_object(ioa, &[&file_name.to_le_bytes(), range_start_time, range_end_time])
}

#[inline]
pub fn encode_single_point(value: f64) -> Result<bool, MeasurementValueError> {
    match value {
        0.0 => Ok(false),
        1.0 => Ok(true),
        _ => Err(MeasurementValueError),
    }
}

#[inline]
pub fn encode_double_point(value: f64) -> Result<u8, MeasurementValueError> {
    if value.is_finite() && value.fract() == 0.0 && (0.0..=3.0).contains(&value) {
        Ok(value as u8)
    } else {
        Err(MeasurementValueError)
    }
}

#[inline]
pub fn encode_step_position(value: f64) -> Result<i8, MeasurementValueError> {
    if value.is_finite() && value.fract() == 0.0 && (-64.0..=63.0).contains(&value) {
        Ok(value as i8)
    } else {
        Err(MeasurementValueError)
    }
}

#[inline]
pub fn encode_bitstring32(value: f64) -> Result<u32, MeasurementValueError> {
    if value.is_finite() && value.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&value) {
        Ok(value as u32)
    } else {
        Err(MeasurementValueError)
    }
}

#[inline]
pub fn encode_normalized(value: f64) -> Result<i16, MeasurementValueError> {
    if !value.is_finite() || !(-1.0..1.0).contains(&value) {
        return Err(MeasurementValueError);
    }
    Ok((value * 32768.0).floor() as i16)
}

#[inline]
pub fn encode_scaled_i16(value: f64) -> Result<i16, MeasurementValueError> {
    if value.is_finite() && value.fract() == 0.0 && (i16::MIN as f64..=i16::MAX as f64).contains(&value) {
        Ok(value as i16)
    } else {
        Err(MeasurementValueError)
    }
}

#[inline]
pub fn encode_short_float(value: f64) -> Result<f32, MeasurementValueError> {
    if !value.is_finite() || value < f32::MIN as f64 || value > f32::MAX as f64 {
        return Err(MeasurementValueError);
    }
    let encoded = value as f32;
    encoded.is_finite().then_some(encoded).ok_or(MeasurementValueError)
}

#[inline]
pub fn encode_counter(value: f64) -> Result<i32, MeasurementValueError> {
    if value.is_finite() && value.fract() == 0.0 && (i32::MIN as f64..=i32::MAX as f64).contains(&value) {
        Ok(value as i32)
    } else {
        Err(MeasurementValueError)
    }
}

#[inline]
pub fn encode_siq(value: f64, quality: u8) -> Result<u8, MeasurementValueError> {
    Ok(quality | u8::from(encode_single_point(value)?))
}

#[inline]
pub fn encode_diq(value: f64, quality: u8) -> Result<u8, MeasurementValueError> {
    Ok(quality | (encode_double_point(value)? & 0x03))
}

#[inline]
pub fn encode_vti_qds(value: f64, transient: bool, quality: u8) -> Result<[u8; 2], MeasurementValueError> {
    let value = encode_step_position(value)? as u8 & 0x7F;
    Ok([value | if transient { 0x80 } else { 0 }, quality])
}

#[inline]
pub fn encode_bsi_qds(value: f64, quality: u8) -> Result<[u8; 5], MeasurementValueError> {
    let value = encode_bitstring32(value)?.to_le_bytes();
    Ok([value[0], value[1], value[2], value[3], quality])
}

#[inline]
pub fn encode_normalized_qds(value: f64, quality: u8) -> Result<[u8; 3], MeasurementValueError> {
    let value = encode_normalized(value)?.to_le_bytes();
    Ok([value[0], value[1], quality])
}

#[inline]
pub fn encode_scaled_qds(value: f64, quality: u8) -> Result<[u8; 3], MeasurementValueError> {
    let value = encode_scaled_i16(value)?.to_le_bytes();
    Ok([value[0], value[1], quality])
}

#[inline]
pub fn encode_short_float_qds(value: f64, quality: u8) -> Result<[u8; 5], MeasurementValueError> {
    let value = encode_short_float(value)?.to_le_bytes();
    Ok([value[0], value[1], value[2], value[3], quality])
}

#[inline]
pub fn encode_bcr(value: f64, quality: u8) -> Result<[u8; 5], MeasurementValueError> {
    let value = encode_counter(value)?.to_le_bytes();
    Ok([value[0], value[1], value[2], value[3], quality])
}

pub fn encode_measurement_object(
    type_id: u8,
    ioa: u32,
    value: f64,
    quality: u8,
    transient: bool,
    timestamp: Option<&[u8]>,
) -> Result<Vec<u8>, MeasurementValueError> {
    let mut payload = Vec::with_capacity(15);
    payload.extend_from_slice(&encode_ioa(ioa));
    match type_id {
        1 | 2 | 30 => payload.push(encode_siq(value, quality)?),
        3 | 4 | 31 => payload.push(encode_diq(value, quality)?),
        5 | 6 | 32 => payload.extend_from_slice(&encode_vti_qds(value, transient, quality)?),
        7 | 8 | 33 => payload.extend_from_slice(&encode_bsi_qds(value, quality)?),
        9 | 10 | 34 => payload.extend_from_slice(&encode_normalized_qds(value, quality)?),
        11 | 12 | 35 => payload.extend_from_slice(&encode_scaled_qds(value, quality)?),
        13 | 14 | 36 => payload.extend_from_slice(&encode_short_float_qds(value, quality)?),
        15 | 16 | 37 => payload.extend_from_slice(&encode_bcr(value, quality)?),
        21 => payload.extend_from_slice(&encode_normalized(value)?.to_le_bytes()),
        _ => return Err(MeasurementValueError),
    }

    let timestamp_len =
        lookup_type_descriptor(TypeId::new(type_id)).and_then(|descriptor| descriptor.timestamp_len).unwrap_or(0);
    match (timestamp_len, timestamp) {
        (0, None) => {}
        (expected, Some(raw)) if raw.len() == expected => payload.extend_from_slice(raw),
        _ => return Err(MeasurementValueError),
    }
    Ok(payload)
}
