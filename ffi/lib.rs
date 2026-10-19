use std::{
    cell::UnsafeCell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicBool, Ordering},
};

struct Request {
    input: Vec<u8>,
    cancelled: AtomicBool,
    output: UnsafeCell<Vec<u8>>,
}

#[repr(C)]
enum Status {
    Ok,
    Error,
    Cancelled,
}

#[repr(C)]
struct Response {
    status: Status,
    data: *const u8,
    len: usize,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mcptype_request_create(data: *const u8, len: usize) -> *mut Request {
    if data.is_null() {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(Request {
        input: unsafe { std::slice::from_raw_parts(data, len) }.to_vec(),
        cancelled: AtomicBool::new(false),
        output: UnsafeCell::new(Vec::new()),
    }))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mcptype_request_cancel(request: *const Request) {
    unsafe { &*request }
        .cancelled
        .store(true, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mcptype_request_free(request: *mut Request) {
    if !request.is_null() {
        drop(unsafe { Box::from_raw(request) });
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mcptype_compile(request: *const Request) -> Response {
    let request = unsafe { &*request };
    let result = catch_unwind(AssertUnwindSafe(|| {
        let input = serde_json::from_slice(&request.input)?;
        let generated = mcptype::compile_cancellable(input, &request.cancelled)?;
        Ok::<_, mcptype::Error>(serde_json::to_vec(&generated)?)
    }));
    let (status, bytes) = match result {
        Ok(Ok(bytes)) => (Status::Ok, bytes),
        Ok(Err(mcptype::Error::Cancelled)) => (Status::Cancelled, Vec::new()),
        Ok(Err(error)) => (Status::Error, error.to_string().into_bytes()),
        Err(_) => (Status::Error, b"mcptype compiler panicked".to_vec()),
    };
    let output = unsafe { &mut *request.output.get() };
    *output = bytes;
    Response {
        status,
        data: output.as_ptr(),
        len: output.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_requests_own_results_and_honor_cancellation() {
        let input = br#"{"language":"python","contracts":{"files":{"tools":[]}}}"#;
        unsafe {
            let request = mcptype_request_create(input.as_ptr(), input.len());
            let response = mcptype_compile(request);
            assert!(matches!(response.status, Status::Ok));
            let output = std::slice::from_raw_parts(response.data, response.len);
            let result: serde_json::Value = serde_json::from_slice(output).unwrap();
            assert!(result["declarations"].as_str().is_some());
            mcptype_request_cancel(request);
            assert!(matches!(mcptype_compile(request).status, Status::Cancelled));
            mcptype_request_free(request);
        }
    }
}
