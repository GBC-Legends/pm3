use crate::models::pm3_config::PmProcessConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn verify_start_config(raw: &str) -> anyhow::Result<PmProcessConfig> {
    let raw = strip_wrappers(raw.trim());
    let params = parse_query(raw)?;

    let proc_name = get_required(&params, "proc_name")?;
    validate_proc_name(proc_name)?;

    let exec_dir = PathBuf::from(get_required(&params, "exec_dir")?);
    let exec_name = PathBuf::from(get_required(&params, "exec_name")?);

    validate_absolute_path("exec_dir", &exec_dir)?;
    validate_absolute_path("exec_name", &exec_name)?;

    let active = parse_bool01(get_required(&params, "active")?)?;

    let exec_args = params
        .get("args")
        .map(|s| split_args_simple(s))
        .unwrap_or_default();

    Ok(PmProcessConfig {
        proc_name: proc_name.to_string(),
        exec_dir,
        exec_name,
        exec_args,
        active,
        _extra: HashMap::new(),
    })
}

fn strip_wrappers(s: &str) -> &str {
    let mut out = s;

    // ["..."]
    if out.starts_with("[\"") && out.ends_with("\"]") && out.len() >= 4 {
        out = &out[2..out.len() - 2];
    }

    // "..."
    if out.starts_with('"') && out.ends_with('"') && out.len() >= 2 {
        out = &out[1..out.len() - 1];
    }

    out
}

fn get_required<'a>(m: &'a HashMap<String, String>, k: &str) -> anyhow::Result<&'a str> {
    m.get(k)
        .map(|s| s.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required param: {k}"))
}

fn validate_proc_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        anyhow::bail!("proc_name is empty");
    }
    if name.len() > 128 {
        anyhow::bail!("proc_name too long");
    }
    if name
        .chars()
        .any(|c| c.is_whitespace() || c == '/' || c == '\\')
    {
        anyhow::bail!("proc_name contains invalid characters");
    }
    Ok(())
}

fn validate_absolute_path(field: &str, p: &Path) -> anyhow::Result<()> {
    if !p.is_absolute() {
        anyhow::bail!("{field} must be absolute path: '{}'", p.display());
    }
    Ok(())
}

fn parse_bool01(s: &str) -> anyhow::Result<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => anyhow::bail!("active must be 0/1/true/false, got '{s}'"),
    }
}

fn split_args_simple(s: &str) -> Vec<String> {
    s.split_whitespace().map(|x| x.to_string()).collect()
}

fn parse_query(q: &str) -> anyhow::Result<HashMap<String, String>> {
    let mut map = HashMap::new();

    if q.trim().is_empty() {
        anyhow::bail!("empty query");
    }

    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k_raw, v_raw) = match pair.split_once('=') {
            Some(x) => x,
            None => (pair, ""),
        };

        let k = url_decode(k_raw)?;
        let v = url_decode(v_raw)?;

        if k.is_empty() {
            anyhow::bail!("empty key in query: '{pair}'");
        }
        if map.contains_key(&k) {
            anyhow::bail!("duplicate key '{k}'");
        }
        map.insert(k, v);
    }

    Ok(map)
}

fn url_decode(s: &str) -> anyhow::Result<String> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());

    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                if i + 2 >= bytes.len() {
                    anyhow::bail!("bad percent-encoding in '{s}'");
                }
                let hi = from_hex(bytes[i + 1])?;
                let lo = from_hex(bytes[i + 2])?;
                out.push((hi << 4) | lo);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }

    Ok(String::from_utf8(out)?)
}

fn from_hex(b: u8) -> anyhow::Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(10 + (b - b'a')),
        b'A'..=b'F' => Ok(10 + (b - b'A')),
        _ => anyhow::bail!("invalid hex digit '{}'", b as char),
    }
}
