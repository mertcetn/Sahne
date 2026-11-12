use super::*;

pub(super) const DEFAULT_RATE: u32 = 1000;
pub(super) const PROBE_RATE: u32 = 100;
const FLAGS: u32 = JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP;

pub(super) fn configure(job: HANDLE, rate: u32) -> io::Result<()> {
    let mut control = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
        ControlFlags: FLAGS,
        ..Default::default()
    };
    control.Anonymous.CpuRate = rate;
    // SAFETY: Live host-owned Job and initialized exact CPU-rate structure.
    check(unsafe {
        SetInformationJobObject(
            job,
            JobObjectCpuRateControlInformation,
            &control as *const _ as *const c_void,
            size_of_val(&control) as u32,
        )
    })?;
    verify(job, rate)
}

fn verify(job: HANDLE, rate: u32) -> io::Result<()> {
    let mut control = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION::default();
    // SAFETY: Live host Job, or NULL for the worker's immediate Job; exact output layout.
    check(unsafe {
        QueryInformationJobObject(
            job,
            JobObjectCpuRateControlInformation,
            &mut control as *mut _ as *mut c_void,
            size_of_val(&control) as u32,
            ptr::null_mut(),
        )
    })?;
    // SAFETY: Query populated CpuRate; exact flags reject weight/min-max union interpretations.
    if control.ControlFlags != FLAGS || unsafe { control.Anonymous.CpuRate } != rate {
        return Err(io::Error::other("CPU hard-cap policy readback mismatch"));
    }
    Ok(())
}

pub fn probe_cpu_budget() -> io::Result<()> {
    if !is_appcontainer()? {
        return Err(io::Error::other("CPU probe requires AppContainer"));
    }
    verify(ptr::null_mut(), PROBE_RATE)
}

pub(super) fn verify_usage(sandbox: &Sandbox) -> io::Result<()> {
    verify(sandbox.job.as_raw_handle(), PROBE_RATE)?;
    if sandbox.cpu_rate != PROBE_RATE || sandbox.wait(0)?.is_some() {
        return Err(io::Error::other(
            "CPU fixture must remain alive under its hard cap",
        ));
    }
    // SAFETY: No pointers; OS returns the count for all active processor groups.
    let logical = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) };
    if logical == 0 || logical > 32 {
        return Err(io::Error::other(
            "CPU measurement fixture requires 1..32 logical processors",
        ));
    }
    let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    // SAFETY: Live owned Job, initialized exact accounting structure.
    check(unsafe {
        QueryInformationJobObject(
            sandbox.job.as_raw_handle(),
            JobObjectBasicAccountingInformation,
            &mut accounting as *mut _ as *mut c_void,
            size_of_val(&accounting) as u32,
            ptr::null_mut(),
        )
    })?;
    let seconds = sandbox.started.elapsed().as_secs_f64();
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    let mut ids = [0u64; 10];
    // SAFETY: Aligned bounded buffers for fixed limits and at most eight process IDs.
    unsafe {
        check(QueryInformationJobObject(
            sandbox.job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            &mut limits as *mut _ as *mut c_void,
            size_of_val(&limits) as u32,
            ptr::null_mut(),
        ))?;
        check(QueryInformationJobObject(
            sandbox.job.as_raw_handle(),
            JobObjectBasicProcessIdList,
            ids.as_mut_ptr().cast(),
            size_of_val(&ids) as u32,
            ptr::null_mut(),
        ))?;
        let list = ids.as_ptr().cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>();
        let count = (*list).NumberOfProcessIdsInList as usize;
        if count > 8 || count != (*list).NumberOfAssignedProcesses as usize {
            return Err(io::Error::other("Oversized CPU Job process list"));
        }
        let processes = std::slice::from_raw_parts(
            std::ptr::addr_of!((*list).ProcessIdList).cast::<usize>(),
            count,
        );
        if limits.BasicLimitInformation.ActiveProcessLimit != 1
            || limits.BasicLimitInformation.LimitFlags & JOB_OBJECT_LIMIT_ACTIVE_PROCESS == 0
            || processes.len() != 1
            || processes[0] != GetProcessId(sandbox.child.process.as_raw_handle()) as usize
        {
            return Err(io::Error::other(
                "CPU Job contains an unexpected process or active-process policy",
            ));
        }
        println!(
            "CPU Job membership: configured active limit={}, flags={:#x}, accounting active={}, process IDs={processes:?}",
            limits.BasicLimitInformation.ActiveProcessLimit,
            limits.BasicLimitInformation.LimitFlags,
            accounting.ActiveProcesses
        );
    }
    let cpu = (accounting.TotalUserTime as f64 + accounting.TotalKernelTime as f64) / 10_000_000.0;
    // Rate limits count cycles per scheduling interval, not exact accumulated wall time.
    // This observational allowance is separate from the exact enforced 1% policy.
    let allowance = seconds * f64::from(logical) * 0.01 * 2.0 + 0.1;
    if accounting.TotalUserTime < 200_000
        || accounting.TotalKernelTime < 0
        || accounting.TotalProcesses != 1
        || accounting.ActiveProcesses != 1
        || cpu > allowance
    {
        return Err(io::Error::other(format!(
            "CPU accounting outside test bounds: cpu={cpu:.4}s, allowance={allowance:.4}s, associations={}, active={}",
            accounting.TotalProcesses, accounting.ActiveProcesses
        )));
    }
    println!(
        "CPU hard cap verified: rate=1%, logical processors={logical}, wall={seconds:.4}s, user+kernel={cpu:.4}s, observational allowance={allowance:.4}s; native loop alive before watchdog stop."
    );
    Ok(())
}
