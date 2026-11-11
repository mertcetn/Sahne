use std::{
    ffi::c_void,
    fs::File,
    io,
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, Cryptography::*, Isolation::*, *},
    Storage::FileSystem::{FILE_GENERIC_EXECUTE, FILE_GENERIC_READ},
    System::{JobObjects::*, Memory::*, Pipes::*, Threading::*},
};

fn wide(value: impl AsRef<std::ffi::OsStr>) -> io::Result<Vec<u16>> {
    let mut text: Vec<u16> = value.as_ref().encode_wide().collect();
    if text.contains(&0) {
        return Err(io::Error::other("Embedded NUL"));
    }
    text.push(0);
    Ok(text)
}
fn check(ok: i32) -> io::Result<()> {
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn winerror(code: u32) -> io::Result<()> {
    if code != 0 {
        Err(io::Error::from_raw_os_error(code as i32))
    } else {
        Ok(())
    }
}
fn hresult(code: i32) -> io::Result<()> {
    if code < 0 {
        Err(io::Error::other(format!("HRESULT {code:#x}")))
    } else {
        Ok(())
    }
}
unsafe fn own(handle: HANDLE) -> io::Result<OwnedHandle> {
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    }
    // SAFETY: Successful Win32 creation transferred one owned handle to the caller.
    else {
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
}
pub fn random_id() -> io::Result<u64> {
    let mut bytes = [0u8; 8];
    // SAFETY: Output is an initialized writable array of the declared length.
    let result = unsafe {
        BCryptGenRandom(
            ptr::null_mut(),
            bytes.as_mut_ptr(),
            8,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if result < 0 {
        return Err(io::Error::other(format!("CSPRNG failed: {result:#x}")));
    }
    let id = u64::from_le_bytes(bytes);
    if id == 0 {
        return random_id();
    }
    Ok(id)
}
struct Profile {
    name: Vec<u16>,
    sid: PSID,
}
impl Profile {
    fn new(id: u64) -> io::Result<Self> {
        let name = wide(format!("Sahne.Spike.{}.{id:016x}", std::process::id()))?;
        let mut sid = ptr::null_mut();
        // SAFETY: Terminated strings/output pointer live through the synchronous call; no capabilities.
        hresult(unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                name.as_ptr(),
                name.as_ptr(),
                ptr::null(),
                0,
                &mut sid,
            )
        })?;
        Ok(Self { name, sid })
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        // SAFETY: The unique profile was created by this instance; sid was allocated by the OS.
        unsafe {
            let result = DeleteAppContainerProfile(self.name.as_ptr());
            if result < 0 {
                eprintln!("AppContainer profile cleanup failed: {result:#x}");
            }
            FreeSid(self.sid);
        }
    }
}
struct LocalAllocation(*mut c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: This pointer was returned by a LocalAlloc-family security API.
            unsafe {
                LocalFree(self.0);
            }
        }
    }
}
fn grant_read_execute(directory: &Path, sid: PSID) -> io::Result<()> {
    let name = wide(directory)?;
    let mut old_acl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    // SAFETY: Initialized output pointers; terminated path. Only the fresh staging directory is passed.
    winerror(unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut old_acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    })?;
    let _descriptor = LocalAllocation(descriptor);
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid.cast(),
        },
    };
    let mut new_acl = ptr::null_mut();
    // SAFETY: Old descriptor/sid/entry live until the new ACL has been applied.
    winerror(unsafe { SetEntriesInAclW(1, &entry, old_acl, &mut new_acl) })?;
    let _acl = LocalAllocation(new_acl.cast());
    // SAFETY: Applies only the constructed ACL, preserving existing rights on the fresh directory.
    winerror(unsafe {
        SetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            new_acl,
            ptr::null(),
        )
    })
}
struct Attributes {
    storage: Vec<u128>,
    initialized: bool,
}
impl Attributes {
    fn new() -> io::Result<Self> {
        let mut size = 0;
        // SAFETY: Size query uses null buffer as required by the API.
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut size);
        }
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut attributes = Self {
            storage: vec![0; size.div_ceil(16)],
            initialized: false,
        };
        // SAFETY: The buffer is 16-byte aligned and at least the requested size.
        check(unsafe { InitializeProcThreadAttributeList(attributes.pointer(), 2, 0, &mut size) })?;
        attributes.initialized = true;
        Ok(attributes)
    }
    fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
    fn set<T>(&mut self, key: usize, value: &T) -> io::Result<()> {
        // SAFETY: The caller keeps value alive until CreateProcess and attribute destruction finish.
        check(unsafe {
            UpdateProcThreadAttribute(
                self.pointer(),
                0,
                key,
                value as *const T as *const c_void,
                size_of::<T>(),
                ptr::null_mut(),
                ptr::null(),
            )
        })
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: Owns an initialized attribute list and its backing storage.
        unsafe {
            if self.initialized {
                DeleteProcThreadAttributeList(self.pointer());
            }
        }
    }
}
fn pipe() -> io::Result<(OwnedHandle, OwnedHandle)> {
    let attrs = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
    };
    let (mut read, mut write) = (ptr::null_mut(), ptr::null_mut());
    // SAFETY: Output handle slots and security structure are valid.
    unsafe {
        check(CreatePipe(&mut read, &mut write, &attrs, 4096))?;
        Ok((own(read)?, own(write)?))
    }
}
fn token(handle: HANDLE) -> io::Result<OwnedHandle> {
    let mut output = ptr::null_mut();
    // SAFETY: Only query access is requested; output is writable.
    unsafe {
        check(OpenProcessToken(handle, TOKEN_QUERY, &mut output))?;
        own(output)
    }
}
fn token_number(token: HANDLE, kind: TOKEN_INFORMATION_CLASS) -> io::Result<u32> {
    let (mut value, mut size) = (0u32, 0u32);
    // SAFETY: Numeric token classes used here return a DWORD.
    check(unsafe {
        GetTokenInformation(
            token,
            kind,
            &mut value as *mut u32 as *mut c_void,
            4,
            &mut size,
        )
    })?;
    Ok(value)
}
pub fn is_appcontainer() -> io::Result<bool> {
    // SAFETY: GetCurrentProcess returns a borrowed pseudo handle, never closed by this code.
    let token = token(unsafe { GetCurrentProcess() })?;
    Ok(token_number(token.as_raw_handle(), TokenIsAppContainer)? == 1)
}
fn verify_token(process: HANDLE, sid: PSID) -> io::Result<()> {
    let handle = token(process)?;
    let raw = handle.as_raw_handle();
    if token_number(raw, TokenIsAppContainer)? != 1 {
        return Err(io::Error::other("Worker is not an AppContainer"));
    }
    let mut size = 0;
    // SAFETY: Query required size, then use aligned storage for returned TOKEN_GROUPS.
    unsafe {
        GetTokenInformation(raw, TokenCapabilities, ptr::null_mut(), 0, &mut size);
    }
    if size == 0 || size > 4096 {
        return Err(io::Error::other("Unexpected capability token size"));
    }
    let mut groups = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: Buffer is aligned and sized according to the API query.
    check(unsafe {
        GetTokenInformation(
            raw,
            TokenCapabilities,
            groups.as_mut_ptr().cast(),
            size,
            &mut size,
        )
    })?;
    if groups[0] as u32 != 0 {
        return Err(io::Error::other("Worker has unexpected capabilities"));
    }
    size = 0;
    // SAFETY: Query variable-size TOKEN_APPCONTAINER_INFORMATION, including backing SID storage.
    unsafe {
        GetTokenInformation(raw, TokenAppContainerSid, ptr::null_mut(), 0, &mut size);
    }
    if size == 0 || size > 4096 {
        return Err(io::Error::other("Unexpected profile token size"));
    }
    let mut info = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: Aligned buffer is at least the requested length; returned SID lives in this buffer.
    check(unsafe {
        GetTokenInformation(
            raw,
            TokenAppContainerSid,
            info.as_mut_ptr().cast(),
            size,
            &mut size,
        )
    })?;
    // SAFETY: Successful query guarantees TOKEN_APPCONTAINER_INFORMATION layout in aligned storage.
    let info = unsafe { &*info.as_ptr().cast::<TOKEN_APPCONTAINER_INFORMATION>() };
    // SAFETY: Both SIDs remain owned by query buffer/profile for comparison.
    if unsafe { EqualSid(info.TokenAppContainer, sid) } == 0 {
        return Err(io::Error::other("Worker profile SID mismatch"));
    }
    Ok(())
}
struct Child {
    process: OwnedHandle,
    thread: OwnedHandle,
}
impl Drop for Child {
    fn drop(&mut self) {
        // SAFETY: Owns the newly created process, including failures before Job assignment/resume.
        unsafe {
            TerminateProcess(self.process.as_raw_handle(), 1);
            WaitForSingleObject(self.process.as_raw_handle(), 1000);
        }
    }
}
pub struct Sandbox {
    child: Child,
    job: OwnedHandle,
    _profile: Profile,
}
impl Sandbox {
    pub fn stop(&self) -> io::Result<()> {
        // SAFETY: Job/process belong to this sandbox and never target unrelated processes.
        unsafe {
            check(TerminateJobObject(self.job.as_raw_handle(), 1))?;
            if WaitForSingleObject(self.child.process.as_raw_handle(), 1000) != WAIT_OBJECT_0 {
                return Err(io::Error::other(
                    "Worker did not terminate within one second",
                ));
            }
        }
        Ok(())
    }
    pub fn wait(&self, milliseconds: u32) -> io::Result<Option<u32>> {
        // SAFETY: Process handle remains valid for the wait and exit-code query.
        unsafe {
            match WaitForSingleObject(self.child.process.as_raw_handle(), milliseconds) {
                WAIT_TIMEOUT => Ok(None),
                WAIT_OBJECT_0 => {
                    let mut code = 0;
                    check(GetExitCodeProcess(
                        self.child.process.as_raw_handle(),
                        &mut code,
                    ))?;
                    Ok(Some(code))
                }
                _ => Err(io::Error::last_os_error()),
            }
        }
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            eprintln!("Worker cleanup: {e}");
        }
    }
}
pub struct Channel {
    pub sandbox: Sandbox,
    pub input: File,
    pub output: File,
}

