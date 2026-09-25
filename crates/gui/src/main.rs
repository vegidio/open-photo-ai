//! Thin binary entry point. All the wiring lives in `gui_lib`.

// Prevents an additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // THE FIRST STATEMENT, which is `prepare_library_path`'s documented contract and the reason this call is here
    // rather than in `gui_lib::run`: it mutates the process environment and on Linux replaces the process image, and
    // both are only sound while no other thread exists. `run` installs the log sink in its first lines, and a sink
    // installed before a re-exec would leave the replacement holding a file handle whose writer died with the old
    // process.
    //
    // The directories it returns are discarded. Nothing in `gui` needs them: `Opai::initialize` resolves its own
    // from the same application name, and a second copy held here would be a second answer to where the libraries
    // are.
    //
    // The failure is carried into `run` rather than unwrapped, because there is no log yet to report it to — and it
    // is not fatal. It means a directory could not be created, which `Opai::initialize` runs into again a moment
    // later and reports through the setup dialog; failing the launch here would replace an explanation with a window
    // that never opens.
    //
    // SAFETY: the first statement of `main`, so no thread but this one exists and nothing has yet read or written the
    // process environment.
    let prepared = unsafe { opai::prepare_library_path(opai::APP_NAME) }.map(|_| ());

    gui_lib::run(prepared)
}
