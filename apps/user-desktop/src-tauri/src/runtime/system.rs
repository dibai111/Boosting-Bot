use super::UserRuntime;
use crate::app_state::CommandResult;

impl UserRuntime {
    pub(crate) async fn app_memory_bytes(&self) -> CommandResult<u64> {
        process_tree_private_working_set(std::process::id()).map_err(|error| error.to_string())
    }
}

#[cfg(windows)]
fn process_tree_private_working_set(root_process_id: u32) -> std::io::Result<u64> {
    let root_working_set = process_private_working_set(root_process_id)?;
    let process_pairs = process_parent_pairs().unwrap_or_default();

    Ok(descendant_process_ids(root_process_id, &process_pairs)
        .into_iter()
        .filter_map(|process_id| process_private_working_set(process_id).ok())
        .fold(root_working_set, u64::saturating_add))
}

#[cfg(windows)]
fn process_private_working_set(process_id: u32) -> std::io::Result<u64> {
    use std::mem::size_of;
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            ProcessStatus::{
                GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX2,
            },
            Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
        },
    };

    // SAFETY: OS supplies the process id and the null handle is checked before use.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let counter_size = u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX2>()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "PROCESS_MEMORY_COUNTERS_EX2 is too large",
        )
    })?;
    let mut counters = PROCESS_MEMORY_COUNTERS_EX2 {
        cb: counter_size,
        ..Default::default()
    };
    // SAFETY: process is valid and the base counters pointer refers to the full EX2 buffer.
    let succeeded = unsafe {
        GetProcessMemoryInfo(
            process,
            (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX2).cast::<PROCESS_MEMORY_COUNTERS>(),
            counter_size,
        )
    };
    // SAFETY: process is the handle returned by OpenProcess and is closed exactly once.
    unsafe { CloseHandle(process) };
    if succeeded == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(counters.PrivateWorkingSetSize as u64)
}

#[cfg(windows)]
fn process_parent_pairs() -> std::io::Result<Vec<(u32, u32)>> {
    use std::mem::size_of;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
    };

    // SAFETY: the snapshot is a process list owned by Windows. The returned handle is checked
    // before it is used and closed exactly once below.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }

    let entry_size = u32::try_from(size_of::<PROCESSENTRY32W>()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "PROCESSENTRY32W is too large",
        )
    })?;
    let mut entry = PROCESSENTRY32W {
        dwSize: entry_size,
        ..Default::default()
    };
    let mut pairs = Vec::new();

    // SAFETY: snapshot is valid and entry points to writable PROCESSENTRY32W storage.
    let has_entry = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    if has_entry {
        loop {
            pairs.push((entry.th32ProcessID, entry.th32ParentProcessID));
            entry.dwSize = entry_size;
            // SAFETY: snapshot remains valid until it is closed and entry is writable storage.
            if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                break;
            }
        }
    }

    // SAFETY: snapshot is the handle returned above and is closed exactly once.
    unsafe { CloseHandle(snapshot) };
    if has_entry {
        Ok(pairs)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn descendant_process_ids(root_process_id: u32, process_pairs: &[(u32, u32)]) -> Vec<u32> {
    use std::collections::{HashMap, HashSet};

    let mut children_by_parent = HashMap::<u32, Vec<u32>>::new();
    for &(process_id, parent_process_id) in process_pairs {
        if process_id != 0 {
            children_by_parent
                .entry(parent_process_id)
                .or_default()
                .push(process_id);
        }
    }

    let mut seen = HashSet::from([root_process_id]);
    let mut pending = vec![root_process_id];
    let mut descendants = Vec::new();
    while let Some(parent_process_id) = pending.pop() {
        let Some(children) = children_by_parent.get(&parent_process_id) else {
            continue;
        };
        for &child_process_id in children {
            if seen.insert(child_process_id) {
                descendants.push(child_process_id);
                pending.push(child_process_id);
            }
        }
    }
    descendants
}

#[cfg(not(windows))]
fn process_tree_private_working_set(_process_id: u32) -> std::io::Result<u64> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "app memory is only available on Windows",
    ))
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn reads_current_process_tree_private_working_set() {
        assert!(super::process_tree_private_working_set(std::process::id()).unwrap() > 0);
    }

    #[test]
    fn finds_each_descendant_once() {
        let mut descendants =
            super::descendant_process_ids(1, &[(2, 1), (3, 2), (4, 1), (1, 3), (4, 1)]);
        descendants.sort_unstable();

        assert_eq!(descendants, vec![2, 3, 4]);
    }
}
