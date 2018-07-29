#[cfg(feature = "backtrace")]
use std::cell::UnsafeCell;
use std::env;
use std::fmt;
use std::os::raw::c_void;
use std::path::{Path, PathBuf};
use std::str;

#[cfg(feature = "backtrace")]
use backtrace_support;

/// Represents a symbol name.
pub struct SymbolName<'a> {
    bytes: &'a [u8],
    #[cfg(feature = "backtrace")]
    demangled: UnsafeCell<Option<String>>,
}

impl<'a> SymbolName<'a> {
    /// Creates a new symbol name from the raw underlying bytes.
    ///
    /// If the backtrace feature is enabled this will also perform basic
    /// demangling.
    pub fn new(bytes: &'a [u8]) -> SymbolName<'a> {
        SymbolName {
            bytes: bytes,
            #[cfg(feature = "backtrace")]
            demangled: UnsafeCell::new(None),
        }
    }

    /// Returns the raw symbol name as a `str` if the symbols is valid utf-8.
    ///
    /// This will in general return a mangled string on most platforms.
    pub fn as_str(&self) -> Option<&'a str> {
        str::from_utf8(self.bytes).ok()
    }

    /// Returns the raw symbol name as a list of bytes
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Returns the demangled version of the name if available.
    pub fn demangled(&self) -> Option<&str> {
        #[cfg(feature = "backtrace")]
        {
            let demangled = self.demangled.get();
            unsafe {
                if (*demangled).is_none() {
                    (*demangled) = Some({
                        let mut sym = backtrace_support::SymbolName::new(self.bytes).to_string();
                        // chop off the hash marker for rust functions
                        let truncate = {
                            let mut iter = sym.rsplitn(2, "::");
                            let chop_last = iter.next()
                                .map_or(false, |h| h.len() == 17 && h.starts_with("h"));
                            if chop_last {
                                Some(iter.next().unwrap_or("").len())
                            } else {
                                None
                            }
                        };
                        if let Some(truncate) = truncate {
                            sym.truncate(truncate);
                        }
                        sym
                    });
                }
                (*demangled).as_ref().map(|x| &x[..])
            }
        }
        #[cfg(not(feature = "backtrace"))]
        {
            None
        }
    }
}

impl<'a> fmt::Display for SymbolName<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(demangled) = self.demangled() {
            fmt::Display::fmt(demangled, f)
        } else if let Some(s) = self.as_str() {
            fmt::Display::fmt(s, f)
        } else {
            fmt::Display::fmt(&String::from_utf8_lossy(self.bytes), f)
        }
    }
}

impl<'a> fmt::Debug for SymbolName<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(demangled) = self.demangled() {
            fmt::Debug::fmt(demangled, f)
        } else if let Some(s) = self.as_str() {
            fmt::Debug::fmt(s, f)
        } else {
            fmt::Debug::fmt(&String::from_utf8_lossy(self.bytes), f)
        }
    }
}

/// Represents a symbol in a frame.
pub struct Symbol {
    name: Option<Vec<u8>>,
    addr: Option<usize>,
    filename: Option<PathBuf>,
    lineno: Option<u32>,
}

impl Symbol {
    /// Returns the name of the symbol
    pub fn name(&self) -> Option<SymbolName> {
        self.name.as_ref().map(|s| SymbolName::new(s))
    }

    /// Returns the address of the symbol
    pub fn addr(&self) -> Option<*mut c_void> {
        self.addr.map(|s| s as *mut c_void)
    }

    /// Returns the filename
    pub fn filename(&self) -> Option<&Path> {
        self.filename.as_ref().map(|p| &**p)
    }

    /// Returns the line number
    pub fn lineno(&self) -> Option<u32> {
        self.lineno
    }

