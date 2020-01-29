// Copyright (c) 2018 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

extern crate libzfs_sys as sys;

use libzfs_types::{LibZfsError, Result};
use nvpair;
use nvpair::ForeignType;
use std::ffi::{CStr, CString};
use std::io::Error;
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::sync::Mutex;
use zfs::Zfs;
use zpool::Zpool;

lazy_static! {
    pub static ref LOCK: Mutex<()> = Mutex::new(());
}

pub struct Libzfs {
    raw: *mut sys::libzfs_handle_t,
}

impl Default for Libzfs {
    fn default() -> Self {
        Libzfs::new()
    }
}

impl Libzfs {
    pub fn new() -> Libzfs {
        Libzfs {
            raw: unsafe { sys::libzfs_init() },
        }
    }
    pub fn pool_by_name(&mut self, name: &str) -> Option<Zpool> {
        unsafe {
            let pool_name = CString::new(name).unwrap();

            let pool = sys::zpool_open_canfail(self.raw, pool_name.as_ptr());

            if pool.is_null() {
                None
            } else {
                Some(Zpool::new(pool))
            }
        }
    }
    pub fn dataset_by_name(&mut self, name: &str) -> Option<Zfs> {
        unsafe {
            let x = CString::new(name).unwrap();
            let name = x.into_raw();

            let ds = sys::zfs_path_to_zhandle(self.raw, name, sys::zfs_type_dataset());
            let _ = CString::from_raw(name);

            if ds.is_null() {
                None
            } else {
                Some(Zfs::new(ds))
            }
        }
    }
    pub fn find_importable_pools(&mut self) -> nvpair::NvList {
        let _l = LOCK.lock().unwrap();
        unsafe {
            sys::thread_init();
            let mut args = sys::import_args();

            let x = sys::zpool_search_import(self.raw, &mut args as *mut sys::importargs);
            sys::thread_fini();

            nvpair::NvList::from_ptr(x)
        }
    }
    pub fn import_all<'a>(&mut self, nvl: &'a nvpair::NvList) -> std::result::Result<(), Vec<(&'a CStr, LibZfsError)>> {
        let mut errors = vec![];
        for pair in nvl.iter() {
            let name = pair.name();
            let nvl2 = match pair.value_nv_list() {
                Ok(nvl2) => nvl2,
                Err(e) => {
                    errors.push((name, e.into()));
                    continue;
                }
            };

            let code = unsafe {
                sys::zpool_import(
                    self.raw,
                    nvl2.as_ptr() as *mut _,
                    ptr::null(),
                    ptr::null_mut(),
                )
            };

            if code != 0 {
                errors.push((name, LibZfsError::Io(Error::from_raw_os_error(code))));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    pub fn export_all<'a>(&mut self, pools: &'a [Zpool]) -> std::result::Result<(), Vec<(&'a Zpool, LibZfsError)>> {
        let mut errors = vec![];
        for pool in pools {
            if let Err(e) = pool.disable_datasets().and_then(|_| pool.export()) {
                errors.push((pool, e));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    pub fn get_imported_pools(&mut self) -> Result<Vec<Zpool>> {
        unsafe extern "C" fn callback(
            handle: *mut sys::zpool_handle_t,
            state: *mut c_void,
        ) -> c_int {
            let state = &mut *(state as *mut Vec<Zpool>);

            state.push(Zpool::new(handle));

            0
        }
        let mut state: Vec<Zpool> = Vec::new();
        let state_ptr: *mut c_void = &mut state as *mut _ as *mut c_void;
        let code = unsafe { sys::zpool_iter(self.raw, Some(callback), state_ptr) };

        match code {
            0 => Ok(state),
            x => Err(LibZfsError::Io(Error::from_raw_os_error(x))),
        }
    }
}

impl Drop for Libzfs {
    fn drop(&mut self) {
        unsafe { sys::libzfs_fini(self.raw) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_close_handle() {
        Libzfs::new();
    }
}
