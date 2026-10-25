//! ABI v1. Caller serializes access to each handle. Cancellation and status never wait for a build.
//! Inputs are borrowed for the call. Results belong to Rust until cec_result_free.
//! All output pointers initialize to null on failure. Codes: 0 OK, 1 invalid input, 2 panic.
#![allow(clippy::missing_safety_doc)]
use compiler_core::{BuildOptions, Compiler};
use prost::Message;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};
pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/compiler.rs"));
}

impl From<messages::BuildOptions> for BuildOptions {
    fn from(o: messages::BuildOptions) -> Self {
        Self {
            engine: o.engine,
            source_ref: o.source_ref,
            msys_root: o.msys_root,
            output_root: o.output_root,
            architecture: o.architecture,
            jobs: o.jobs,
            pgo: o.pgo,
            strip: o.strip,
            backend: o.backend,
            python_path: o.python_path,
            visual_studio: o.visual_studio,
            cuda_root: o.cuda_root,
            weights_path: o.weights_path,
            auto_architecture: o.auto_architecture,
        }
    }
}
impl From<BuildOptions> for messages::BuildOptions {
    fn from(o: BuildOptions) -> Self {
        Self {
            engine: o.engine,
            source_ref: o.source_ref,
            msys_root: o.msys_root,
            output_root: o.output_root,
            architecture: o.architecture,
            jobs: o.jobs,
            pgo: o.pgo,
            strip: o.strip,
            backend: o.backend,
            python_path: o.python_path,
            visual_studio: o.visual_studio,
            cuda_root: o.cuda_root,
            weights_path: o.weights_path,
            auto_architecture: o.auto_architecture,
        }
    }
}
impl From<compiler_core::Snapshot> for messages::Snapshot {
    fn from(s: compiler_core::Snapshot) -> Self {
        Self {
            state: s.state,
            phase: s.phase,
            message: s.message,
            output: s.output,
            log_path: s.log_path,
            log_tail: s.log_tail,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cec_abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cec_create(out: *mut *mut Compiler) -> u32 {
    if out.is_null() {
        return 1;
    }
    unsafe {
        *out = ptr::null_mut();
    }
    catch_unwind(AssertUnwindSafe(|| {
        unsafe {
            *out = Box::into_raw(Box::default());
        }
        0
    }))
    .unwrap_or(2)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cec_call(
    core: *mut Compiler,
    request: *const u8,
    length: usize,
    out: *mut *mut Vec<u8>,
) -> u32 {
    if out.is_null() {
        return 1;
    }
    unsafe {
        *out = ptr::null_mut();
    }
    if core.is_null() || request.is_null() || length > 1024 * 1024 {
        return 1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let Ok(request) = messages::Request::decode(bytes) else {
            return 1;
        };
        let core = unsafe { &*core };
        let mut response = messages::Response {
            request_id: request.request_id,
            ..Default::default()
        };
        let operation = messages::request::Operation::try_from(request.operation);
        use messages::request::Operation::*;
        let result = match operation {
            Ok(Hardware) => {
                let h = compiler_core::hardware::detect();
                response.hardware = Some(messages::Hardware {
                    cpu_name: h.cpu_name,
                    recommended: h.recommended,
                    explanation: h.explanation,
                    choices: h.choices,
                    berserk_recommended: h.berserk_recommended,
                    berserk_explanation: h.berserk_explanation,
                    berserk_choices: h.berserk_choices,
                    obsidian_recommended: h.obsidian_recommended,
                    obsidian_choices: h.obsidian_choices,
                });
                Ok(())
            }
            Ok(GithubStart) => match request.github {
                Some(q) => core
                    .github
                    .lock()
                    .unwrap()
                    .start(compiler_core::github::Query {
                        action: q.action,
                        engine: q.engine,
                        auth_mode: q.auth_mode,
                        token: q.token,
                        page: q.page,
                        id: q.id,
                        output_folder: q.output_folder,
                    }),
                None => Err("GitHub options are required.".into()),
            },
            Ok(GithubStatus | GithubCancel) => {
                let worker = core.github.lock().unwrap();
                if operation == Ok(GithubCancel) {
                    worker.cancel();
                }
                response.github = Some(worker.snapshot().into());
                Ok(())
            }
            Ok(Status) => Ok(()),
            Ok(Defaults) => {
                response.options = Some(
                    compiler_core::defaults(
                        request
                            .options
                            .as_ref()
                            .map(|o| o.engine.as_str())
                            .unwrap_or("stockfish"),
                    )
                    .into(),
                );
                Ok(())
            }
            Ok(Cancel) => {
                core.cancel();
                Ok(())
            }
            Ok(Plan | Start) => match request.options {
                None => Err("Build options are required.".into()),
                Some(options) => {
                    let options: BuildOptions = options.into();
                    if operation == Ok(Plan) {
                        compiler_core::plan(&options).map(|plan| response.steps = plan.steps)
                    } else {
                        core.start(options)
                    }
                }
            },
            Err(_) => Err("Unknown operation. Update the app and native core together.".into()),
        };
        if let Err(error) = result {
            response.error = error;
        }
        response.snapshot = Some(core.snapshot().into());
        unsafe {
            *out = Box::into_raw(Box::new(response.encode_to_vec()));
        }
        0
    }))
    .unwrap_or(2)
}
impl From<compiler_core::github::NetworkSnapshot> for messages::GitHubSnapshot {
    fn from(s: compiler_core::github::NetworkSnapshot) -> Self {
        let source = |v: compiler_core::github::SourceVersion| messages::SourceVersion {
            id: v.id,
            label: v.label,
            reference: v.reference,
            date: v.date,
            url: v.url,
            prerelease: v.prerelease,
        };
        Self {
            state: s.state,
            message: s.message,
            releases: s.releases.into_iter().map(source).collect(),
            development: s.development.into_iter().map(source).collect(),
            items: s
                .items
                .into_iter()
                .map(|i| messages::DownloadItem {
                    id: i.id,
                    name: i.name,
                    size: i.size,
                    date: i.date,
                    digest: i.digest,
                    expired: i.expired,
                    details: i.details,
                })
                .collect(),
            login: s.login,
            output: s.output,
            received: s.received,
            total: s.total,
            page: s.page,
            more: s.more,
            default_branch: s.default_branch,
            releases_more: s.releases_more,
            development_more: s.development_more,
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cec_result_bytes(
    result: *const Vec<u8>,
    bytes: *mut *const u8,
    length: *mut usize,
) -> u32 {
    if bytes.is_null() || length.is_null() {
        return 1;
    }
    unsafe {
        *bytes = ptr::null();
        *length = 0;
    }
    if result.is_null() {
        return 1;
    }
    unsafe {
        *bytes = (*result).as_ptr();
        *length = (*result).len();
    }
    0
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cec_result_free(result: *mut Vec<u8>) {
    if !result.is_null() {
        unsafe {
            drop(Box::from_raw(result));
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cec_destroy(core: *mut Compiler) {
    if !core.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
            drop(Box::from_raw(core));
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi_roundtrip_and_malformed_input() {
        unsafe {
            let mut core = ptr::null_mut();
            assert_eq!(cec_create(&mut core), 0);
            let request = messages::Request {
                request_id: 42,
                operation: 1,
                options: None,
                github: None,
            }
            .encode_to_vec();
            let mut result = ptr::null_mut();
            assert_eq!(
                cec_call(core, request.as_ptr(), request.len(), &mut result),
                0
            );
            let mut bytes = ptr::null();
            let mut length = 0;
            assert_eq!(cec_result_bytes(result, &mut bytes, &mut length), 0);
            let response =
                messages::Response::decode(std::slice::from_raw_parts(bytes, length)).unwrap();
            assert_eq!(response.request_id, 42);
            assert_eq!(response.options.unwrap().engine, "stockfish");
            cec_result_free(result);
            assert_eq!(cec_call(core, [255].as_ptr(), 1, &mut result), 1);
            assert!(result.is_null());
            cec_destroy(core);
        }
    }
}
