//! The terminal front end for Open Photo AI.

fn main() {
    // Before anything this binary prints, so that a session's records begin where the session does — and before the
    // version, which is the one thing this binary says today and is not a log record.
    //
    // A failure is reported and the run continues: a terminal application that cannot write a log is still a terminal
    // application, and the one thing it does below does not need one. The report goes to stderr, which unlike the
    // GUI's this binary genuinely has — so it stays out of stdout, where only the version belongs.
    if let Err(error) = opai::logging::init(opai::APP_NAME, opai::CLI) {
        eprintln!("opai: could not install the log sink; continuing without a log: {error}");
    }

    println!("{}", opai::version());
}