/// Launch a copy of the trusted probe only; root must be a fresh directory.
/// No permissions are changed on the source executable or repository directories.
pub fn spawn_probe(
    executable: &Path,
    root: &Path,
    session: u64,
    case: &str,
    port: u16,
) -> io::Result<Channel> {
    if ![
        "healthy",
        "network",
        "network-egress",
        "memory",
        "hang",
        "oversized",
        "crash",
    ]
    .contains(&case)
    {
        return Err(io::Error::other("Unknown probe case"));
    }
    std::fs::create_dir(root)?;
    let bin = root.join("bin");
    std::fs::create_dir(&bin)?;
    let marker = root.join("denied.txt");
    std::fs::write(&marker, b"Sahne sandbox sentinel")?;
    let profile =
        Profile::new(session).map_err(|e| io::Error::other(format!("Create profile: {e}")))?;
    grant_read_execute(&bin, profile.sid)
        .map_err(|e| io::Error::other(format!("Staging ACL: {e}")))?;
    let copied = bin.join("probe.exe");
    std::fs::copy(executable, &copied)?;
    let app = wide(&copied)?;
    let cwd = wide(&bin)?;
    let command = format!(
        "\"{}\" --worker {session} {case} \"{}\" {} {port}",
        copied.display(),
        marker.display(),
        std::process::id()
    );
    let mut command = wide(command)?;
    let (child_in, parent_in) = pipe()?;
    let (parent_out, child_out) = pipe()?;
    // SAFETY: Clear inheritance on exactly the host-side pipe handles.
    unsafe {
        check(SetHandleInformation(
            parent_in.as_raw_handle(),
            HANDLE_FLAG_INHERIT,
            0,
        ))?;
        check(SetHandleInformation(
            parent_out.as_raw_handle(),
            HANDLE_FLAG_INHERIT,
            0,
        ))?;
    }
    let caps = SECURITY_CAPABILITIES {
        AppContainerSid: profile.sid,
        Capabilities: ptr::null_mut(),
        CapabilityCount: 0,
        Reserved: 0,
    };
    let inherited = [child_in.as_raw_handle(), child_out.as_raw_handle()];
    let mut attributes = Attributes::new()?;
    attributes.set(PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize, &caps)?;
    attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize, &inherited)?;
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = child_in.as_raw_handle();
    startup.StartupInfo.hStdOutput = child_out.as_raw_handle();
    startup.StartupInfo.hStdError = child_out.as_raw_handle();
    startup.lpAttributeList = attributes.pointer();
    // Windows resolves the AppContainer profile while constructing the child environment.
    // Copy only OS location metadata, never the caller's full environment (tokens/proxies/PATH).
    let mut environment = Vec::new();
    for key in ["APPDATA", "LOCALAPPDATA", "SystemRoot", "USERPROFILE"] {
        let value =
            std::env::var_os(key).ok_or_else(|| io::Error::other(format!("Missing {key}")))?;
        let mut entry: Vec<u16> = key.encode_utf16().collect();
        entry.push(b'=' as u16);
        entry.extend(wide(value)?);
        environment.extend(entry);
    }
    environment.push(0);
    // SAFETY: Job creation transfers a new owned handle; no name exposes it to the worker.
    let job = unsafe { own(CreateJobObjectW(ptr::null(), ptr::null()))? };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_PROCESS_MEMORY
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
    limits.BasicLimitInformation.ActiveProcessLimit = 1;
    limits.ProcessMemoryLimit = 256 * 1024 * 1024;
    // SAFETY: Exact information layout and length for this owned job.
    check(unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const c_void,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    })?;
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: All strings, environment, handles, security capabilities and attribute buffers live through process creation. Only listed handles inherit; worker starts suspended.
    check(unsafe {
        CreateProcessW(
            app.as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            CREATE_SUSPENDED
                | CREATE_NO_WINDOW
                | EXTENDED_STARTUPINFO_PRESENT
                | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr().cast(),
            cwd.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        )
    })?;
    // SAFETY: CreateProcess succeeded and transferred both handles.
    let child = unsafe {
        Child {
            process: own(process.hProcess)?,
            thread: own(process.hThread)?,
        }
    };
    // SAFETY: Assign before any worker code can run.
    check(unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.process.as_raw_handle()) })?;
    verify_token(child.process.as_raw_handle(), profile.sid)?;
    let sandbox = Sandbox {
        child,
        job,
        _profile: profile,
    };
    // Drop host copies of child-side endpoints before resuming, so EOF works on termination.
    drop(child_in);
    drop(child_out);
    // SAFETY: Resume exactly the primary thread of the verified restricted process.
    if unsafe { ResumeThread(sandbox.child.thread.as_raw_handle()) } == u32::MAX {
        return Err(io::Error::last_os_error());
    }
    Ok(Channel {
        sandbox,
        input: File::from(parent_in),
        output: File::from(parent_out),
    })
}
pub fn host_process_access_denied(pid: u32) -> io::Result<bool> {
    // SAFETY: Probe only requests access and immediately closes a successful handle; no memory is read/written.
    unsafe {
        let handle = OpenProcess(PROCESS_VM_READ | PROCESS_VM_WRITE, 0, pid);
        if handle.is_null() {
            Ok(io::Error::last_os_error().raw_os_error() == Some(5))
        } else {
            drop(own(handle)?);
            Ok(false)
        }
    }
}

