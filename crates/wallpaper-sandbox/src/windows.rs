use std::{
    ffi::c_void,
    fs::File,
    io::{self, Read, Write},
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        io::{AsHandle, AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, Cryptography::*, Isolation::*, *},
    Storage::FileSystem::{
        CreateFileW, FILE_ALL_ACCESS, FILE_ATTRIBUTE_NORMAL, FILE_CREATE_PIPE_INSTANCE,
        FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_READ_DATA,
        FILE_WRITE_DATA, OPEN_EXISTING, PIPE_ACCESS_DUPLEX, READ_CONTROL, SYNCHRONIZE, WRITE_DAC,
    },
    System::{
        Diagnostics::Debug::{GetErrorMode, SEM_NOGPFAULTERRORBOX, SetErrorMode},
        JobObjects::*,
        Memory::*,
        Pipes::*,
        SystemServices::PROCESS_MITIGATION_STRICT_HANDLE_CHECK_POLICY,
        Threading::*,
    },
};
mod cpu;
mod filesystem;
mod memory;
pub use cpu::probe_cpu_budget;
pub use memory::probe_incremental_memory;
mod profile;
mod registry;
pub use filesystem::{probe_filesystem, verify_filesystem_fixture};
use profile::Profile;
pub use profile::{recover_pending_preview_profiles, recover_preview_profile};
pub use registry::probe_registry;

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
    fn new(count: u32) -> io::Result<Self> {
        let mut size = 0;
        // SAFETY: Size query uses null buffer as required by the API.
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), count, 0, &mut size);
        }
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut attributes = Self {
            storage: vec![0; size.div_ceil(16)],
            initialized: false,
        };
        // SAFETY: The buffer is 16-byte aligned and at least the requested size.
        check(unsafe {
            InitializeProcThreadAttributeList(attributes.pointer(), count, 0, &mut size)
        })?;
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
fn host_user_acl() -> io::Result<LocalAllocation> {
    host_user_acl_with_rights(FILE_ALL_ACCESS)
}
fn host_user_acl_with_rights(access: u32) -> io::Result<LocalAllocation> {
    // Use the actual host user's SID, not a broad default object DACL or package grant.
    // SAFETY: Borrowed current-process pseudo handle; token() owns only its returned token.
    let user_token = token(unsafe { GetCurrentProcess() })?;
    let mut bytes = 0;
    // SAFETY: Required-size query uses a null buffer as specified by GetTokenInformation.
    unsafe {
        GetTokenInformation(
            user_token.as_raw_handle(),
            TokenUser,
            ptr::null_mut(),
            0,
            &mut bytes,
        );
    }
    if bytes < size_of::<TOKEN_USER>() as u32 || bytes > 4096 {
        return Err(io::Error::other("Unexpected host user token size"));
    }
    let mut user = vec![0u64; (bytes as usize).div_ceil(8)];
    // SAFETY: Aligned, initialized buffer at least the queried size; SID remains live below.
    check(unsafe {
        GetTokenInformation(
            user_token.as_raw_handle(),
            TokenUser,
            user.as_mut_ptr().cast(),
            bytes,
            &mut bytes,
        )
    })?;
    // SAFETY: Successful query supplies TOKEN_USER and backing SID storage in this buffer.
    let sid = unsafe { (*user.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: access,
        grfAccessMode: SET_ACCESS,
        grfInheritance: NO_INHERITANCE,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: sid.cast(),
        },
    };
    let mut acl = ptr::null_mut();
    // SAFETY: One live trustee/SID, no old ACL. The API creates an owned ACL including ACE SID bytes.
    winerror(unsafe { SetEntriesInAclW(1, &entry, ptr::null(), &mut acl) })?;
    if acl.is_null() {
        return Err(io::Error::other("Refusing a null pipe DACL"));
    }
    Ok(LocalAllocation(acl.cast()))
}
fn pipe() -> io::Result<(OwnedHandle, OwnedHandle)> {
    let acl = host_user_acl()?;
    let mut descriptor = SECURITY_DESCRIPTOR::default();
    let descriptor_ptr = &mut descriptor as *mut _ as PSECURITY_DESCRIPTOR;
    // SAFETY: Initialize the absolute descriptor before use. The non-null ACL stays live
    // through creation; protection prevents inherited grants from broadening this DACL.
    unsafe {
        check(InitializeSecurityDescriptor(descriptor_ptr, 1))?;
        check(SetSecurityDescriptorDacl(
            descriptor_ptr,
            1,
            acl.0.cast(),
            0,
        ))?;
        check(SetSecurityDescriptorControl(
            descriptor_ptr,
            SE_DACL_PROTECTED,
            SE_DACL_PROTECTED,
        ))?;
    }
    let attrs = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor_ptr,
        bInheritHandle: 0,
    };
    let (mut read, mut write) = (ptr::null_mut(), ptr::null_mut());
    // SAFETY: Output handle slots and security structure are valid.
    unsafe {
        check(CreatePipe(&mut read, &mut write, &attrs, 4096))?;
        let read = own(read)?;
        let write = own(write)?;
        verify_pipe_acl(&read, acl.0.cast())?;
        verify_pipe_acl(&write, acl.0.cast())?;
        // Only data and synchronous I/O rights survive. The broad creation handles are
        // closed here, before any worker is created; no DUPLICATE_SAME_ACCESS shortcut.
        Ok((
            data_handle(&read, FILE_READ_DATA)?,
            data_handle(&write, FILE_WRITE_DATA)?,
        ))
    }
}
fn verify_pipe_acl(handle: &OwnedHandle, expected: *const ACL) -> io::Result<()> {
    let (mut acl, mut descriptor) = (ptr::null_mut(), ptr::null_mut());
    // SAFETY: Live owned handle with original READ_CONTROL rights; initialized outputs.
    winerror(unsafe {
        GetSecurityInfo(
            handle.as_raw_handle(),
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    })?;
    let _descriptor = LocalAllocation(descriptor);
    if acl.is_null() || descriptor.is_null() {
        return Err(io::Error::other("Pipe has no explicit DACL"));
    }
    let (mut control, mut revision) = (0, 0);
    // SAFETY: The descriptor and ACL are returned by the OS; expected is the live ACL
    // allocated by SetEntriesInAclW. GetAce yields initialized ACE storage in each ACL.
    unsafe {
        check(GetSecurityDescriptorControl(
            descriptor,
            &mut control,
            &mut revision,
        ))?;
        if control & (SE_DACL_PRESENT | SE_DACL_PROTECTED) != (SE_DACL_PRESENT | SE_DACL_PROTECTED)
            || (*acl).AceCount != 1
            || (*expected).AceCount != 1
        {
            return Err(io::Error::other("Pipe DACL protection/ACE count mismatch"));
        }
        let (mut actual_ace, mut expected_ace) = (ptr::null_mut(), ptr::null_mut());
        check(GetAce(acl, 0, &mut actual_ace))?;
        check(GetAce(expected, 0, &mut expected_ace))?;
        let actual_size = (*actual_ace.cast::<ACE_HEADER>()).AceSize as usize;
        let expected_size = (*expected_ace.cast::<ACE_HEADER>()).AceSize as usize;
        if actual_size != expected_size
            || std::slice::from_raw_parts(actual_ace.cast::<u8>(), actual_size)
                != std::slice::from_raw_parts(expected_ace.cast::<u8>(), expected_size)
        {
            return Err(io::Error::other("Pipe DACL trustee/rights mismatch"));
        }
    }
    Ok(())
}
fn data_handle(source: &impl AsHandle, access: u32) -> io::Result<OwnedHandle> {
    let mut duplicate = ptr::null_mut();
    // SAFETY: Borrowed live source and current-process pseudo handles, writable output;
    // DuplicateHandle returns one owned handle. No source-close option is used.
    unsafe {
        let process = GetCurrentProcess();
        check(DuplicateHandle(
            process,
            source.as_handle().as_raw_handle(),
            process,
            &mut duplicate,
            access | SYNCHRONIZE,
            1,
            0,
        ))?;
        own(duplicate)
    }
}

/// Fixed worker fixture: the inherited streams permit data only in their intended direction.
pub fn probe_channel_rights(input: &impl AsHandle, output: &impl AsHandle) -> io::Result<()> {
    if !is_appcontainer()? {
        return Err(io::Error::other(
            "Channel rights probe requires AppContainer",
        ));
    }
    // A valid non-null empty DACL is restrictive even if a regression unexpectedly
    // allows the mutation. Never test WRITE_DAC by applying a null/permissive DACL.
    #[repr(C, align(4))]
    struct AlignedAcl(ACL);
    let mut empty = AlignedAcl(ACL::default());
    // SAFETY: ACL is DWORD-aligned, writable, and exactly the size required for zero ACEs.
    check(unsafe { InitializeAcl(&mut empty.0, size_of::<ACL>() as u32, ACL_REVISION) })?;
    for handle in [input.as_handle(), output.as_handle()] {
        for access in [READ_CONTROL, WRITE_DAC, FILE_CREATE_PIPE_INSTANCE] {
            let mut duplicate = ptr::null_mut();
            // SAFETY: Valid borrowed handle and pseudo-process handles; initialized output.
            // Unexpected grants are immediately owned/closed without exercising the new right.
            let result = unsafe {
                let process = GetCurrentProcess();
                DuplicateHandle(
                    process,
                    handle.as_raw_handle(),
                    process,
                    &mut duplicate,
                    access,
                    0,
                    0,
                )
            };
            if result != 0 {
                // SAFETY: Successful DuplicateHandle transferred this new handle to us.
                drop(unsafe { own(duplicate) }?);
                return Err(io::Error::other(format!(
                    "Pipe duplicate gained rights {access:#x}"
                )));
            }
            if io::Error::last_os_error().raw_os_error() != Some(5) {
                return Err(io::Error::other(
                    "Pipe rights upgrade did not return access denied",
                ));
            }
        }
        let mut descriptor = ptr::null_mut();
        // SAFETY: Valid borrowed pipe handle and initialized outputs. On unexpected success
        // the returned descriptor is owned and freed without exposing it to application IPC.
        let result = unsafe {
            GetSecurityInfo(
                handle.as_raw_handle(),
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        let _descriptor = LocalAllocation(descriptor);
        if result != ERROR_ACCESS_DENIED {
            return Err(io::Error::other(format!(
                "Pipe DACL query returned {result}"
            )));
        }
        // SAFETY: Borrowed valid handle and live initialized empty ACL; no asynchronous use.
        let result = unsafe {
            SetSecurityInfo(
                handle.as_raw_handle(),
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                &empty.0,
                ptr::null(),
            )
        };
        if result != ERROR_ACCESS_DENIED {
            return Err(io::Error::other(format!(
                "Pipe DACL mutation returned {result}"
            )));
        }
    }
    // Clone the borrowed handles without adding rights. File performs actual OS I/O,
    // bypassing stdio buffering; no allowed-direction bytes are consumed or injected.
    let mut wrong_write = File::from(input.as_handle().try_clone_to_owned()?);
    let mut wrong_read = File::from(output.as_handle().try_clone_to_owned()?);
    if !wrong_write
        .write(&[0])
        .is_err_and(|e| e.raw_os_error() == Some(5))
    {
        return Err(io::Error::other(
            "Input pipe accepted wrong-direction write",
        ));
    }
    if !wrong_read
        .read(&mut [0])
        .is_err_and(|e| e.raw_os_error() == Some(5))
    {
        return Err(io::Error::other(
            "Output pipe accepted wrong-direction read",
        ));
    }
    Ok(())
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
        // SAFETY: Owns the newly created process, including failures before resume.
        unsafe {
            TerminateProcess(self.process.as_raw_handle(), 1);
            WaitForSingleObject(self.process.as_raw_handle(), 1000);
        }
    }
}
pub struct Sandbox {
    child: Child,
    job: OwnedHandle,
    _excluded: Option<File>,
    _endpoint: Option<EndpointFixture>,
    strict_handles: bool,
    profile: Profile,
    registry: Option<registry::RegistryFixture>,
    cpu_rate: u32,
    started: std::time::Instant,
}
impl Sandbox {
    pub fn verify_incremental_memory(&self) -> io::Result<()> {
        memory::verify_peak(self)
    }
    pub fn cleanup_registry(&mut self) -> io::Result<bool> {
        if let Some(fixture) = self.registry.as_mut() {
            fixture.cleanup()?;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn verify_cpu_rate(&self) -> io::Result<()> {
        cpu::verify_usage(self)
    }
    /// Call after stopping the worker and joining the channel reader. Errors prohibit retry.
    pub fn cleanup_profile(&mut self) -> io::Result<()> {
        self.stop()?;
        self.profile.cleanup()
    }
    pub fn strict_handle_checks(&self) -> bool {
        self.strict_handles
    }
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
    previous_session: Option<u64>,
) -> io::Result<Channel> {
    if ![
        "healthy",
        "package-basic",
        "package-snapshot",
        "package-script-snapshot",
        "package-invalid",
        "compiler-basic",
        "compiler-invalid",
        "image-basic",
        "image-invalid",
        "hierarchy-basic",
        "hierarchy-invalid",
        "groups-basic",
        "groups-invalid",
        "network",
        "network-egress",
        "memory",
        "memory-incremental",
        "process",
        "handles",
        "endpoint",
        "ipc-rights",
        "ipc-roundtrip",
        "ipc-roundtrip-replay",
        "ipc-roundtrip-stall",
        "scene-transaction",
        "scene-script",
        "local-clock-package",
        "local-script-package",
        "local-lifecycle-package",
        "preview-lifecycle-package",
        "live-lifecycle-package",
        "live-lifecycle-watchdog-check",
        "live-lifecycle-exit-check",
        "live-lifecycle-stale-check",
        "scene-timer",
        "scene-timer-registration",
        "preview-clock-utc",
        "ipc-write-stall",
        "ipc-write-drain",
        "cpu-busy",
        "filesystem",
        "registry",
        "hang",
        "oversized",
        "crash",
        "ipc-flood",
        "ipc-stall",
        "ipc-truncated",
        "ipc-replay",
        "ipc-open",
        "js-basic",
        "js-interrupt",
        "js-memory",
        "js-stack",
        "js-jobs",
        "js-timers",
        "ipc-old-session",
        "no-hello",
    ]
    .contains(&case)
    {
        return Err(io::Error::other("Unknown probe case"));
    }
    if case == "ipc-old-session" && previous_session.is_none_or(|old| old == 0 || old == session) {
        return Err(io::Error::other(
            "Old-session fixture requires a distinct previous session",
        ));
    }
    std::fs::create_dir(root)?;
    let bin = root.join("bin");
    std::fs::create_dir(&bin)?;
    let marker = root.join("denied.txt");
    std::fs::write(&marker, b"Sahne sandbox sentinel")?;
    let mut profile = Profile::new(session, root)
        .map_err(|e| io::Error::other(format!("Create profile: {e}")))?;
    grant_read_execute(&bin, profile.sid)
        .map_err(|e| io::Error::other(format!("Staging ACL: {e}")))?;
    if case == "filesystem" {
        filesystem::prepare(root)?;
    }
    let endpoint = if case == "endpoint" {
        Some(
            EndpointFixture::new(session)
                .map_err(|e| io::Error::other(format!("Endpoint positive control: {e}")))?,
        )
    } else {
        None
    };
    let registry = if case == "registry" {
        Some(registry::RegistryFixture::new(session)?)
    } else {
        None
    };
    let copied = bin.join("probe.exe");
    std::fs::copy(executable, &copied)?;
    let app = wide(&copied)?;
    let cwd = wide(&bin)?;
    // Deliberately inheritable, but omitted from PROC_THREAD_ATTRIBUTE_HANDLE_LIST.
    // Keep it open through worker termination so a stale/recycled host handle cannot pass.
    let excluded = if case == "handles" {
        let file = File::open(&marker)?;
        // SAFETY: Changes only the inheritance flag of our fresh test-file handle.
        check(unsafe {
            SetHandleInformation(
                file.as_raw_handle(),
                HANDLE_FLAG_INHERIT,
                HANDLE_FLAG_INHERIT,
            )
        })?;
        let mut flags = 0;
        // SAFETY: Borrowed live file handle and initialized output slot.
        check(unsafe { GetHandleInformation(file.as_raw_handle(), &mut flags) })?;
        if flags & HANDLE_FLAG_INHERIT == 0 {
            return Err(io::Error::other("Excluded test handle is not inheritable"));
        }
        Some(file)
    } else {
        None
    };
    let excluded_value = excluded
        .as_ref()
        .map_or(0, |file| file.as_raw_handle() as usize);
    let command = format!(
        "\"{}\" --worker {session} {case} \"{}\" {} {port} {excluded_value} {}",
        copied.display(),
        marker.display(),
        std::process::id(),
        previous_session.unwrap_or(0)
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
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = child_in.as_raw_handle();
    startup.StartupInfo.hStdOutput = child_out.as_raw_handle();
    startup.StartupInfo.hStdError = child_out.as_raw_handle();
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
    let cpu_rate = if case == "cpu-busy" {
        cpu::PROBE_RATE
    } else {
        cpu::DEFAULT_RATE
    };
    cpu::configure(job.as_raw_handle(), cpu_rate)?;
    let jobs = [job.as_raw_handle()];
    let mut attributes = Attributes::new(3)?;
    attributes.set(PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize, &caps)?;
    attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize, &inherited)?;
    attributes.set(PROC_THREAD_ATTRIBUTE_JOB_LIST as usize, &jobs)?;
    startup.lpAttributeList = attributes.pointer();
    profile.record_launch_intent()?;
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: All attribute buffers and handles live through creation. Windows starts
    // the worker suspended in the configured job; only the listed pipe handles inherit.
    check(unsafe {
        CreateProcessW(
            app.as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            CREATE_SUSPENDED
                | DETACHED_PROCESS
                | EXTENDED_STARTUPINFO_PRESENT
                | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr().cast(),
            cwd.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        )
    })?;
    drop(attributes);
    // SAFETY: CreateProcess succeeded and transferred both handles.
    let child = unsafe {
        Child {
            process: own(process.hProcess)?,
            thread: own(process.hThread)?,
        }
    };
    let mut in_job = 0;
    // SAFETY: Queries the exact owned child and job handles before the child can run.
    check(unsafe {
        IsProcessInJob(
            child.process.as_raw_handle(),
            job.as_raw_handle(),
            &mut in_job,
        )
    })?;
    if in_job == 0 {
        return Err(io::Error::other(
            "AppContainer worker was not created in its job",
        ));
    }
    verify_token(child.process.as_raw_handle(), profile.sid)?;
    profile.record_worker(process.dwProcessId)?;
    let strict_handles = if case == "handles" {
        let mut policy = PROCESS_MITIGATION_STRICT_HANDLE_CHECK_POLICY::default();
        // SAFETY: Query only the new suspended process; correctly sized initialized output.
        check(unsafe {
            GetProcessMitigationPolicy(
                child.process.as_raw_handle(),
                ProcessStrictHandleCheckPolicy,
                &mut policy as *mut _ as *mut c_void,
                size_of::<PROCESS_MITIGATION_STRICT_HANDLE_CHECK_POLICY>(),
            )
        })?;
        // SAFETY: This API fills the DWORD Flags member of the documented union.
        unsafe { policy.Anonymous.Flags & 1 != 0 }
    } else {
        false
    };
    let sandbox = Sandbox {
        child,
        job,
        _excluded: excluded,
        _endpoint: endpoint,
        strict_handles,
        profile,
        registry,
        cpu_rate,
        started: std::time::Instant::now(),
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
fn endpoint_name(host: u32, session: u64) -> String {
    format!(r"\\.\pipe\Sahne.Probe.{host}.{session:016x}")
}
struct EndpointFixture {
    _control_client: OwnedHandle,
    _control_server: OwnedHandle,
    _available_server: OwnedHandle,
}
impl EndpointFixture {
    fn new(session: u64) -> io::Result<Self> {
        let _acl = host_user_acl()?;
        let acl = _acl.0.cast();

        let mut descriptor = SECURITY_DESCRIPTOR::default();
        let descriptor_ptr = &mut descriptor as *mut _ as PSECURITY_DESCRIPTOR;
        // SAFETY: Absolute descriptor is initialized first; non-null ACL stays alive through both
        // synchronous creations. No null/absent DACL and no inherited access are permitted.
        unsafe {
            check(InitializeSecurityDescriptor(descriptor_ptr, 1))?;
            check(SetSecurityDescriptorDacl(descriptor_ptr, 1, acl, 0))?;
            check(SetSecurityDescriptorControl(
                descriptor_ptr,
                SE_DACL_PROTECTED,
                SE_DACL_PROTECTED,
            ))?;
        }
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor_ptr,
            bInheritHandle: 0,
        };
        let name = wide(endpoint_name(std::process::id(), session))?;
        // SAFETY: Paths/descriptor/ACL remain live; kernel copies security information at creation.
        // First-instance mode prevents silently reusing a pipe created by another process.
        let server = unsafe {
            own(CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                2,
                1024,
                1024,
                0,
                &security,
            ))?
        };
        // SAFETY: Open this freshly owned local endpoint only; no data is sent or read.
        // A client may connect after CreateNamedPipe even before ConnectNamedPipe is called.
        let client = unsafe {
            own(CreateFileW(
                name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            ))?
        };
        // Keep the successful host connection open and create a second available instance at
        // the same name, so worker denial cannot be attributed to a busy/absent endpoint.
        // SAFETY: Same name, access mode and instance cap; descriptor/ACL remain valid.
        let available = unsafe {
            own(CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                2,
                1024,
                1024,
                0,
                &security,
            ))?
        };
        println!(
            "Endpoint control: host opened the same name; user-only protected DACL, remote clients rejected, second instance available; no endpoint handle inherits."
        );
        Ok(Self {
            _control_client: client,
            _control_server: server,
            _available_server: available,
        })
    }
}

/// Trusted direct-OS probe for a known live host endpoint, not an application IPC API.
pub fn probe_host_endpoint(host: u32, session: u64) -> io::Result<()> {
    if !is_appcontainer()? {
        return Err(io::Error::other("Endpoint probe requires AppContainer"));
    }
    let name = wide(endpoint_name(host, session))?;
    for access in [GENERIC_READ, GENERIC_WRITE, GENERIC_READ | GENERIC_WRITE, 0] {
        // SAFETY: Open only the fixed synthetic endpoint. No handle ownership is assumed on
        // failure; unexpected success is owned/closed immediately without reading/writing.
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                access,
                0,
                ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        };
        if handle != INVALID_HANDLE_VALUE && !handle.is_null() {
            // SAFETY: The successful API call transferred one new owned handle.
            drop(unsafe { own(handle)? });
            return Err(io::Error::other(format!(
                "Unexpected endpoint connection: access={access:#x}"
            )));
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_ACCESS_DENIED as i32) {
            return Err(io::Error::other(format!(
                "Endpoint access={access:#x}, unexpected error={:?}",
                error.raw_os_error()
            )));
        }
    }
    Ok(())
}

/// Check only this process's uniquely named fixture after its owning Sandbox is dropped.
pub fn verify_host_endpoint_closed(session: u64) -> io::Result<()> {
    let name = wide(endpoint_name(std::process::id(), session))?;
    // SAFETY: Fixed synthetic local name; no data I/O. Unexpected success transfers one
    // handle, which is immediately closed. A live/busy/inaccessible endpoint is not absence.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            0,
            0,
            ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    };
    if handle != INVALID_HANDLE_VALUE && !handle.is_null() {
        // SAFETY: Successful open transferred this handle; never closes another owner's handle.
        drop(unsafe { own(handle)? });
        return Err(io::Error::other(
            "Endpoint still opens after owner destruction",
        ));
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(ERROR_FILE_NOT_FOUND as i32) {
        return Err(error);
    }
    Ok(())
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

/// Query a known host-only handle value without borrowing ownership or closing it.
/// Numeric collisions or errors other than INVALID_HANDLE fail this trusted fixture.
/// Strict-handle policy may terminate the worker instead; the host verifies that path.
pub fn probe_excluded_handle(value: usize) -> io::Result<()> {
    if !is_appcontainer()? || value == 0 || value == usize::MAX {
        return Err(io::Error::other("Invalid excluded-handle fixture"));
    }
    // SAFETY: Only this trusted single-threaded probe process changes its error mode.
    // Suppress WER for this intentional fatal reference, retaining all other bits.
    // This does not change strict-handle mitigation, isolation, or the host deadline.
    unsafe {
        SetErrorMode(GetErrorMode() | SEM_NOGPFAULTERRORBOX);
    }
    let mut flags = 0;
    // SAFETY: GetHandleInformation validates the opaque value; no pointer is dereferenced
    // and no ownership is assumed. Output storage remains live for the synchronous query.
    if unsafe { GetHandleInformation(value as HANDLE, &mut flags) } != 0 {
        return Err(io::Error::other(
            "Excluded handle was inherited or its value collided",
        ));
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(ERROR_INVALID_HANDLE as i32) {
        return Err(error);
    }
    Ok(())
}

/// Attempt only the current trusted executable, suspended and with no inherited handles.
/// Any unexpectedly created process is terminated without running its primary thread.
pub fn probe_child_process_limit() -> io::Result<()> {
    if !is_appcontainer()? {
        return Err(io::Error::other("Process probe requires AppContainer"));
    }
    let mut limits = JOBOBJECT_BASIC_LIMIT_INFORMATION::default();
    // SAFETY: Null selects this worker's immediate Job; output has the required layout.
    check(unsafe {
        QueryInformationJobObject(
            ptr::null_mut(),
            JobObjectBasicLimitInformation,
            &mut limits as *mut _ as *mut c_void,
            size_of::<JOBOBJECT_BASIC_LIMIT_INFORMATION>() as u32,
            ptr::null_mut(),
        )
    })?;
    if limits.LimitFlags & JOB_OBJECT_LIMIT_ACTIVE_PROCESS == 0 || limits.ActiveProcessLimit != 1 {
        return Err(io::Error::other("Unexpected worker active-process limit"));
    }
    let app = wide(std::env::current_exe()?)?;
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: Explicit trusted executable, no PATH lookup, no inheritance; strings and
    // output slots live through creation. Suspension prevents any unexpected child code.
    let created = unsafe {
        CreateProcessW(
            app.as_ptr(),
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            0,
            CREATE_SUSPENDED | DETACHED_PROCESS,
            ptr::null(),
            ptr::null(),
            &startup,
            &mut process,
        )
    };
    if created == 0 {
        let error = io::Error::last_os_error();
        // Access denied, missing image or other launch errors are not proof of Job quota.
        return if error.raw_os_error() == Some(ERROR_NOT_ENOUGH_QUOTA as i32) {
            Ok(())
        } else {
            Err(error)
        };
    }
    // SAFETY: Successful creation transferred both handles; Child kills/waits on drop.
    let unexpected = unsafe {
        Child {
            process: own(process.hProcess)?,
            thread: own(process.hThread)?,
        }
    };
    drop(unexpected);
    Err(io::Error::other(
        "Child process creation unexpectedly succeeded",
    ))
}
