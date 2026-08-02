//! Sente 1.22.0 msgpack wire codec, implemented from the packer source
//! (taoensso/msgpack impl, extracted from the jar — see DESIGN.md ICD notes).
//!
//! Frame = msgpack of a 2-array `[payload, ?cb-uuid]`.
//! Extension types: 0 keyword (utf8, no colon, namespace kept), 1 symbol,
//! 2 char, 3 ratio, 4 set (nested msgpack array), 8 cached map key (1-byte
//! index into a per-frame insertion-ordered cache of keyword/string map keys),
//! 9 uuid (utf8), -1 instant (u32 nanos BE ++ i64 epoch-seconds BE),
//! 100 jinteki LocalDateTime (utf8 ISO).
//!
//! Decoding MUST maintain the key cache (the server emits ext-8 refs).
//! Encoding legally skips the cache: emitting full keys is always valid.

use rmpv::Value as Mp;
use serde_json::{json, Map, Value as Js};

pub struct Frame {
    pub payload: Js,
    pub cb_uuid: Option<String>,
}

pub fn decode_frame(bytes: &[u8]) -> Result<Frame, String> {
    let mut cur = bytes;
    let v = rmpv::decode::read_value(&mut cur).map_err(|e| format!("msgpack: {e}"))?;
    let Mp::Array(mut parts) = v else {
        return Err("frame is not an array".into());
    };
    if parts.len() != 2 {
        return Err(format!("frame arity {} != 2", parts.len()));
    }
    let cb = parts.pop().unwrap();
    let payload = parts.pop().unwrap();
    let cb_uuid = match cb {
        Mp::Nil => None,
        Mp::String(s) => s.as_str().map(|s| s.to_string()),
        _ => None,
    };
    let mut cache: Vec<String> = Vec::new();
    Ok(Frame {
        payload: mp_to_json(&payload, &mut cache),
        cb_uuid,
    })
}

fn key_to_string(k: &Mp, cache: &mut Vec<String>) -> String {
    match k {
        Mp::Ext(8, data) => {
            let idx = data.first().copied().unwrap_or(0) as usize;
            cache.get(idx).cloned().unwrap_or_default()
        }
        Mp::Ext(0, data) | Mp::Ext(1, data) => {
            let s = String::from_utf8_lossy(data).to_string();
            cache.push(s.clone());
            s
        }
        Mp::String(s) => {
            let s = s.as_str().unwrap_or_default().to_string();
            cache.push(s.clone());
            s
        }
        other => format!("{other}"),
    }
}

fn mp_to_json(v: &Mp, cache: &mut Vec<String>) -> Js {
    match v {
        Mp::Nil => Js::Null,
        Mp::Boolean(b) => json!(b),
        Mp::Integer(i) => {
            if let Some(n) = i.as_i64() {
                json!(n)
            } else if let Some(n) = i.as_u64() {
                json!(n)
            } else {
                Js::Null
            }
        }
        Mp::F32(f) => json!(f),
        Mp::F64(f) => json!(f),
        Mp::String(s) => json!(s.as_str().unwrap_or_default()),
        Mp::Binary(_) => Js::Null,
        Mp::Array(items) => Js::Array(items.iter().map(|i| mp_to_json(i, cache)).collect()),
        Mp::Map(pairs) => {
            let mut m = Map::new();
            for (k, val) in pairs {
                let key = key_to_string(k, cache);
                m.insert(key, mp_to_json(val, cache));
            }
            Js::Object(m)
        }
        Mp::Ext(0, data) | Mp::Ext(1, data) | Mp::Ext(2, data) | Mp::Ext(9, data)
        | Mp::Ext(100, data) => {
            json!(String::from_utf8_lossy(data).to_string())
        }
        Mp::Ext(4, data) => {
            // Set: payload is a complete nested msgpack array.
            let mut cur = data.as_slice();
            match rmpv::decode::read_value(&mut cur) {
                Ok(inner) => mp_to_json(&inner, cache),
                Err(_) => Js::Null,
            }
        }
        Mp::Ext(8, data) => {
            let idx = data.first().copied().unwrap_or(0) as usize;
            json!(cache.get(idx).cloned().unwrap_or_default())
        }
        Mp::Ext(-1, data) if data.len() == 12 => {
            let secs = i64::from_be_bytes(data[4..12].try_into().unwrap());
            json!(format!("inst:{secs}"))
        }
        Mp::Ext(_, _) => Js::Null,
    }
}

