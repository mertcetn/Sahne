//! Bounded committed-memory pressure in the trusted worker, never host allocation.
use super::*;

const MIB: usize = 1024 * 1024;
const LIMIT: usize = 256 * MIB;
const CHUNK: usize = 16 * MIB;
const MAX_CHUNKS: usize = 16;
const MIN_COMMITTED: usize = 192 * MIB;

struct Allocation(*mut c_void);
impl Allocation {
    fn new(bytes: usize) -> io::Result<Self> {
        // SAFETY: Fixed bounded size in this process only; no executable protection.
        // The pointer is owned below and is never dereferenced or shared.
        let address =
            unsafe { VirtualAlloc(ptr::null(), bytes, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE) };
        if address.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(address))
    }
    fn release(&mut self) -> io::Result<()> {
        if !self.0.is_null() {
            // SAFETY: Original base returned by VirtualAlloc, size zero for MEM_RELEASE.
            check(unsafe { VirtualFree(self.0, 0, MEM_RELEASE) })?;
            self.0 = ptr::null_mut();
        }
        Ok(())
    }
}
impl Drop for Allocation {
    fn drop(&mut self) {
        // Best-effort release only on error/unwind; normal paths check each release.
        let _ = self.release();
    }
}

fn query_limits(job: HANDLE) -> io::Result<JOBOBJECT_EXTENDED_LIMIT_INFORMATION> {
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    // SAFETY: Host-owned Job or NULL for this worker's immediate Job; exact output layout.
    check(unsafe {
        QueryInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &mut limits as *mut _ as *mut c_void,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ptr::null_mut(),
        )
    })?;
    if limits.ProcessMemoryLimit != LIMIT
        || limits.BasicLimitInformation.LimitFlags & JOB_OBJECT_LIMIT_PROCESS_MEMORY == 0
    {
        return Err(io::Error::other(
            "Incremental fixture Job memory policy mismatch",
        ));
    }
    Ok(limits)
}

/// Commit at most sixteen fixed 16 MiB regions; release all before reporting success.
pub fn probe_incremental_memory() -> io::Result<()> {
    if !is_appcontainer()? {
        return Err(io::Error::other(
            "Incremental memory probe requires AppContainer",
        ));
    }
    query_limits(ptr::null_mut())?;
    Allocation::new(64 * 1024)?.release()?;
    // Allocate bookkeeping before pressure. No logging/string building occurs while full.
    let mut allocations = Vec::with_capacity(MAX_CHUNKS);
    let mut denied = false;
    for _ in 0..MAX_CHUNKS {
        match Allocation::new(CHUNK) {
            Ok(allocation) => allocations.push(allocation),
            Err(error) if matches!(error.raw_os_error(), Some(8 | 1455)) => {
                denied = true;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    let committed = allocations.len() * CHUNK;
    // Release explicitly before error formatting or subsequent protocol allocation.
    for allocation in &mut allocations {
        allocation.release()?;
    }
    if !denied || committed < MIN_COMMITTED {
        return Err(io::Error::other(format!(
            "Incremental commit fixture outside bounds: committed={committed}, denied={denied}"
        )));
    }
    // Recover enough headroom for a larger successful allocation, then normal IPC/exit.
    Allocation::new(32 * MIB)?.release()?;
    validate_peak(&query_limits(ptr::null_mut())?)
}

fn validate_peak(limits: &JOBOBJECT_EXTENDED_LIMIT_INFORMATION) -> io::Result<()> {
    if !(MIN_COMMITTED..=LIMIT).contains(&limits.PeakProcessMemoryUsed) {
        return Err(io::Error::other(
            "Incremental fixture Job peak outside 192..256 MiB",
        ));
    }
    Ok(())
}

pub(super) fn verify_peak(sandbox: &Sandbox) -> io::Result<()> {
    if sandbox.wait(1000)? != Some(0) {
        return Err(io::Error::other(
            "Incremental memory worker must exit normally",
        ));
    }
    let limits = query_limits(sandbox.job.as_raw_handle())?;
    validate_peak(&limits)?;
    let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    // SAFETY: Live host-owned Job and correctly sized initialized output.
    check(unsafe {
        QueryInformationJobObject(
            sandbox.job.as_raw_handle(),
            JobObjectBasicAccountingInformation,
            &mut accounting as *mut _ as *mut c_void,
            size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            ptr::null_mut(),
        )
    })?;
    if accounting.TotalProcesses != 1 || accounting.ActiveProcesses != 0 {
        return Err(io::Error::other(
            "Incremental memory Job process accounting mismatch",
        ));
    }
    println!(
        "Incremental memory host evidence: peak committed={} bytes, limit={} bytes; one worker exited 0 after release and recovery allocation.",
        limits.PeakProcessMemoryUsed, limits.ProcessMemoryLimit
    );
    Ok(())
}
