//! ABI version 1 constants shared with hosts/web/vn-host.js and documented in hosts/web/ABI.md. Changing a value is an ABI
//! change: bump ABI_VERSION and both sides together.

use vn_runtime::host::{HostEvent, HostNode, ListenerOutcome};

pub const ABI_VERSION: u32 = 1;

/// Status codes returned by fallible imports (0 = Ok). The message of a failure is fetched with `host_error_message`.
pub const STATUS_OK: i32 = 0;
pub const STATUS_NOT_FOUND: i32 = 1;
pub const STATUS_UNSUPPORTED: i32 = 2;
pub const STATUS_INVALID_STATE: i32 = 3;

/// Namespace codes of `create_element`.
pub const NS_HTML: u32 = 0;
pub const NS_SVG: u32 = 1;
pub const NS_MATHML: u32 = 2;

/// Flags of `add_event_listener`.
pub const LISTENER_CAPTURE: u32 = 1;
pub const LISTENER_ONCE: u32 = 2;
pub const LISTENER_PASSIVE: u32 = 4;

/// Bits returned by the `vn_event` export (the glue applies them to the real event).
pub const OUTCOME_STOP_PROPAGATION: u32 = 1;
pub const OUTCOME_STOP_IMMEDIATE: u32 = 2;
pub const OUTCOME_PREVENT_DEFAULT: u32 = 4;
pub const OUTCOME_THREW: u32 = 8;

/// `kind` codes of `HostValue` crossing `get_property` (out struct) and `set_property` (arguments).
pub const VALUE_UNDEFINED: u32 = 0;
pub const VALUE_NULL: u32 = 1;
pub const VALUE_BOOL: u32 = 2;
pub const VALUE_NUMBER: u32 = 3;
pub const VALUE_STRING: u32 = 4;

/// Levels of `host_log`.
pub const LOG_DEBUG: u32 = 0;
pub const LOG_INFO: u32 = 1;
pub const LOG_WARN: u32 = 2;
pub const LOG_ERROR: u32 = 3;

/// Event payload of `vn_event`: a little-endian header of `EVENT_HEADER_LEN` bytes followed by UTF-8 string bytes. Offsets
/// inside the header are relative to the start of the payload.
pub const EVENT_HEADER_LEN: usize = 80;
pub const EVENT_FLAG_BUBBLES: u32 = 1 << 0;
pub const EVENT_FLAG_CANCELABLE: u32 = 1 << 1;
pub const EVENT_FLAG_COMPOSED: u32 = 1 << 2;
pub const EVENT_FLAG_DEFAULT_PREVENTED: u32 = 1 << 3;
pub const EVENT_FLAG_CTRL: u32 = 1 << 4;
pub const EVENT_FLAG_SHIFT: u32 = 1 << 5;
pub const EVENT_FLAG_ALT: u32 = 1 << 6;
pub const EVENT_FLAG_META: u32 = 1 << 7;
pub const EVENT_FLAG_IS_COMPOSING: u32 = 1 << 8;
pub const EVENT_FLAG_HAS_INPUT_DATA: u32 = 1 << 9;

/// Packs a listener outcome into the bits returned by `vn_event`.
pub fn outcome_bits(o: ListenerOutcome) -> u32 {
    let mut bits = 0;
    if o.stop_propagation {
        bits |= OUTCOME_STOP_PROPAGATION;
    }
    if o.stop_immediate {
        bits |= OUTCOME_STOP_IMMEDIATE;
    }
    if o.prevent_default {
        bits |= OUTCOME_PREVENT_DEFAULT;
    }
    if o.threw {
        bits |= OUTCOME_THREW;
    }
    bits
}

