use crate::checkers::CheckResult;
use crate::config::CheckConfig;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::ProcessStatus::EnumProcesses;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};

/// Get list of all running process names
fn get_running_processes() -> Result<Vec<String>, String> {
    unsafe {
        // Get list of process IDs
        let mut pids: [u32; 2048] = [0; 2048];
        let mut bytes_returned: u32 = 0;

        let result = EnumProcesses(
            pids.as_mut_ptr(),
            std::mem::size_of_val(&pids) as u32,
            &mut bytes_returned,
        );

        if result.is_err() {
            return Err("Failed to enumerate processes".to_string());
        }

        let count = bytes_returned as usize / std::mem::size_of::<u32>();
        let mut process_names = Vec::new();

        for &pid in &pids[..count] {
            if pid == 0 {
                continue;
            }

            // Try to open the process
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);

            if let Ok(handle) = handle {
                if handle != HANDLE::default() {
                    // Get the full process image path using QueryFullProcessImageNameW
                    // This works with PROCESS_QUERY_LIMITED_INFORMATION unlike GetModuleBaseNameW
                    let mut path_buffer: [u16; 260] = [0; 260];
                    let mut size = path_buffer.len() as u32;

                    if QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR::from_raw(path_buffer.as_mut_ptr()), &mut size).is_ok() && size > 0 {
                        let full_path = String::from_utf16_lossy(&path_buffer[..size as usize]);
                        // Extract just the filename from the full path
                        if let Some(name) = full_path.rsplit('\\').next() {
                            process_names.push(name.to_string());
                        }
                    }

                    let _ = CloseHandle(handle);
                }
            }
        }

        Ok(process_names)
    }
}

/// Check if a process is running (case-insensitive)
fn is_process_running(process_name: &str) -> Result<bool, String> {
    let processes = get_running_processes()?;
    let target = process_name.to_lowercase();

    Ok(processes.iter().any(|p| p.to_lowercase() == target))
}

/// Check that a process is NOT running
pub fn check_absent(config: &CheckConfig) -> CheckResult {
    let process_name = match &config.process_name {
        Some(name) => name,
        None => {
            return CheckResult::error(
                &config.id,
                &config.name,
                "Missing process_name in config",
            )
        }
    };

    match is_process_running(process_name) {
        Ok(running) => {
            if running {
                CheckResult::fail(
                    &config.id,
                    &config.name,
                    "Running",
                    "Not Running",
                )
            } else {
                CheckResult::pass(
                    &config.id,
                    &config.name,
                    "Not Running",
                    "Not Running",
                )
            }
        }
        Err(e) => CheckResult::error(&config.id, &config.name, &e),
    }
}

/// Check that a process IS running
pub fn check_present(config: &CheckConfig) -> CheckResult {
    let process_name = match &config.process_name {
        Some(name) => name,
        None => {
            return CheckResult::error(
                &config.id,
                &config.name,
                "Missing process_name in config",
            )
        }
    };

    match is_process_running(process_name) {
        Ok(running) => {
            if running {
                CheckResult::pass(
                    &config.id,
                    &config.name,
                    "Running",
                    "Running",
                )
            } else {
                CheckResult::fail(
                    &config.id,
                    &config.name,
                    "Not Running",
                    "Running",
                )
            }
        }
        Err(e) => CheckResult::error(&config.id, &config.name, &e),
    }
}

/// Terminate all instances of a process by name (case-insensitive)
/// Returns Ok(count) with number of processes terminated, or Err on failure
pub fn terminate_process(process_name: &str) -> Result<u32, String> {
    let target = process_name.to_lowercase();
    let mut terminated_count = 0u32;

    unsafe {
        // Get list of process IDs
        let mut pids: [u32; 2048] = [0; 2048];
        let mut bytes_returned: u32 = 0;

        let result = EnumProcesses(
            pids.as_mut_ptr(),
            std::mem::size_of_val(&pids) as u32,
            &mut bytes_returned,
        );

        if result.is_err() {
            return Err("Failed to enumerate processes".to_string());
        }

        let count = bytes_returned as usize / std::mem::size_of::<u32>();

        for &pid in &pids[..count] {
            if pid == 0 {
                continue;
            }

            // First check if this is the process we want to terminate
            let query_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
            if let Ok(query_handle) = query_handle {
                if query_handle != HANDLE::default() {
                    let mut path_buffer: [u16; 260] = [0; 260];
                    let mut size = path_buffer.len() as u32;

                    let mut process_name_found = None;
                    if QueryFullProcessImageNameW(query_handle, PROCESS_NAME_WIN32, PWSTR::from_raw(path_buffer.as_mut_ptr()), &mut size).is_ok() && size > 0 {
                        let full_path = String::from_utf16_lossy(&path_buffer[..size as usize]);
                        if let Some(name) = full_path.rsplit('\\').next() {
                            process_name_found = Some(name.to_string());
                        }
                    }
                    let _ = CloseHandle(query_handle);

                    if let Some(name) = process_name_found {
                        if name.to_lowercase() == target {
                            // Found matching process, try to terminate it
                            if let Ok(term_handle) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                                if term_handle != HANDLE::default() {
                                    if TerminateProcess(term_handle, 0).is_ok() {
                                        terminated_count += 1;
                                    }
                                    let _ = CloseHandle(term_handle);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(terminated_count)
}
