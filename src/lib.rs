use std::ffi::c_ulong;
use std::sync::OnceLock;
use std::{ffi::c_void, net::SocketAddr};

use widestring::U16CString;
use windows_sys::Win32::{
    Foundation::{BOOL, HINSTANCE, TRUE},
    System::SystemServices::DLL_PROCESS_ATTACH,
};

use crate::textractor::{CurrentSelect, InfoForExtension, SentenceInfo, TextNumber};

mod mio_channel;
mod textractor;
mod websocket;

// windows-rs does not define DWORD and LPVOID
// https://github.com/microsoft/windows-rs/issues/881
#[allow(clippy::upper_case_acronyms)]
type DWORD = c_ulong;
#[allow(clippy::upper_case_acronyms)]
type LPVOID = *mut c_void;

static SERVER: OnceLock<websocket::ServerStarted> = OnceLock::new();

fn start_websocket_server() -> websocket::ServerStarted {
    println!("Starting WebSocket server at `0.0.0.0:6677`");
    websocket::Server::new(SocketAddr::from(([0, 0, 0, 0], 6677))).start()
}

// Disable because this is a false positive clippy lint
// https://github.com/rust-lang/rust-clippy/issues/3045
//
// And adding attributes to expressions is not yet supported
// https://github.com/rust-lang/rust/issues/15701
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn OnNewSentence(
    sentence: *const u16,
    sentence_info: *const InfoForExtension,
) -> *const u16 {
    // SAFETY: Constructing a  `U16Cstring` from `*const u16` is safe because
    // Textractor should return a valid pointer when this function is called
    let sentence_as_cstring = unsafe { U16CString::from_ptr_str(sentence) };

    // We cannot assume that the sentence will always be a valid UTF-8 string
    // because the text hook might be bad and contain random bytes
    let sentence_as_lossy_string = sentence_as_cstring.to_string_lossy();
    let sentence_info = SentenceInfo::new(sentence_info);
    let current_select = sentence_info.get_current_select();
    let text_number = sentence_info.get_text_number();
    if let CurrentSelect::UserSelectedTextThread(_) = current_select {
        if matches!(text_number, TextNumber::TextThread(_) | TextNumber::Clipboard) {
            SERVER
            .get_or_init(start_websocket_server)
            .send_message(sentence_as_lossy_string.into());
        }
    }

    sentence
}

#[no_mangle]
pub extern "system" fn DllMain(
    _h_module: HINSTANCE,
    fdw_reason: DWORD,
    _lpv_reserved: LPVOID,
) -> BOOL {
    if fdw_reason == DLL_PROCESS_ATTACH {
        SERVER
            .set(start_websocket_server())
            .unwrap_or_else(|_| panic!("WebSocket server should not have started yet"));
    }

    TRUE
}