/// Field offsets of the event header (see hosts/web/ABI.md, "Event payload").
mod at {
    pub const TYPE: usize = 0; // u32 offset, u32 length
    pub const KEY: usize = 8;
    pub const CODE: usize = 16;
    pub const INPUT_DATA: usize = 24;
    pub const TIME_STAMP: usize = 32; // f64
    pub const CLIENT_X: usize = 40; // f64
    pub const CLIENT_Y: usize = 48; // f64
    pub const BUTTON: usize = 56; // i32
    pub const DETAIL: usize = 60; // i32
    pub const TARGET: usize = 64; // u32 handle, 0 = none
    pub const CURRENT_TARGET: usize = 68; // u32 handle, 0 = none
    pub const PHASE: usize = 72; // u32, DOM eventPhase 0..3
    pub const FLAGS: usize = 76; // u32 bit set (EVENT_FLAG_*)
}

/// Decodes an event payload written by the glue. Every offset and length is bounds-checked: a malformed payload is an
/// error, never a partial event.
pub fn decode_event(buf: &[u8]) -> Result<HostEvent, String> {
    if buf.len() < EVENT_HEADER_LEN {
        return Err(format!("payload is {} bytes, the header alone is {EVENT_HEADER_LEN}", buf.len()));
    }
    let flags = u32_at(buf, at::FLAGS)?;
    let phase = u32_at(buf, at::PHASE)?;
    if phase > 3 {
        return Err(format!("eventPhase {phase} is outside 0..=3"));
    }
    let node = |off: usize| match u32_at(buf, off) {
        Ok(0) => Ok(None),
        Ok(h) => Ok(Some(HostNode(h))),
        Err(e) => Err(e),
    };
    let flag = |bit: u32| flags & bit != 0;
    Ok(HostEvent {
        ty: str_at(buf, at::TYPE)?,
        target: node(at::TARGET)?,
        current_target: node(at::CURRENT_TARGET)?,
        phase: phase as u8,
        bubbles: flag(EVENT_FLAG_BUBBLES),
        cancelable: flag(EVENT_FLAG_CANCELABLE),
        composed: flag(EVENT_FLAG_COMPOSED),
        default_prevented: flag(EVENT_FLAG_DEFAULT_PREVENTED),
        time_stamp: f64_at(buf, at::TIME_STAMP)?,
        key: str_at(buf, at::KEY)?,
        code: str_at(buf, at::CODE)?,
        ctrl_key: flag(EVENT_FLAG_CTRL),
        shift_key: flag(EVENT_FLAG_SHIFT),
        alt_key: flag(EVENT_FLAG_ALT),
        meta_key: flag(EVENT_FLAG_META),
        button: i32_at(buf, at::BUTTON)?,
        client_x: f64_at(buf, at::CLIENT_X)?,
        client_y: f64_at(buf, at::CLIENT_Y)?,
        input_data: if flag(EVENT_FLAG_HAS_INPUT_DATA) { Some(str_at(buf, at::INPUT_DATA)?) } else { None },
        is_composing: flag(EVENT_FLAG_IS_COMPOSING),
        detail: i32_at(buf, at::DETAIL)?,
    })
}

fn field<const N: usize>(buf: &[u8], off: usize) -> Result<[u8; N], String> {
    buf.get(off..off + N)
        .and_then(|s| <[u8; N]>::try_from(s).ok())
        .ok_or_else(|| format!("field at byte {off} is outside the {}-byte payload", buf.len()))
}

fn u32_at(buf: &[u8], off: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(field::<4>(buf, off)?))
}

fn i32_at(buf: &[u8], off: usize) -> Result<i32, String> {
    Ok(i32::from_le_bytes(field::<4>(buf, off)?))
}

fn f64_at(buf: &[u8], off: usize) -> Result<f64, String> {
    Ok(f64::from_le_bytes(field::<8>(buf, off)?))
}

