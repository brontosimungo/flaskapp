// hot_loader.rs (côté hôte)
use anyhow::{bail, Result};
use libloading::{Library, Symbol};
use nockvm::jets::hot::HotEntry;
use std::{path::Path, slice}; // même type que dans la lib

type LenFn = unsafe extern "C" fn() -> usize;
type PtrFn = unsafe extern "C" fn() -> *const HotEntry;
type ApiFn = unsafe extern "C" fn() -> u32;

pub struct HotLibrary {
    _lib: Library,        // garde la lib vivante
    ptr: *const HotEntry, // pointeur vers la table dans la lib
    len: usize,
}

impl HotLibrary {
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
