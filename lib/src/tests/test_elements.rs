use std::{ffi::CStr, mem::MaybeUninit};

use crate::{
    msg::{dc_msg_free, dc_msg_get_data},
    msg_buf::dc_msg_buf_write,
    msg_receiver::dc_msg_receiver_recv,
    pipeline::{dc_pipeline_get_msg_buf, dc_pipeline_set_result_msg},
    plugin::{
        dc_element_new, dc_element_set_finalizer_creator, dc_plugin_register_element,
        DcElementResult, DcFinalizer, DcMsgReceiver, DcPipeline, DcPlugin,
    },
};
use byteorder::{ByteOrder, NativeEndian, WriteBytesExt};
use libc::{c_char, c_void};

pub unsafe extern "C-unwind" fn dc_plugin_init_test(plugin: *mut DcPlugin) -> bool {
    unsafe {
        let element = dc_element_new(
            "test-src\0".as_ptr() as _,
            0,
            1,
            test_src_new,
            test_src_next,
            test_src_free,
        );
        dc_element_set_finalizer_creator(element, test_src_finalizer_creator);
        dc_plugin_register_element(plugin, element);

        let element = dc_element_new(
            "test-sink\0".as_ptr() as _,
            1,
            0,
            test_sink_new,
            test_sink_next,
            test_sink_free,
        );
        dc_plugin_register_element(plugin, element);

        let element = dc_element_new(
            "test-filter\0".as_ptr() as _,
            1,
            1,
            test_filter_new,
            test_filter_next,
            test_filter_free,
        );
        dc_plugin_register_element(plugin, element);

        true
    }
}

#[derive(serde::Deserialize)]
pub struct TestSrcConf {
    repeat: i32,
}

unsafe extern "C-unwind" fn test_src_new(config: *const c_char) -> *mut c_void {
    unsafe {
        let config = CStr::from_ptr(config).to_str().unwrap();
        let conf: TestSrcConf = serde_json::from_str(config).unwrap();
        Box::into_raw(Box::new((conf, 0i32))) as _
    }
}

unsafe extern "C-unwind" fn test_src_next(
    element: *mut c_void,
    pipeline: *mut DcPipeline,
    _msg_receiver: *mut DcMsgReceiver,
) -> DcElementResult {
    unsafe {
        let element: &mut (TestSrcConf, i32) = &mut *(element as *mut _);

        element.1 += 1;

        if element.1 == element.0.repeat {
            return DcElementResult::Close;
        }

        let msg_buf = dc_pipeline_get_msg_buf(pipeline, 0);

        let mut buf = Vec::new();
        buf.write_i32::<NativeEndian>(element.1).unwrap();

        dc_msg_buf_write(msg_buf, buf.as_ptr(), buf.len());

        DcElementResult::MsgBuf
    }
}

unsafe extern "C-unwind" fn test_src_finalizer_creator(
    _p: *mut c_void,
    finalizer: *mut DcFinalizer,
) -> bool {
    unsafe {
        *finalizer = DcFinalizer {
            context: std::ptr::null_mut(),
            f: Some(test_src_finalizer),
        };
        true
    }
}

unsafe extern "C-unwind" fn test_src_finalizer(_: *mut c_void) -> bool {
    eprintln!("test_src_finalizer");
    true
}

unsafe extern "C-unwind" fn test_src_free(p: *mut c_void) {
    let p: *mut (TestSrcConf, i32) = p as _;
    let _ = unsafe { Box::from_raw(p) };
    eprintln!("test_src_free");
}

unsafe extern "C-unwind" fn test_sink_new(_config: *const c_char) -> *mut c_void {
    Box::into_raw(Box::new(0i32)) as _
}

unsafe extern "C-unwind" fn test_sink_next(
    element: *mut c_void,
    _pipeline: *mut DcPipeline,
    msg_receiver: *mut DcMsgReceiver,
) -> DcElementResult {
    unsafe {
        let _element: &mut i32 = &mut *(element as *mut i32);
        let mut msg = MaybeUninit::uninit();

        loop {
            if dc_msg_receiver_recv(msg_receiver, 0, msg.as_mut_ptr()) {
                let msg = msg.assume_init();
                let mut len = 0;
                let mut data = std::ptr::null();
                dc_msg_get_data(&msg, &mut data, &mut len);
                let data = std::slice::from_raw_parts(data, len);
                let _data = NativeEndian::read_i32(data);
                dc_msg_free(msg);
            } else {
                return DcElementResult::Close;
            }
        }
    }
}

unsafe extern "C-unwind" fn test_sink_free(p: *mut c_void) {
    let p: *mut i32 = p as _;
    let _ = unsafe { Box::from_raw(p) };
    eprintln!("test_sink_free");
}

unsafe extern "C-unwind" fn test_filter_new(_config: *const c_char) -> *mut c_void {
    Box::into_raw(Box::new(1i32)) as _
}

unsafe extern "C-unwind" fn test_filter_next(
    element: *mut c_void,
    pipeline: *mut DcPipeline,
    msg_receiver: *mut DcMsgReceiver,
) -> DcElementResult {
    unsafe {
        let _element: &mut i32 = &mut *(element as *mut i32);
        let mut msg = MaybeUninit::uninit();

        if dc_msg_receiver_recv(msg_receiver, 0, msg.as_mut_ptr()) {
            let msg = msg.assume_init();
            dc_pipeline_set_result_msg(pipeline, 0, msg);
            DcElementResult::Msg
        } else {
            DcElementResult::Close
        }
    }
}

unsafe extern "C-unwind" fn test_filter_free(p: *mut c_void) {
    let p: *mut i32 = p as _;
    let _ = unsafe { Box::from_raw(p) };
    eprintln!("test_filter_free");
}