/// A string slot: `slot` holds the u32 offset and `slot + 4` the u32 length. Strings must lie after the header.
fn str_at(buf: &[u8], slot: usize) -> Result<String, String> {
    let start = u32_at(buf, slot)? as usize;
    let len = u32_at(buf, slot + 4)? as usize;
    let end = start.checked_add(len).ok_or("string range overflows")?;
    if start < EVENT_HEADER_LEN || end > buf.len() {
        return Err(format!("string at {start}+{len} is outside the string area of a {}-byte payload", buf.len()));
    }
    String::from_utf8(buf[start..end].to_vec()).map_err(|_| format!("string at {start} is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a payload by the layout table of hosts/web/ABI.md (written by hand here, independently of decode_event).
    fn payload(ty: &str, key: &str, flags: u32, target: u32, phase: u32, input: Option<&str>) -> Vec<u8> {
        let strings: Vec<&[u8]> = vec![ty.as_bytes(), key.as_bytes(), b"KeyE", input.unwrap_or("").as_bytes()];
        let mut offs = Vec::new();
        let mut at = EVENT_HEADER_LEN;
        for s in &strings {
            offs.push(at as u32);
            at += s.len();
        }
        let mut b = vec![0u8; EVENT_HEADER_LEN];
        let mut put = |off: usize, bytes: &[u8]| b[off..off + bytes.len()].copy_from_slice(bytes);
        for (i, s) in strings.iter().enumerate() {
            put(i * 8, &offs[i].to_le_bytes());
            put(i * 8 + 4, &(s.len() as u32).to_le_bytes());
        }
        put(32, &1234.5f64.to_le_bytes());
        put(40, &(-3.0f64).to_le_bytes());
        put(48, &7.25f64.to_le_bytes());
        put(56, &2i32.to_le_bytes());
        put(60, &1i32.to_le_bytes());
        put(64, &target.to_le_bytes());
        put(68, &0u32.to_le_bytes());
        put(72, &phase.to_le_bytes());
        put(76, &flags.to_le_bytes());
        for s in &strings {
            b.extend_from_slice(s);
        }
        b
    }

    #[test]
    fn decodes_every_field() {
        let flags = EVENT_FLAG_BUBBLES | EVENT_FLAG_CTRL | EVENT_FLAG_IS_COMPOSING | EVENT_FLAG_HAS_INPUT_DATA;
        let ev = decode_event(&payload("keydown", "é", flags, 9, 2, Some("x"))).unwrap();
        assert_eq!(ev.ty, "keydown");
        assert_eq!(ev.key, "é");
        assert_eq!(ev.code, "KeyE");
        assert_eq!(ev.target, Some(HostNode(9)));
        assert_eq!(ev.current_target, None);
        assert_eq!(ev.phase, 2);
        assert!(ev.bubbles && ev.ctrl_key && ev.is_composing && !ev.cancelable && !ev.shift_key);
        assert_eq!(ev.input_data.as_deref(), Some("x"));
        assert_eq!((ev.time_stamp, ev.client_x, ev.client_y), (1234.5, -3.0, 7.25));
        assert_eq!((ev.button, ev.detail), (2, 1));
    }

    #[test]
    fn input_data_is_absent_without_its_flag() {
        let ev = decode_event(&payload("click", "", 0, 0, 0, None)).unwrap();
        assert_eq!(ev.input_data, None);
        assert_eq!(ev.target, None);
    }

    #[test]
    fn rejects_truncated_and_inconsistent_payloads() {
        let good = payload("click", "", 0, 1, 1, None);
        assert!(decode_event(&good[..EVENT_HEADER_LEN - 1]).is_err());
        assert!(decode_event(&good[..good.len() - 1]).is_err(), "type string cut short");
        assert!(decode_event(&payload("click", "", 0, 1, 4, None)).unwrap_err().contains("eventPhase"));
        let mut bad_offset = good.clone();
        bad_offset[0..4].copy_from_slice(&3u32.to_le_bytes());
        assert!(decode_event(&bad_offset).is_err(), "string may not overlap the header");
    }

    #[test]
    fn outcome_bits_match_the_documented_values() {
        let o = ListenerOutcome { stop_propagation: true, stop_immediate: false, prevent_default: true, threw: true };
        assert_eq!(outcome_bits(o), 1 | 4 | 8);
        assert_eq!(outcome_bits(ListenerOutcome::default()), 0);
    }
}
