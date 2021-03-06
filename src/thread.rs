use ::{ NifEnv, NifTerm, NifEncoder };
use ::wrapper::nif_interface::{ self, ErlNifPid };
use ::types::atom;
use std::mem;
use std::thread;
use std::panic;
use std::marker::PhantomData;

/// Return the calling process's pid.
fn caller<'a>(caller_env: NifEnv<'a>) -> ErlNifPid {
    let mut pid: ErlNifPid = unsafe { mem::uninitialized() };
    unsafe {
        nif_interface::enif_self(caller_env.as_c_arg(), &mut pid);
    }
    pid
}

/// Implements threaded NIFs.
///
/// This spawns a thread that calls the given closure `thread_fn`. When the closure returns, the
/// thread sends its return value back to the calling process.  If the closure panics, an `{error,
/// Reason}` tuple is sent instead.
///
/// Note that the thread creates a new `NifEnv` and passes it to the closure, so the closure
/// runs under a separate environment, not under `env`.
///
pub fn spawn<'a, F>(env: NifEnv<'a>, thread_fn: F)
    where F: for<'b> FnOnce(NifEnv<'b>) -> NifTerm<'b> + Send + panic::UnwindSafe + 'static,
{
    let pid = caller(env);
    thread::spawn(move || {
        let env = NifEnv {
            env: unsafe { nif_interface::enif_alloc_env() },
            id: PhantomData
        };

        let message = match panic::catch_unwind(|| thread_fn(env)) {
            Ok(term) => term,
            Err(err) => {
                // Try to get an error message from Rust.
                let reason =
                    if let Some(string) = err.downcast_ref::<String>() {
                        string.encode(env)
                    } else if let Some(&s) = err.downcast_ref::<&'static str>() {
                        s.encode(env)
                    } else {
                        atom::get_atom_init("nif_panic").to_term(env)
                    };
                env.error_tuple(reason)
            }
        };

        unsafe {
            nif_interface::enif_send(env.as_c_arg(),
                                     &pid,
                                     env.as_c_arg(),
                                     message.as_c_arg());
        }
    });
}
