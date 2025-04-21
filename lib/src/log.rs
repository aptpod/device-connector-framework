use crossbeam::atomic::AtomicCell;
use libc::c_char;
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::ffi::CStr;
use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use termcolor::{Buffer, BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

static LEVEL: AtomicCell<DcLogLevel> = AtomicCell::new(DcLogLevel::Info);
static COLORED: AtomicBool = AtomicBool::new(false);
static BUF_WRITER: Lazy<BufferWriter> = Lazy::new(|| {
    let color_choice = if COLORED.load(Ordering::Relaxed) {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };
    BufferWriter::stderr(color_choice)
});

thread_local! {
    static BUFFER: RefCell<Buffer> = RefCell::new(BUF_WRITER.buffer());
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum DcLogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

/// Initialize logger.
#[no_mangle]
pub extern "C" fn dc_log_init(level: DcLogLevel) {
    LEVEL.store(level);

    if std::io::stderr().is_terminal() {
        COLORED.store(true, Ordering::Relaxed);
    }
}

/// Get current log level.
#[no_mangle]
pub extern "C" fn dc_log_get_level() -> DcLogLevel {
    LEVEL.load()
}

/// Append a log.
#[no_mangle]
pub unsafe extern "C" fn dc_log(
    level: DcLogLevel,
    plugin: *const c_char,
    module: *const c_char,
    msg: *const c_char,
) {
    if level > dc_log_get_level() {
        return;
    }

    let time = humantime::format_rfc3339_seconds(std::time::SystemTime::now());
    let (level, level_color) = match level {
        DcLogLevel::Error => ("ERROR", Color::Red),
        DcLogLevel::Warn => ("WARN", Color::Yellow),
        DcLogLevel::Info => ("INFO", Color::Green),
        DcLogLevel::Debug => ("DEBUG", Color::Blue),
        DcLogLevel::Trace => ("TRACE", Color::Cyan),
    };
    let module_color = Color::Ansi256(8); // BrightBlack

    let plugin = unsafe { CStr::from_ptr(plugin) };
    let module = unsafe { CStr::from_ptr(module) };
    let msg = unsafe { CStr::from_ptr(msg) };

    BUFFER.with_borrow_mut(|buffer| {
        let _ = write!(buffer, "{}", time);
        let _ = buffer.set_color(ColorSpec::new().set_fg(Some(level_color)));
        let _ = write!(buffer, " {}", level);
        let _ = buffer.set_color(ColorSpec::new().set_fg(Some(module_color)));
        let _ = write!(
            buffer,
            " [{} ({})]",
            module.to_string_lossy(),
            plugin.to_string_lossy(),
        );
        let _ = buffer.set_color(ColorSpec::new().set_fg(None));
        let _ = writeln!(buffer, ": {}", msg.to_string_lossy());
        let _ = BUF_WRITER.print(buffer);
        buffer.clear();
    });
}
