// hot_loader.rs (côté hôte)
use anyhow::{bail, Result};
use libloading::{Library, Symbol};
use nockvm::jets::hot::HotEntry;
use std::{path::{Path, PathBuf}, slice, env}; // même type que dans la lib

type LenFn = unsafe extern "C" fn() -> usize;
type PtrFn = unsafe extern "C" fn() -> *const HotEntry;
type ApiFn = unsafe extern "C" fn() -> u32;

pub struct HotLibrary {
    _lib: Library,        // garde la lib vivante
    ptr: *const HotEntry, // pointeur vers la table dans la lib
    len: usize,
}

impl HotLibrary {
    /// Find the library in the binary directory
    pub fn find_library() -> Option<PathBuf> {
        // Try to get the current executable path
        if let Ok(exe_path) = env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let lib_path = exe_dir.join("libzkvm_jetpack.so");
                if lib_path.exists() {
                    return Some(lib_path);
                }
            }
        }

        // Fallback to current directory
        let current_dir_path = PathBuf::from("libzkvm_jetpack.so");
        if current_dir_path.exists() {
            Some(current_dir_path)
        } else {
            None
        }
    }

    /// Try to load library automatically (binary dir, then current dir)
    pub unsafe fn load_auto() -> Result<Self> {
        if let Some(path) = Self::find_library() {
            Self::load(path)
        } else {
            bail!("libzkvm_jetpack.so not found in binary directory or current directory")
        }
    }

    /// Charge la lib dynamique et récupère le slice des jets
    pub unsafe fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let lib = Library::new(path.as_ref())?;

        // Résout les symboles
        let api: Symbol<ApiFn> = lib.get(b"prover_hot_api_version")?;
        if api() != 1 {
            bail!("Hot API version mismatch");
        }

        let len_sym: Symbol<LenFn> = lib.get(b"prover_hot_state_len")?;
        let ptr_sym: Symbol<PtrFn> = lib.get(b"prover_hot_state_ptr")?;

        let len = len_sym();
        let ptr = ptr_sym();

        if ptr.is_null() || len == 0 {
            bail!("Empty or null hot state from dynamic library");
        }

        Ok(Self {
            _lib: lib,
            ptr,
            len,
        })
    }

    /// Vue sur la table des jets (vivante tant que `self` vit)
    pub fn jets(&self) -> &[HotEntry] {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}