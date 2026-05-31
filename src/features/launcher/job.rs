use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use std::ptr::{null, null_mut};

use winapi::ctypes::c_void;
use winapi::um::handleapi::CloseHandle;
use winapi::um::jobapi2::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject, TerminateJobObject,
};
use winapi::um::winnt::{
    HANDLE, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation,
};

pub struct JobObject {
    handle: HANDLE,
}

impl JobObject {
    pub fn new() -> io::Result<Self> {
        unsafe {
            let handle = CreateJobObjectW(null_mut(), null());
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let ok = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as *mut c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                let error = io::Error::last_os_error();
                CloseHandle(handle);
                return Err(error);
            }

            Ok(Self { handle })
        }
    }

    pub fn assign(&self, child: &Child) -> io::Result<()> {
        let handle = child.as_raw_handle() as HANDLE;
        let ok = unsafe { AssignProcessToJobObject(self.handle, handle) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// kills every process currently in the job (the launched child and any
    /// processes it spawned). the job object stays valid for reuse afterwards.
    pub fn terminate(&self) -> io::Result<()> {
        let ok = unsafe { TerminateJobObject(self.handle, 0) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        // closing the last handle triggers KILL_ON_JOB_CLOSE
        unsafe { CloseHandle(self.handle) };
    }
}