    /// Returns `true` if this is an internal symbol.
    fn is_backtrace_internal(&self) -> bool {
        let name = match self.name() {
            Some(name) => name,
            None => return false,
        };

        if let Some(raw_name) = name.as_str() {
            if raw_name.starts_with("__ZN6defeat") || raw_name.starts_with("defeat::") {
                return true;
            }
        }

        if let Some(name) = name.demangled() {
            if name.starts_with("defeat::") {
                return true;
            }
        }

        false
    }

    /// Returns `true` if this is the border frame leaving rust user code.
    fn is_end_of_user_code(&self) -> bool {
        let name = match self.name() {
            Some(name) => name,
            None => return false,
        };

        if let Some(raw_name) = name.as_str() {
            if raw_name.starts_with("__ZN3std2rt10lang_start")
                || raw_name.starts_with("std::rt::lang_start::")
            {
                return true;
            }
        }

        if let Some(name) = name.demangled() {
            if name.starts_with("std::rt::lang_start::") {
                return true;
            }
        }

        false
    }
}

/// The reason why a backtrace is captured.
pub enum CapturePurpose {
    /// Capture a backtrace for a panic.
    Panic,
    /// Capture a backtrace for an error.
    Error,
}

/// A hint to what type of IP is stored in a frame.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AddrHint {
    /// A precise address
    Precise,
    /// A return address
    Return,
}

/// Represents a frame.
pub struct Frame {
    ip: *mut c_void,
    hint: AddrHint,
    #[cfg(feature = "backtrace")]
    resolved: UnsafeCell<Option<Vec<Symbol>>>,
}

#[cfg(feature = "backtrace")]
fn resolve_frame(ip: *mut c_void) -> Vec<Symbol> {
    let mut rv = Vec::with_capacity(1);
    backtrace_support::resolve(ip, |symbol| {
        rv.push(Symbol {
            name: symbol.name().map(|m| m.as_bytes().to_vec()),
            addr: symbol.addr().map(|a| a as usize),
            filename: symbol.filename().map(|m| m.to_path_buf()),
            lineno: symbol.lineno(),
        });
    });
    rv
}

impl Frame {
    /// Creates a new frame.
    pub fn new(ip: *mut c_void, addr_hint: AddrHint) -> Frame {
        Frame {
            ip: ip,
            hint: addr_hint,
            #[cfg(feature = "backtrace")]
            resolved: UnsafeCell::new(None),
        }
    }

    /// Creates a new resolved frame.
    #[cfg(feature = "backtrace")]
    fn new_resolved(ip: *mut c_void, addr_hint: AddrHint, symbols: Vec<Symbol>) -> Frame {
        Frame {
            ip: ip,
            hint: addr_hint,
            resolved: UnsafeCell::new(Some(symbols)),
        }
    }

    /// The instruction pointer of this frame.
    pub fn ip(&self) -> *mut c_void {
        self.ip
    }

    /// The address hint.
    pub fn addr_hint(&self) -> AddrHint {
        self.hint
    }

    /// The address of the call.
    ///
    /// If the frame address hint is `Return` this will attempt to unwind back
    /// to the location of the call.
    pub fn call_ip(&self) -> *mut c_void {
        match self.hint {
            AddrHint::Precise => self.ip,
            AddrHint::Return => {
                // XXX: unsafe, stupid and wrong
                unsafe { self.ip.offset(-1) }
            }
        }
    }

    /// The symbols corresponding with this frame.
    ///
    /// If the symbols are not known this might be an empty list.
    #[cfg(feature = "backtrace")]
    pub fn symbols(&self) -> &[Symbol] {
        #[cfg(feature = "backtrace")]
        {
            let resolved = self.resolved.get();
            unsafe {
                if (*resolved).is_none() {
                    (*resolved) = Some(resolve_frame(self.call_ip()));
                }
                &(*resolved).as_ref().unwrap()[..]
            }
        }
        #[cfg(not(feature = "backtrace"))]
        {
            &[]
        }
    }

