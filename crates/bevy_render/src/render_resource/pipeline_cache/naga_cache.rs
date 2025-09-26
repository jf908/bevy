use std::sync::{LazyLock, Mutex};

use bevy_platform::collections::HashMap;
use naga_oil::compose::ShaderDefValue;

pub static NAGA_MODULE_CACHE: LazyLock<Mutex<HashMap<String, naga::Module>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn stringify_shader_defs(
    shader_defs: &std::collections::HashMap<String, ShaderDefValue>,
) -> String {
    use itertools::Itertools;

    shader_defs
        .iter()
        .sorted_by_key(|e| e.0)
        .map(|(k, v)| {
            format!(
                "{}={}",
                k,
                match v {
                    ShaderDefValue::Bool(val) => val.to_string(),
                    ShaderDefValue::Int(val) => val.to_string(),
                    ShaderDefValue::UInt(val) => val.to_string(),
                }
            )
        })
        .join(",")
}

pub fn read_cache<R: std::io::Read>(reader: &mut R) {
    let mut map = NAGA_MODULE_CACHE.try_lock().unwrap();
    *map = bincode::serde::decode_from_std_read(reader, bincode::config::standard()).unwrap();
}

#[cfg(feature = "naga_cache_writer")]
pub fn write_cache<W: std::io::Write>(writer: &mut W) {
    let map = NAGA_MODULE_CACHE.try_lock().unwrap();
    bincode::serde::encode_into_std_write(&*map, writer, bincode::config::standard()).unwrap();
}
