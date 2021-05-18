use crate::bindings::root as bindings;
use crate::btif::RawAddress;
use crate::ccall;
use crate::topstack::get_dispatchers;

use num_traits::cast::{FromPrimitive, ToPrimitive};
use std::sync::{Arc, Mutex};
use topshim_macros::cb_variant;

#[derive(Debug, FromPrimitive, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum BthhConnectionState {
    Connected = 0,
    Connecting,
    Disconnected,
    Disconnecting,
    Unknown = 0xff,
}

impl From<bindings::bthh_connection_state_t> for BthhConnectionState {
    fn from(item: bindings::bthh_connection_state_t) -> Self {
        BthhConnectionState::from_u32(item).unwrap_or_else(|| BthhConnectionState::Unknown)
    }
}

#[derive(Debug, FromPrimitive, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum BthhStatus {
    Ok = 0,
    HsHidNotReady,
    HsInvalidRptId,
    HsTransNotSpt,
    HsInvalidParam,
    HsError,
    Error,
    ErrSdp,
    ErrProto,
    ErrDbFull,
    ErrTodUnspt,
    ErrNoRes,
    ErrAuthFailed,
    ErrHdl,

    Unknown,
}

impl From<bindings::bthh_status_t> for BthhStatus {
    fn from(item: bindings::bthh_status_t) -> Self {
        BthhStatus::from_u32(item).unwrap_or_else(|| BthhStatus::Unknown)
    }
}

pub type BthhHidInfo = bindings::bthh_hid_info_t;

#[derive(Debug, FromPrimitive, ToPrimitive, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum BthhProtocolMode {
    ReportMode = 0,
    BootMode = 1,
    UnsupportedMode = 0xff,
}

impl From<bindings::bthh_protocol_mode_t> for BthhProtocolMode {
    fn from(item: bindings::bthh_protocol_mode_t) -> Self {
        BthhProtocolMode::from_u32(item).unwrap_or_else(|| BthhProtocolMode::UnsupportedMode)
    }
}

impl From<BthhProtocolMode> for bindings::bthh_protocol_mode_t {
    fn from(item: BthhProtocolMode) -> Self {
        item.to_u32().unwrap()
    }
}

#[derive(Debug, FromPrimitive, ToPrimitive, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum BthhReportType {
    InputReport = 1,
    OutputReport = 2,
    FeatureReport = 3,
}

impl From<BthhReportType> for bindings::bthh_report_type_t {
    fn from(item: BthhReportType) -> Self {
        item.to_u32().unwrap()
    }
}

fn convert_report(count: i32, raw: *mut u8) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::new();
    for i in 0..isize::from_i32(count).unwrap() {
        let p: *const u8 = unsafe { raw.offset(i) };
        v.push(unsafe { *p });
    }

    return v;
}

pub enum HHCallbacks {
    ConnectionState(RawAddress, BthhConnectionState),
    VirtualUnplug(RawAddress, BthhStatus),
    HidInfo(RawAddress, BthhHidInfo),
    ProtocolMode(RawAddress, BthhStatus, BthhProtocolMode),
    IdleTime(RawAddress, BthhStatus, i32),
    GetReport(RawAddress, BthhStatus, Vec<u8>, i32),
    Handshake(RawAddress, BthhStatus),
}

pub struct HHCallbacksDispatcher {
    dispatch: Box<dyn Fn(HHCallbacks) + Send>,
}

type HHCb = Arc<Mutex<HHCallbacksDispatcher>>;

cb_variant!(HHCb, connection_state_cb -> HHCallbacks::ConnectionState,
*mut RawAddress, bindings::bthh_connection_state_t -> BthhConnectionState, {
    let _0 = unsafe {*_0};
});
cb_variant!(HHCb, virtual_unplug_cb -> HHCallbacks::VirtualUnplug,
*mut RawAddress, bindings::bthh_status_t -> BthhStatus, {
    let _0 = unsafe {*_0};
});
cb_variant!(HHCb, hid_info_cb -> HHCallbacks::HidInfo,
*mut RawAddress, bindings::bthh_hid_info_t -> BthhHidInfo, {
    let _0 = unsafe {*_0};
});
cb_variant!(HHCb, protocol_mode_cb -> HHCallbacks::ProtocolMode,
*mut RawAddress, bindings::bthh_status_t -> BthhStatus,
bindings::bthh_protocol_mode_t -> BthhProtocolMode, {
    let _0 = unsafe {*_0};
});
cb_variant!(HHCb, idle_time_cb -> HHCallbacks::IdleTime,
*mut RawAddress, bindings::bthh_status_t -> BthhStatus, i32, {
    let _0 = unsafe {*_0};
});
cb_variant!(HHCb, get_report_cb -> HHCallbacks::GetReport,
*mut RawAddress, bindings::bthh_status_t -> BthhStatus, *mut u8, i32, {
    let _0 = unsafe {*_0};
    let _2 = convert_report(_3, _2);
});
cb_variant!(HHCb, handshake_cb -> HHCallbacks::Handshake,
*mut RawAddress, bindings::bthh_status_t -> BthhStatus, {
    let _0 = unsafe{*_0};
});