    /// Releases the symbols
    fn take_symbols(&self) -> Vec<Symbol> {
        self.symbols();
        unsafe { (*self.resolved.get()).take().unwrap() }
    }
}

enum BacktraceRepr {
    /// a backtrace that is always empty
    Empty,
    /// A backtrace made from frames.
    #[cfg(feature = "backtrace")]
    Frames(Vec<Frame>),
}

/// Represents a backtrace.
pub struct Backtrace {
    repr: BacktraceRepr,
}

impl Backtrace {
    /// Produces an empty backtrace.
    pub fn empty() -> Backtrace {
        Default::default()
    }

    /// Checks if backtraces are generally supported.
    pub fn supported() -> bool {
        cfg!(feature = "backtrace")
    }

    /// Captures the backtrace at the current position.
    ///
    /// If the platform does not support backtrace capturing then `None` is
    /// returned.
    #[inline(never)]
    pub fn capture() -> Option<Backtrace> {
        #[cfg(feature = "backtrace")]
        {
            Some(Backtrace {
                repr: BacktraceRepr::Frames(capture_backtrace(false)),
            })
        }
        #[cfg(not(feature = "backtrace"))]
        {
            None
        }
    }

    /// Captures the backtrace specific for the current purpose.
    ///
    /// If the platform does not support backtrace capturing then `None` is
    /// returned.
    pub fn conditional_capture(purpose: CapturePurpose) -> Option<Backtrace> {
        let var = match purpose {
            CapturePurpose::Panic => "RUST_PANIC_BACKTRACE",
            CapturePurpose::Error => "RUST_ERROR_BACKTRACE",
        };

        match env::var(var).as_ref().map(|x| x.as_str()).ok() {
            Some("1") | Some("full") => {
                return Backtrace::capture();
            }
            Some("0") => {
                return None;
            }
            _ => {}
        }

        match env::var("RUST_BACKTRACE").as_ref().map(|x| x.as_str()).ok() {
            Some("1") | Some("full") => Backtrace::capture(),
            _ => None,
        }
    }

    /// Checks if the stacktrace is empty.
    pub fn is_empty(&self) -> bool {
        match self.repr {
            BacktraceRepr::Empty => true,
            #[cfg(feature = "backtrace")]
            BacktraceRepr::Frames(ref frames) => frames.is_empty(),
        }
    }

    /// Automatically trim the stacktrace.
    ///
    /// This removes uninteresting frames from the top and bottom of the
    /// stacktrace to make it easier to inspect to users.  This removes
    /// all internal frames from the backtrace system itself as well as
    /// frames below the user's main function.
    pub fn trimmed(self) -> Backtrace {
        #[cfg(feature = "backtrace")]
        {
            let frameiter = match self.repr {
                BacktraceRepr::Empty => return self,
                BacktraceRepr::Frames(frames) => frames.into_iter(),
            };

            enum State {
                BeforeBacktraceInternal,
                FoundBacktraceInternal,
                InStack,
            }

            let mut state = State::BeforeBacktraceInternal;
            let symbols = frameiter.flat_map(|x| {
                let ip = x.ip();
                let addr_hint = x.addr_hint();
                x.take_symbols()
                    .into_iter()
                    .map(move |s| (ip, addr_hint, s))
            });

            let mut pending_frame = None;
            let mut rv = vec![];

            for (ip, addr_hint, symbol) in symbols {
                match state {
                    State::BeforeBacktraceInternal => {
                        if symbol.is_backtrace_internal() {
                            state = State::FoundBacktraceInternal;
                        }
                        continue;
                    }
                    State::FoundBacktraceInternal => {
                        if symbol.is_backtrace_internal() {
                            continue;
                        }
                        state = State::InStack;
                    }
                    State::InStack => {
                        if symbol.is_end_of_user_code() {
                            break;
                        }
                    }
                }

                match pending_frame {
                    None => {
                        pending_frame = Some((ip, addr_hint, vec![symbol]));
                    }
                    Some((cur_ip, cur_addr_hint, ref mut symbols))
                        if cur_ip == ip && cur_addr_hint == addr_hint =>
                    {
                        symbols.push(symbol);
                    }
                    Some((_, _, symbols)) => {
                        if !symbols.is_empty() {
                            rv.push(Frame::new_resolved(ip, addr_hint, symbols));
                        }
                        pending_frame = Some((ip, addr_hint, vec![symbol]));
                    }
                }
            }

            if let Some((ip, addr_hint, symbols)) = pending_frame {
                if !symbols.is_empty() {
                    rv.push(Frame::new_resolved(ip, addr_hint, symbols));
                }
            }

            Backtrace {
                repr: BacktraceRepr::Frames(rv),
            }
        }

        #[cfg(not(feature = "backtrace"))]
        {
            self
        }
    }