/// Trusted single-allocation limit probe; never touches pages or exhausts the host.
/// Returns the exact OS error from the rejected allocation, not a JS/runtime error.
pub fn probe_memory_limit() -> io::Result<i32> {
    if !is_appcontainer()? {
        return Err(io::Error::other("Memory probe requires AppContainer"));
    }
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    // SAFETY: Null job selects the caller's associated Job. Output is initialized,
    // correctly sized/aligned and remains valid through this synchronous query.
    check(unsafe {
        QueryInformationJobObject(
            ptr::null_mut(),
            JobObjectExtendedLimitInformation,
            &mut limits as *mut _ as *mut c_void,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ptr::null_mut(),
        )
    })?;
    if limits.BasicLimitInformation.LimitFlags & JOB_OBJECT_LIMIT_PROCESS_MEMORY == 0
        || limits.ProcessMemoryLimit != 256 * 1024 * 1024
    {
        return Err(io::Error::other("Unexpected worker Job memory limit"));
    }
    // SAFETY: Allocations affect only this process. No returned pointer is dereferenced;
    // successful allocations are released with the original base and MEM_RELEASE/size 0.
    unsafe {
        let baseline = VirtualAlloc(
            ptr::null(),
            64 * 1024,
            MEM_RESERVE | MEM_COMMIT,
            PAGE_READWRITE,
        );
        if baseline.is_null() {
            return Err(io::Error::last_os_error());
        }
        check(VirtualFree(baseline, 0, MEM_RELEASE))?;
        let oversized = VirtualAlloc(
            ptr::null(),
            257 * 1024 * 1024,
            MEM_RESERVE | MEM_COMMIT,
            PAGE_READWRITE,
        );
        if oversized.is_null() {
            let error = io::Error::last_os_error();
            // Other failures must not be misclassified as memory-limit enforcement.
            return match error.raw_os_error() {
                Some(code @ (8 | 1455)) => Ok(code),
                _ => Err(error),
            };
        }
        check(VirtualFree(oversized, 0, MEM_RELEASE))?;
    }
    Err(io::Error::other(
        "Allocation above Job limit unexpectedly succeeded",
    ))
}