struct RawHHWrapper {
    raw: *const bindings::bthh_interface_t,
}

// Pointers unsafe due to ownership but this is a static pointer so Send is ok
unsafe impl Send for RawHHWrapper {}

pub struct HidHost {
    internal: RawHHWrapper,
    is_init: bool,
    // Keep callback object in memory (underlying code doesn't make copy)
    callbacks: Option<Box<bindings::bthh_callbacks_t>>,
}

impl HidHost {
    pub fn is_initialized(&self) -> bool {
        self.is_init
    }

    pub fn initialize(&mut self, callbacks: HHCallbacksDispatcher) -> bool {
        // Register dispatcher
        if get_dispatchers().lock().unwrap().set::<HHCb>(Arc::new(Mutex::new(callbacks))) {
            panic!("Tried to set dispatcher for HHCallbacks but it already existed");
        }

        let mut callbacks = Box::new(bindings::bthh_callbacks_t {
            size: 8 * 8,
            connection_state_cb: Some(connection_state_cb),
            hid_info_cb: Some(hid_info_cb),
            protocol_mode_cb: Some(protocol_mode_cb),
            idle_time_cb: Some(idle_time_cb),
            get_report_cb: Some(get_report_cb),
            virtual_unplug_cb: Some(virtual_unplug_cb),
            handshake_cb: Some(handshake_cb),
        });

        let rawcb = &mut *callbacks;

        let init = ccall!(self, init, rawcb);
        self.is_init = BthhStatus::from(init) == BthhStatus::Ok;
        self.callbacks = Some(callbacks);

        return self.is_init;
    }

    pub fn connect(&self, addr: &mut RawAddress) -> BthhStatus {
        BthhStatus::from(ccall!(self, connect, addr))
    }

    pub fn disconnect(&self, addr: &mut RawAddress) -> BthhStatus {
        BthhStatus::from(ccall!(self, disconnect, addr))
    }

    pub fn virtual_unplug(&self, addr: &mut RawAddress) -> BthhStatus {
        BthhStatus::from(ccall!(self, virtual_unplug, addr))
    }

    pub fn set_info(&self, addr: &mut RawAddress, info: BthhHidInfo) -> BthhStatus {
        BthhStatus::from(ccall!(self, set_info, addr, info))
    }

    pub fn get_protocol(&self, addr: &mut RawAddress, mode: BthhProtocolMode) -> BthhStatus {
        BthhStatus::from(ccall!(
            self,
            get_protocol,
            addr,
            bindings::bthh_protocol_mode_t::from(mode)
        ))
    }

    pub fn set_protocol(&self, addr: &mut RawAddress, mode: BthhProtocolMode) -> BthhStatus {
        BthhStatus::from(ccall!(
            self,
            set_protocol,
            addr,
            bindings::bthh_protocol_mode_t::from(mode)
        ))
    }

    pub fn get_idle_time(&self, addr: &mut RawAddress) -> BthhStatus {
        BthhStatus::from(ccall!(self, get_idle_time, addr))
    }

    pub fn set_idle_time(&self, addr: &mut RawAddress, idle_time: u8) -> BthhStatus {
        BthhStatus::from(ccall!(self, set_idle_time, addr, idle_time))
    }

    pub fn get_report(
        &self,
        addr: &mut RawAddress,
        report_type: BthhReportType,
        report_id: u8,
        buffer_size: i32,
    ) -> BthhStatus {
        BthhStatus::from(ccall!(
            self,
            get_report,
            addr,
            bindings::bthh_report_type_t::from(report_type),
            report_id,
            buffer_size
        ))
    }

    pub fn set_report(
        &self,
        addr: &mut RawAddress,
        report_type: BthhReportType,
        report: &mut [u8],
    ) -> BthhStatus {
        BthhStatus::from(ccall!(
            self,
            set_report,
            addr,
            bindings::bthh_report_type_t::from(report_type),
            report.as_mut_ptr() as *mut i8
        ))
    }

    pub fn cleanup(&self) {
        ccall!(self, cleanup)
    }
}