    /// Iterates over the frames.
    pub fn iter_frames<'a>(&'a self) -> FrameIter<'a> {
        FrameIter {
            bt: &self.repr,
            #[cfg(feature = "backtrace")]
            idx: 0,
        }
    }
}

/// An iterator over all frames in a backtrace.
pub struct FrameIter<'a> {
    bt: &'a BacktraceRepr,
    #[cfg(feature = "backtrace")]
    idx: usize,
}

impl<'a> Iterator for FrameIter<'a> {
    type Item = &'a Frame;

    fn next(&mut self) -> Option<&'a Frame> {
        match *self.bt {
            BacktraceRepr::Empty => None,
            #[cfg(feature = "backtrace")]
            BacktraceRepr::Frames(ref frames) => match frames.get(self.idx) {
                Some(frame) => {
                    self.idx += 1;
                    Some(frame)
                }
                None => None,
            },
        }
    }
}

impl Default for Backtrace {
    fn default() -> Backtrace {
        Backtrace {
            repr: BacktraceRepr::Empty,
        }
    }
}

#[cfg(feature = "backtrace")]
fn capture_backtrace(light: bool) -> Vec<Frame> {
    let mut idx = 0;
    let mut rv = vec![];
    backtrace_support::trace(|frame| {
        let hint = if idx == 0 {
            AddrHint::Precise
        } else {
            AddrHint::Return
        };
        rv.push(Frame::new(frame.ip(), hint));
        idx += 1;
        !light || idx < 3
    });
    rv
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Symbol")
            .field("name", &self.name())
            .field("addr", &self.addr())
            .field("filename", &self.filename())
            .field("lineno", &self.lineno())
            .finish()
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref name) = self.name() {
            fmt::Display::fmt(name, f)?;
        } else {
            write!(f, "?")?;
        }
        let file = self.filename().and_then(|x| x.file_name().map(Path::new));
        let lineno = self.lineno();
        match (file, lineno) {
            (Some(file), Some(lineno)) => write!(f, " ({}:{})", file.display(), lineno)?,
            (Some(file), None) => write!(f, " ({})", file.display())?,
            _ => {}
        }
        Ok(())
    }
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Frame")
            .field("ip", &self.ip)
            .field("hint", &self.hint)
            .field("symbols", &self.symbols())
            .finish()
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "  ")?;
        if f.alternate() {
            write!(f, "{: >14p} ", self.ip())?;
        }
        for (idx, sym) in self.symbols().iter().enumerate() {
            if idx > 0 {
                write!(f, "\n")?;
                if f.alternate() {
                    write!(f, "{: >14} ", "")?;
                }
            }
            write!(f, "in {}", sym)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Backtrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let frames: Vec<_> = self.iter_frames().collect();
        f.debug_struct("Backtrace")
            .field("frames", &&frames[..])
            .finish()
    }
}

impl fmt::Display for Backtrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Backtrace (most recent call first):")?;
        for frame in self.iter_frames() {
            write!(f, "\n")?;
            fmt::Display::fmt(frame, f)?;
        }
        Ok(())
    }
}
