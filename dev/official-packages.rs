//! Trusted development producer. Package bytes are data, never an execution grant.
#[cfg(windows)]
mod producer {
    use diy_ime2_nodes::node::{NodeDefinition, Registry};
    use serde_json::{Value, json};
    use std::{
        collections::BTreeSet,
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        os::windows::ffi::OsStrExt,
        path::{Path, PathBuf},
    };
    use windows_sys::Win32::{Security::Cryptography::*, Storage::FileSystem::MoveFileW};
    const MAX_MODULE: usize = 64 * 1024 * 1024;
    const MAX_MANIFEST: usize = 1024 * 1024;
    const MAX_TOTAL: usize = 512 * 1024 * 1024;
    const LOCK: &[u8] = include_bytes!("../Cargo.lock");
    const PROVENANCE: &[u8] = include_bytes!("../migration-provenance.json");
    const INVENTORY: &[u8] = include_bytes!("../docs/legacy-node-inventory.json");
    fn invalid(message: &str) -> String {
        message.into()
    }
    pub fn digest(bytes: &[u8]) -> Result<String, String> {
        struct Algorithm(BCRYPT_ALG_HANDLE);
        impl Drop for Algorithm {
            fn drop(&mut self) {
                unsafe {
                    BCryptCloseAlgorithmProvider(self.0, 0);
                }
            }
        }
        let name: Vec<u16> = "SHA256\0".encode_utf16().collect();
        let mut provider = std::ptr::null_mut();
        if unsafe { BCryptOpenAlgorithmProvider(&mut provider, name.as_ptr(), std::ptr::null(), 0) }
            < 0
        {
            return Err(invalid("SHA256 provider unavailable"));
        }
        let provider = Algorithm(provider);
        let length =
            u32::try_from(bytes.len()).map_err(|_| invalid("digest input exceeds limit"))?;
        let mut result = [0u8; 32];
        if unsafe {
            BCryptHash(
                provider.0,
                std::ptr::null(),
                0,
                bytes.as_ptr(),
                length,
                result.as_mut_ptr(),
                32,
            )
        } < 0
        {
            return Err(invalid("SHA256 failed"));
        }
        Ok(result.iter().map(|b| format!("{b:02x}")).collect())
    }
    fn hex(value: &str, length: usize) -> bool {
        value.len() == length
            && value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    }
    fn identifier(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 128
            && !value.starts_with('.')
            && !value.ends_with('.')
            && value
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(&c))
    }
    fn output_name(name: &std::ffi::OsStr) -> bool {
        let Some(name) = name.to_str() else {
            return false;
        };
        if name.is_empty()
            || name.len() > 256
            || name.ends_with('.')
            || name.ends_with(' ')
            || name
                .chars()
                .any(|c| c.is_control() || "\\/:*?\"<>|".contains(c))
        {
            return false;
        }
        let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
        !matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) && !((stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(
                &stem[3..],
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            ))
    }
    fn encode(value: &impl serde::Serialize) -> Result<Vec<u8>, String> {
        serde_json::to_vec(value).map_err(|_| invalid("metadata encoding failed"))
    }
    struct Item {
        file: String,
        manifest: Vec<u8>,
        info: Value,
    }
    fn items(module: &[u8], expected: &str, commit: &str) -> Result<(Vec<Item>, Value), String> {
        if !hex(expected, 64)
            || !hex(commit, 40)
            || module.len() > MAX_MODULE
            || !module.starts_with(b"\0asm\x01\0\0\0")
            || digest(module)? != expected
        {
            return Err(invalid(
                "invalid module, expected digest or source identity",
            ));
        }
        let baseline: Value =
            serde_json::from_slice(INVENTORY).map_err(|_| invalid("invalid compiled inventory"))?;
        let known = baseline["nodes"]
            .as_array()
            .ok_or_else(|| invalid("invalid compiled inventory"))?;
        let expected_kinds: BTreeSet<_> = known
            .iter()
            .filter(|n| n["registered"] == true && n["classification"] != "system")
            .map(|n| n["definition"]["kind"].as_str().unwrap_or("").to_owned())
            .collect();
        if expected_kinds.len() != 48 {
            return Err(invalid("official registered inventory differs"));
        }
        let mut registry = Registry::new();
        diy_ime2_nodes::register_nodes(&mut registry)
            .map_err(|_| invalid("official registry unavailable"))?;
        let definitions: Vec<NodeDefinition> = registry.definitions().cloned().collect();
        if definitions.len() != 48
            || definitions
                .iter()
                .map(|d| d.kind.clone())
                .collect::<BTreeSet<_>>()
                != expected_kinds
        {
            return Err(invalid(
                "official registry differs from registered inventory",
            ));
        }
        let mut result = Vec::new();
        let mut total = 0usize;
        for definition in definitions {
            let kind = &definition.kind;
            if !identifier(kind)
                || matches!(
                    kind.as_str(),
                    "input.text" | "demo.packet" | "e2e.input" | "e2e.output"
                )
            {
                return Err(invalid("invalid official processing kind"));
            }
            let entry = known
                .iter()
                .find(|n| n["definition"]["kind"] == *kind)
                .ok_or_else(|| invalid("missing inventory definition"))?;
            if serde_json::to_value(&definition)
                .map_err(|_| invalid("definition encoding failed"))?
                != entry["definition"]
            {
                return Err(invalid(
                    "registered definition differs from frozen inventory",
                ));
            }
            let class = entry["classification"]
                .as_str()
                .ok_or_else(|| invalid("classification missing"))?;
            let status = entry["status"]
                .as_str()
                .ok_or_else(|| invalid("status missing"))?;
            let package_id = format!("official.{kind}");
            let capabilities = if kind == "text.dictionary-exact" {
                json!([{"id":"dictionary.read","binding":"dictionary"}])
            } else {
                json!([])
            };
            let manifest = encode(
                &json!({"packageId":package_id,"version":format!("{}+g{commit}",env!("CARGO_PKG_VERSION")),"displayName":kind,"runtime":"wasm32","guestApiVersion":1,"node":definition,"moduleSha256":expected,"capabilities":capabilities}),
            )?;
            if manifest.len() > MAX_MANIFEST {
                return Err(invalid("official manifest exceeds limit"));
            }
            let bytes = frame(&manifest, module)?;
            total = total
                .checked_add(bytes.len())
                .ok_or_else(|| invalid("aggregate length overflow"))?;
            if total > MAX_TOTAL {
                return Err(invalid("official distribution exceeds aggregate limit"));
            }
            let file = format!("{kind}.diynode");
            let info = json!({"file":file,"packageId":package_id,"sha256":digest(&bytes)?,"bytes":bytes.len(),"kind":kind,"nodeVersion":definition.version,"classification":class,"status":status});
            result.push(Item {
                file,
                manifest,
                info,
            });
        }
        result.sort_by(|a, b| a.file.cmp(&b.file));
        let index = json!({"formatVersion":1,"source":{"repository":"pitapa79/DIY_IME_node","commit":commit,"cargoLockSha256":digest(LOCK)?,"migrationProvenanceSha256":digest(PROVENANCE)?,"inventorySha256":digest(INVENTORY)?},"module":{"sha256":expected,"bytes":module.len(),"guestApiVersion":1},"packageBytes":total,"packages":result.iter().map(|i|&i.info).collect::<Vec<_>>()});
        Ok((result, index))
    }
    fn frame(manifest: &[u8], module: &[u8]) -> Result<Vec<u8>, String> {
        let length =
            u32::try_from(manifest.len()).map_err(|_| invalid("manifest length overflow"))?;
        let mut bytes = Vec::with_capacity(24 + manifest.len() + module.len());
        bytes.extend_from_slice(b"DIY2NODE");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(&(module.len() as u64).to_le_bytes());
        bytes.extend_from_slice(manifest);
        bytes.extend_from_slice(module);
        Ok(bytes)
    }
    struct Staged {
        path: PathBuf,
        files: Vec<PathBuf>,
        published: bool,
    }
    impl Staged {
        fn new(parent: &Path) -> Result<Self, String> {
            let mut random = [0u8; 16];
            if unsafe {
                BCryptGenRandom(
                    std::ptr::null_mut(),
                    random.as_mut_ptr(),
                    16,
                    BCRYPT_USE_SYSTEM_PREFERRED_RNG,
                )
            } < 0
            {
                return Err(invalid("staging nonce unavailable"));
            }
            let name: String = random.iter().map(|b| format!("{b:02x}")).collect();
            let path = parent.join(format!(".diyime-official-{name}"));
            if path.parent() != Some(parent) {
                return Err(invalid("invalid staging containment"));
            }
            fs::create_dir(&path).map_err(|_| invalid("could not create staging directory"))?;
            Ok(Self {
                path,
                files: Vec::new(),
                published: false,
            })
        }
        fn write(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
            let path = self.path.join(name);
            if path.parent() != Some(self.path.as_path()) {
                return Err(invalid("invalid output containment"));
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|_| invalid("could not create staged file"))?;
            self.files.push(path);
            file.write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| invalid("could not write complete staged file"))
        }
        fn publish(&mut self, parent: &Path, target: &Path) -> Result<(), String> {
            if self.path.parent() != Some(parent)
                || target.parent() != Some(parent)
                || self.path == target
            {
                return Err(invalid("invalid publication containment"));
            }
            let old: Vec<u16> = self.path.as_os_str().encode_wide().chain(Some(0)).collect();
            let new: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
            // MoveFileW never replaces an existing destination. All files are synced.
            if unsafe { MoveFileW(old.as_ptr(), new.as_ptr()) } == 0 {
                return Err(invalid(
                    "publication refused; output exists or storage unavailable",
                ));
            }
            self.published = true;
            Ok(())
        }
    }
    impl Drop for Staged {
        fn drop(&mut self) {
            if self.published {
                return;
            }
            for path in &self.files {
                if path.parent() == Some(self.path.as_path()) {
                    let _ = fs::remove_file(path);
                }
            }
            let _ = fs::remove_dir(&self.path);
        }
    }
    pub fn produce(
        module: &[u8],
        expected: &str,
        commit: &str,
        output: &Path,
    ) -> Result<Value, String> {
        let (items, index) = items(module, expected, commit)?;
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = parent
            .canonicalize()
            .map_err(|_| invalid("output parent unavailable"))?;
        let name = output
            .file_name()
            .filter(|n| output_name(n))
            .ok_or_else(|| invalid("invalid output name"))?;
        let target = parent.join(name);
        if target.exists() || fs::symlink_metadata(&target).is_ok() {
            return Err(invalid("output already exists; refusing overwrite"));
        }
        let mut staged = Staged::new(&parent)?;
        for item in items {
            staged.write(&item.file, &frame(&item.manifest, module)?)?;
        }
        staged.write("official-index.json", &encode(&index)?)?;
        staged.publish(&parent, &target)?;
        Ok(index)
    }
    pub fn run() -> Result<(), String> {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if args.len() != 4 {
            return Err(invalid(
                "usage: official_packages MODULE EXPECTED_SHA256 SOURCE_COMMIT NEW_OUTPUT_DIRECTORY",
            ));
        }
        let expected = args[1]
            .to_str()
            .ok_or_else(|| invalid("invalid digest argument"))?;
        let commit = args[2]
            .to_str()
            .ok_or_else(|| invalid("invalid source argument"))?;
        let mut module = Vec::new();
        File::open(&args[0])
            .and_then(|f| f.take(MAX_MODULE as u64 + 1).read_to_end(&mut module))
            .map_err(|_| invalid("module unavailable"))?;
        let index = produce(&module, expected, commit, Path::new(&args[3]))?;
        println!(
            "{}",
            json!({"packages":index["packages"].as_array().map(Vec::len),"moduleSha256":expected,"sourceCommit":commit,"indexSha256":digest(&encode(&index)?)?})
        );
        Ok(())
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn sha256_matches_known_vector_and_frames_match_the_published_format() {
            assert_eq!(
                digest(b"abc").unwrap(),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            );
            let manifest = b"{}";
            let module = b"\0asm\x01\0\0\0";
            let bytes = frame(manifest, module).unwrap();
            assert_eq!(&bytes[..8], b"DIY2NODE");
            assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1);
            assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 2);
            assert_eq!(u64::from_le_bytes(bytes[16..24].try_into().unwrap()), 8);
            assert_eq!(&bytes[24..26], manifest);
            assert_eq!(&bytes[26..], module);
        }
        #[test]
        fn packages_are_deterministic_registered_only_and_have_exact_dictionary_requests() {
            let module = b"\0asm\x01\0\0\0";
            let hash = digest(module).unwrap();
            let commit = "a".repeat(40);
            let (a, index) = items(module, &hash, &commit).unwrap();
            let (b, other) = items(module, &hash, &commit).unwrap();
            assert_eq!(index, other);
            assert_eq!(a.len(), 48);
            let mut dictionary = 0;
            for (one, two) in a.iter().zip(&b) {
                assert_eq!(
                    frame(&one.manifest, module).unwrap(),
                    frame(&two.manifest, module).unwrap()
                );
                let manifest: Value = serde_json::from_slice(&one.manifest).unwrap();
                assert!(manifest["node"]["kind"] != "input.text");
                if manifest["node"]["kind"] == "text.dictionary-exact" {
                    dictionary += 1;
                    assert_eq!(
                        manifest["capabilities"],
                        json!([{"id":"dictionary.read","binding":"dictionary"}])
                    )
                } else {
                    assert_eq!(manifest["capabilities"], json!([]));
                }
            }
            assert_eq!(dictionary, 1);
            for (module, hash, commit) in [
                (b"bad".as_slice(), hash.clone(), commit.clone()),
                (module, "0".repeat(64), commit.clone()),
                (module, hash.clone(), "not-source".into()),
            ] {
                assert!(items(module, &hash, &commit).is_err());
            }
        }
        #[test]
        fn completed_directory_is_published_once_and_existing_files_are_preserved() {
            let parent = std::env::temp_dir().canonicalize().unwrap();
            let mut owner = Staged::new(&parent).unwrap();
            let output = owner.path.join("packages");
            let module = b"\0asm\x01\0\0\0";
            let hash = digest(module).unwrap();
            let commit = "b".repeat(40);
            let index = produce(module, &hash, &commit, &output).unwrap();
            let before = fs::read(output.join("official-index.json")).unwrap();
            assert!(produce(module, &hash, &commit, &output).is_err());
            assert_eq!(
                before,
                fs::read(output.join("official-index.json")).unwrap()
            );
            for package in index["packages"].as_array().unwrap() {
                let path = output.join(package["file"].as_str().unwrap());
                let bytes = fs::read(&path).unwrap();
                assert_eq!(digest(&bytes).unwrap(), package["sha256"]);
                assert_eq!(bytes.len() as u64, package["bytes"].as_u64().unwrap());
                fs::remove_file(&path).unwrap();
            }
            fs::remove_file(output.join("official-index.json")).unwrap();
            fs::remove_dir(output).unwrap();
            owner.files.clear();
        }
    }
}
#[cfg(windows)]
fn main() {
    if let Err(error) = producer::run() {
        eprintln!("official-packages: {error}");
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("official-packages requires the supported Windows development environment");
    std::process::exit(1);
}
