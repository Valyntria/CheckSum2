use crate::{hash::normalize_hash, Algorithm, Error, FileEntry};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

pub fn relative_name(root: &Path, path: &Path) -> Result<String, Error> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| Error::invalid("File is outside the selected folder"))?;
    let parts = relative
        .components()
        .map(|c| {
            c.as_os_str()
                .to_str()
                .ok_or_else(|| Error::invalid("Non-Unicode filename"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(parts.join("/"))
}

pub fn validate_name(name: &str) -> Result<(), Error> {
    if name.is_empty()
        || name.contains('\0')
        || name.starts_with(['/', '\\'])
        || name.as_bytes().get(1) == Some(&b':')
        || name.split(['/', '\\']).any(|c| c == "..")
        || Path::new(name)
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(Error::invalid(format!("Unsafe manifest path: {name:?}")));
    }
    if !Path::new(name)
        .components()
        .any(|c| matches!(c, Component::Normal(_)))
    {
        return Err(Error::invalid("Manifest path must name a file"));
    }
    Ok(())
}

// GNU checksum escaping: a leading backslash marks escaped filenames.
fn decode_name(name: &str) -> Result<String, Error> {
    let mut result = String::new();
    let mut chars = name.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            result.push(match chars.next() {
                Some('\\') => '\\',
                Some('n') => '\n',
                Some('r') => '\r',
                _ => return Err(Error::invalid("Invalid escape in checksum filename")),
            });
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

pub fn parse(text: &str, algorithm: Algorithm) -> Result<BTreeMap<String, String>, Error> {
    let mut entries = BTreeMap::new();
    for (i, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (escaped, line) = if let Some(rest) = line.strip_prefix('\\') {
            (true, rest)
        } else {
            (false, line)
        };
        let width = algorithm.hex_len();
        if line.len() < width + 3
            || !line.is_char_boundary(width)
            || line.as_bytes()[width] != b' '
            || !matches!(line.as_bytes()[width + 1], b' ' | b'*')
        {
            return Err(Error::invalid(format!(
                "Malformed {} checksum on line {}",
                algorithm.label(),
                i + 1
            )));
        }
        let hash = normalize_hash(&line[..width], algorithm)?;
        let name = if escaped {
            decode_name(&line[width + 2..])?
        } else {
            line[width + 2..].to_owned()
        };
        validate_name(&name)?;
        // Normalize harmless './' components so duplicates cannot hide behind aliases.
        let canonical_name = Path::new(&name)
            .components()
            .filter_map(|c| match c {
                Component::Normal(v) => v.to_str(),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/");
        if entries.insert(canonical_name.clone(), hash).is_some() {
            return Err(Error::invalid(format!(
                "Duplicate manifest path: {canonical_name}"
            )));
        }
    }
    if entries.is_empty() {
        return Err(Error::invalid("The manifest contains no checksums"));
    }
    Ok(entries)
}

pub fn read(path: &Path, algorithm: Algorithm) -> Result<BTreeMap<String, String>, Error> {
    use std::io::Read;
    const MAX: u64 = 16 * 1024 * 1024;
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(MAX + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX {
        return Err(Error::invalid("Manifest exceeds the 16 MiB limit"));
    }
    parse(&text, algorithm)
}

// Reject symlink components, including a final symlink, before reading a manifest target.
pub fn target(root: &Path, relative: &str) -> Result<PathBuf, Error> {
    validate_name(relative)?;
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        path.push(component.as_os_str());
        match std::fs::symlink_metadata(&path) {
            Ok(md) if md.file_type().is_symlink() => {
                return Err(Error::invalid("Manifest target contains a symbolic link"))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    if let Ok(resolved) = path.canonicalize() {
        if !resolved.starts_with(root) {
            return Err(Error::invalid(
                "Manifest target escapes the selected folder",
            ));
        }
    }
    Ok(path)
}

pub fn encode(entries: &[FileEntry], algorithm: Algorithm) -> Result<String, Error> {
    let mut sorted: Vec<_> = entries.iter().collect();
    sorted.sort_by_key(|e| &e.relative_path);
    let mut output = String::new();
    for entry in sorted {
        validate_name(&entry.relative_path)?;
        let hash = entry.hashes.get(&algorithm).ok_or_else(|| {
            Error::invalid(format!(
                "Missing {} checksum for {}",
                algorithm.label(),
                entry.relative_path
            ))
        })?;
        let hash = normalize_hash(hash, algorithm)?;
        let name = &entry.relative_path;
        let escaped = name.contains(['\\', '\n', '\r']);
        if escaped {
            output.push('\\');
        }
        output.push_str(&hash);
        output.push_str(" *");
        if escaped {
            output.push_str(
                &name
                    .replace('\\', "\\\\")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r"),
            );
        } else {
            output.push_str(name);
        }
        output.push('\n');
    }
    Ok(output)
}