// ── encoding ───────────────────────────────────────────────────────────────

pub fn kw(name: &str) -> Mp {
    Mp::Ext(0, name.as_bytes().to_vec())
}

pub fn uuid(s: &str) -> Mp {
    Mp::Ext(9, s.as_bytes().to_vec())
}

/// JSON → msgpack with keyword map keys (what Clojure destructuring expects).
pub fn json_to_mp(v: &Js) -> Mp {
    match v {
        Js::Null => Mp::Nil,
        Js::Bool(b) => Mp::Boolean(*b),
        Js::Number(n) => {
            if let Some(i) = n.as_i64() {
                Mp::from(i)
            } else if let Some(u) = n.as_u64() {
                Mp::from(u)
            } else {
                Mp::F64(n.as_f64().unwrap_or(0.0))
            }
        }
        Js::String(s) => Mp::String(s.clone().into()),
        Js::Array(items) => Mp::Array(items.iter().map(json_to_mp).collect()),
        Js::Object(m) => Mp::Map(
            m.iter()
                .map(|(k, val)| (kw(k), json_to_mp(val)))
                .collect(),
        ),
    }
}

/// Encode one client→server frame: `[[:ev-id ?payload] ?cb-uuid]`.
pub fn encode_event(ev_id: &str, payload: Option<Mp>, cb_uuid: Option<&str>) -> Vec<u8> {
    let mut ev = vec![kw(ev_id)];
    if let Some(p) = payload {
        ev.push(p);
    }
    let frame = Mp::Array(vec![
        Mp::Array(ev),
        match cb_uuid {
            Some(u) => Mp::String(u.to_string().into()),
            None => Mp::Nil,
        },
    ]);
    let mut out = Vec::new();
    rmpv::encode::write_value(&mut out, &frame).expect("msgpack encode");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_event_with_keyword_keys() {
        let bytes = encode_event(
            "game/action",
            Some(json_to_mp(&json!({"command": "credit", "args": {}}))),
            None,
        );
        let f = decode_frame(&bytes).unwrap();
        assert_eq!(f.payload[0], json!("game/action"));
        assert_eq!(f.payload[1]["command"], json!("credit"));
        assert!(f.cb_uuid.is_none());
    }

    #[test]
    fn decodes_key_cache_refs() {
        // Map {:a 1} twice: second key arrives as ext-8 cache index 0.
        let inner = Mp::Array(vec![
            Mp::Map(vec![(kw("a"), Mp::from(1))]),
            Mp::Map(vec![(Mp::Ext(8, vec![0]), Mp::from(2))]),
        ]);
        let frame = Mp::Array(vec![inner, Mp::Nil]);
        let mut out = Vec::new();
        rmpv::encode::write_value(&mut out, &frame).unwrap();
        let f = decode_frame(&out).unwrap();
        assert_eq!(f.payload[0]["a"], json!(1));
        assert_eq!(f.payload[1]["a"], json!(2));
    }

    #[test]
    fn decodes_bare_ping_keyword() {
        let frame = Mp::Array(vec![kw("chsk/ws-ping"), Mp::Nil]);
        let mut out = Vec::new();
        rmpv::encode::write_value(&mut out, &frame).unwrap();
        let f = decode_frame(&out).unwrap();
        assert_eq!(f.payload, json!("chsk/ws-ping"));
    }
}
